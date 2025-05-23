# pip3 install --break-system-packages google-generativeai markdown

import os
import argparse
import google.generativeai as genai
import json
import re
import logging
from concurrent.futures import ThreadPoolExecutor, as_completed
import time

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

def slugify(text):
    """
    Converts a string to a URL-friendly slug.
    Removes non-alphanumeric characters, converts spaces to hyphens, and lowercases.
    """
    text = text.strip().lower()
    text = re.sub(r'[^\w\s-]', '', text) # Remove all non-word chars except spaces and hyphens
    text = re.sub(r'[\s_-]+', '-', text) # Replace spaces/underscores with single hyphen
    return text.strip('-') # Remove leading/trailing hyphens

def translate_file(input_filepath, output_dir, model, target_language):
    """
    Translates a single markdown file using Gemini AI.
    Asks for both content and filename translation in one request.
    """
    original_filename = os.path.basename(input_filepath)
    filename_without_ext, ext = os.path.splitext(original_filename)

    logging.info(f"Processing '{original_filename}'...")

    try:
        with open(input_filepath, 'r', encoding='utf-8') as f:
            markdown_content = f.read()

        if not markdown_content.strip():
            logging.warning(f"Skipping empty file: '{original_filename}'.")
            return

        # Prompt for Gemini to return both content and filename in JSON
        prompt = f"""
        Your task is to translate the provided markdown content and its corresponding filename into {target_language}. 
        The content should have a maximum of 90 characters per line.

        The translated filename should be a slug-friendly version suitable for a file path (lowercase, hyphens instead of spaces, 
        no special characters, no file extension).

        Please respond with a JSON object containing two keys:
        - `translated_content`: The translated markdown content.
        - `translated_filename`: The translated filename slug (without extension).

        Original filename: "{filename_without_ext}"

        Markdown content:
        ```markdown
        {markdown_content}
        ```
        """

        # Make the API request
        # Using generation_config to ensure JSON output and adjust temperature for more direct translation
        response = model.generate_content(
            prompt,
            generation_config=genai.types.GenerationConfig(
                temperature=1.0, # Lower temperature for less creative, more direct translation
                response_mime_type="application/json" # Request JSON output
            )
        )

        # Parse the JSON response
        try:
            # Access the content directly from text attribute for JSON responses
            response_json = json.loads(response.text)
            translated_content = response_json.get('translated_content')
            translated_filename_raw = response_json.get('translated_filename')

            if not translated_content or not translated_filename_raw:
                raise ValueError("Gemini response did not contain expected 'translated_content' or 'translated_filename' keys.")

            # Ensure the translated filename is slug-friendly
            translated_filename = slugify(translated_filename_raw)

            output_filename = f"{translated_filename}{ext}"
            output_filepath = os.path.join(output_dir, output_filename)

            with open(output_filepath, 'w', encoding='utf-8') as f:
                f.write(translated_content)

            logging.info(f"Successfully translated '{original_filename}' to '{output_filename}'.")

        except json.JSONDecodeError as e:
            logging.error(f"Error decoding JSON response for '{original_filename}': {e}. Response: {response.text}")
            raise
        except ValueError as e:
            logging.error(f"Invalid data in Gemini response for '{original_filename}': {e}. Response: {response.text}")
            raise

    except genai.types.core.GenerationError as e:
        logging.error(f"Gemini API error for '{original_filename}': {e}")
    except FileNotFoundError:
        logging.error(f"File not found: '{input_filepath}'")
    except Exception as e:
        logging.error(f"An unexpected error occurred while processing '{original_filename}': {e}")

def main():
    parser = argparse.ArgumentParser(
        description="Translate Markdown files in a directory using Gemini AI.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter
    )
    parser.add_argument(
        "--input-dir",
        type=str,
        required=True,
        help="Path to the directory containing input Markdown files."
    )
    parser.add_argument(
        "--output-dir",
        type=str,
        required=True,
        help="Path to the directory where translated Markdown files will be saved."
    )
    parser.add_argument(
        "--api-key",
        type=str,
        required=True,
        help="Your Google Gemini API key."
    )
    parser.add_argument(
        "--language",
        type=str,
        required=True,
        help="The target language for translation (e.g., 'German', 'French', 'Brazilian Portuguese', 'Spanish')."
    )
    parser.add_argument(
        "--max-workers",
        type=int,
        default=5,
        help="Maximum number of concurrent translation tasks. Adjust based on API rate limits and machine resources."
    )

    args = parser.parse_args()

    # Validate input directory
    if not os.path.isdir(args.input_dir):
        logging.error(f"Input directory not found: '{args.input_dir}'")
        exit(1)

    # Create output directory if it doesn't exist
    os.makedirs(args.output_dir, exist_ok=True)
    logging.info(f"Output directory set to: '{args.output_dir}'")

    # Configure Gemini AI
    genai.configure(api_key=args.api_key)
    try:
        # Use a model that supports JSON mode, e.g., gemini-1.5-pro or gemini-1.0-pro (if available/suitable)
        # Note: gemini-pro is often good for text, but gemini-1.5-pro offers better instruction following
        # and typically has the response_mime_type feature readily available.
        model = genai.GenerativeModel('gemini-2.5-flash-preview-05-20')
        logging.info("Gemini model 'gemini-2.5-flash-preview-05-20' initialized.")
    except Exception as e:
        logging.error(f"Failed to initialize Gemini model: {e}")
        logging.info("Attempting to use 'gemini-pro' instead.")
        try:
            model = genai.GenerativeModel('gemini-pro')
            logging.info("Gemini model 'gemini-pro' initialized.")
        except Exception as e:
            logging.error(f"Failed to initialize 'gemini-pro' model. Please check your API key and network connection: {e}")
            exit(1)


    markdown_files = []
    for root, _, files in os.walk(args.input_dir):
        for file in files:
            if file.endswith(('.md', '.markdown')):
                markdown_files.append(os.path.join(root, file))

    if not markdown_files:
        logging.warning(f"No Markdown files found in '{args.input_dir}'. Exiting.")
        return

    logging.info(f"Found {len(markdown_files)} Markdown files to translate.")

    # Use ThreadPoolExecutor for concurrent processing
    # Be mindful of API rate limits if increasing max_workers significantly
    with ThreadPoolExecutor(max_workers=args.max_workers) as executor:
        futures = {executor.submit(translate_file, filepath, args.output_dir, model, args.language): filepath for filepath in markdown_files}
        
        for future in as_completed(futures):
            filepath = futures[future]
            try:
                future.result() # This will re-raise any exceptions from the translate_file function
            except Exception as exc:
                logging.error(f'{filepath} generated an exception: {exc}')

    logging.info("Translation process completed.")

if __name__ == "__main__":
    main()