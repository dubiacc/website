import os
import argparse
from selenium import webdriver
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.common.by import By
from selenium.webdriver.chrome.options import Options
from selenium.common.exceptions import WebDriverException, NoSuchElementException, TimeoutException
from tqdm import tqdm
import sys
import re

# Try to import ChromeDriverManager for automatic driver management
try:
    from webdriver_manager.chrome import ChromeDriverManager
    USE_WEBDRIVER_MANAGER = True
except ImportError:
    print("Warning: webdriver_manager not found. Install with 'pip install webdriver-manager' for automatic ChromeDriver management.")
    print("You will need to manually specify --webdriver-path if your ChromeDriver is not in PATH.")
    USE_WEBDRIVER_MANAGER = False

def sanitize_text_for_metadata(text, max_length=100):
    """
    Sanitizes text for safe inclusion within the metadata block of the output .txt file.
    It replaces characters that might be problematic in some contexts, and truncates if too long.
    This function is NOT used for sanitizing filenames themselves.
    """
    if not text:
        return ""
    # Replace control characters and other problematic ones with a space or underscore
    s = re.sub(r'[\x00-\x1F\x7F-\x9F]', ' ', text) # Remove non-printable ASCII chars
    s = re.sub(r'[\s]+', ' ', s).strip() # Normalize whitespace
    
    # Truncate if too long (for readability within the text file)
    if len(s) > max_length:
        s = s[:max_length].rsplit(' ', 1)[0] + "..." if ' ' in s[:max_length] else s[:max_length] + "..."
    return s


def extract_text_from_html(
    html_file_path,
    output_dir,
    main_css_selector,
    chrome_path,
    webdriver_path=None,
    author_css_selector=None,
    date_css_selector=None,
    title_css_selector=None
):
    """
    Loads an HTML file in a headless Chrome browser, extracts text based on CSS selectors,
    and saves it to a .txt file.
    This function no longer performs a pre-check for existing output files;
    that logic is now handled in the main function.
    """
    # Get the base HTML filename, including its extension (e.g., 'my_page.html')
    html_base_name = os.path.basename(html_file_path)

    extracted_text = []
    metadata_prefix_parts = [] # This list will hold metadata strings for content inside the file

    # Configure Chrome options
    chrome_options = Options()
    chrome_options.add_argument("--headless")  # Run Chrome in headless mode
    chrome_options.add_argument("--disable-gpu") # Recommended for headless
    chrome_options.add_argument("--no-sandbox") # Bypass OS security model, required in some environments
    chrome_options.add_argument("--disable-dev-shm-usage") # Overcome limited resource problems
    chrome_options.add_argument("--log-level=3") # Suppress verbose logs
    chrome_options.binary_location = chrome_path # Set custom Chrome binary path

    driver = None
    try:
        if USE_WEBDRIVER_MANAGER and webdriver_path is None:
            service = Service(ChromeDriverManager().install())
        elif webdriver_path:
            service = Service(webdriver_path)
        else:
            raise FileNotFoundError("ChromeDriver path not specified and webdriver_manager not available.")

        driver = webdriver.Chrome(service=service, options=chrome_options)
        driver.set_page_load_timeout(30) # Set a page load timeout to prevent indefinite hanging
        
        # Load the local HTML file (must be absolute path)
        driver.get(f"file:///{os.path.abspath(html_file_path)}")

        # --- Extract Metadata (Optional) ---
        if author_css_selector:
            try:
                author_elements = driver.find_elements(By.CSS_SELECTOR, author_css_selector)
                if author_elements:
                    author_text = " ".join([e.text.strip() for e in author_elements if e.text.strip()])
                    if author_text:
                        metadata_prefix_parts.append(f"Author: {sanitize_text_for_metadata(author_text)}")
            except NoSuchElementException:
                pass
            except Exception as e:
                print(f"Warning: Error extracting author from {html_file_path}: {e}", file=sys.stderr)

        if date_css_selector:
            try:
                date_elements = driver.find_elements(By.CSS_SELECTOR, date_css_selector)
                if date_elements:
                    date_text = " ".join([e.text.strip() for e in date_elements if e.text.strip()])
                    if date_text:
                        metadata_prefix_parts.append(f"Date: {sanitize_text_for_metadata(date_text)}")
            except NoSuchElementException:
                pass
            except Exception as e:
                print(f"Warning: Error extracting date from {html_file_path}: {e}", file=sys.stderr)

        if title_css_selector:
            try:
                title_elements = driver.find_elements(By.CSS_SELECTOR, title_css_selector)
                if title_elements:
                    title_text = " ".join([e.text.strip() for e in title_elements if e.text.strip()])
                    if title_text:
                        metadata_prefix_parts.append(f"Title: {sanitize_text_for_metadata(title_text)}")
            except NoSuchElementException:
                pass
            except Exception as e:
                print(f"Warning: Error extracting title from {html_file_path}: {e}", file=sys.stderr)

        # --- Extract Main Content ---
        main_content_elements = driver.find_elements(By.CSS_SELECTOR, main_css_selector)
        if not main_content_elements:
            return False, f"No elements found for main CSS selector '{main_css_selector}' in {html_file_path}"

        for element in main_content_elements:
            text = element.text.strip()
            if text:
                extracted_text.append(text)

        if not extracted_text:
            return False, f"Extracted no visible text for main CSS selector '{main_css_selector}' in {html_file_path}"
        
        # --- Prepare Output Filename: X.html -> X.html.txt ---
        output_filename = f"{html_base_name}.txt"
            
        # Basic OS-level sanitization for filename characters (applies to 'X.html.txt' itself)
        # This is a general safety measure, replacing forbidden chars like \ / : * ? " < > |
        sanitized_output_filename = re.sub(r'[\\/:*?"<>|]', '_', output_filename)
        
        output_file_path = os.path.join(output_dir, sanitized_output_filename)

        # --- Write to File ---
        with open(output_file_path, 'w', encoding='utf-8') as f:
            if metadata_prefix_parts:
                f.write("--- METADATA ---\n")
                f.write("\n".join(metadata_prefix_parts))
                f.write("\n--------------\n\n")
            f.write("\n\n".join(extracted_text)) # Join main content with double newlines for readability

        return True, f"Successfully extracted text from {os.path.basename(html_file_path)} to {os.path.basename(output_file_path)}"

    except FileNotFoundError as e:
        return False, f"Error: ChromeDriver or Chrome binary not found. {e}"
    except WebDriverException as e:
        return False, f"Selenium WebDriver error for {os.path.basename(html_file_path)}: {e}"
    except TimeoutException:
        return False, f"Page load timed out for {os.path.basename(html_file_path)}"
    except Exception as e:
        return False, f"An unexpected error occurred for {os.path.basename(html_file_path)}: {e}"
    finally:
        if driver:
            driver.quit() # Always close the browser instance

def main():
    parser = argparse.ArgumentParser(
        description="Extract visible text from HTML files using a headless Chrome browser.",
        formatter_class=argparse.RawTextHelpFormatter # For multiline help text
    )
    parser.add_argument(
        "input_dir",
        help="Path to the directory containing HTML files (.html, .htm)."
    )
    parser.add_argument(
        "output_dir",
        help="Path to the directory where extracted text files will be saved."
    )
    parser.add_argument(
        "main_css_selector",
        help="CSS selector for the main content to extract (e.g., 'article', 'div.content', 'body')."
    )
    parser.add_argument(
        "--chrome-path",
        required=True,
        help="Full path to the Google Chrome executable (e.g., '/usr/bin/google-chrome' or 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe')."
    )
    parser.add_argument(
        "--webdriver-path",
        help="Full path to the ChromeDriver executable. "
             "Optional if ChromeDriver is in your system's PATH or if 'webdriver-manager' is installed (recommended)."
    )
    parser.add_argument(
        "--author-css",
        help="Optional: CSS selector for the author (e.g., '.author-name', 'span[itemprop=\"author\"]')."
    )
    parser.add_argument(
        "--date-css",
        help="Optional: CSS selector for the date (e.g., '.post-date', 'time')."
    )
    parser.add_argument(
        "--title-css",
        help="Optional: CSS selector for the title (e.g., 'h1.entry-title', 'head title')."
    )

    args = parser.parse_args()

    # Validate input directory
    if not os.path.isdir(args.input_dir):
        print(f"Error: Input directory '{args.input_dir}' not found.")
        sys.exit(1)

    # Create output directory if it doesn't exist
    os.makedirs(args.output_dir, exist_ok=True)

    # 1. Get all HTML files from the input directory
    html_files = []
    for root, _, files in os.walk(args.input_dir):
        for file in files:
            if file.lower().endswith(('.html', '.htm')):
                html_files.append(os.path.join(root, file))
    
    if not html_files:
        print(f"No HTML files found in '{args.input_dir}'. Exiting.")
        sys.exit(0)

    # 2. Determine which files have already been processed
    # Build a set of *sanitized* expected output filenames (e.g., 'my_file.html.txt')
    # that already exist in output_dir
    processed_output_filenames = set()
    if os.path.isdir(args.output_dir): # Ensure output_dir exists before listing contents
        for existing_output_file in os.listdir(args.output_dir):
            if existing_output_file.lower().endswith('.txt'):
                # Apply the same sanitization that extract_text_from_html applies to its output file name
                sanitized_existing_output_file = re.sub(r'[\\/:*?"<>|]', '_', existing_output_file)
                processed_output_filenames.add(sanitized_existing_output_file)

    html_files_to_process = []
    skipped_extractions = 0

    print(f"Found {len(html_files)} HTML files in '{args.input_dir}'.")
    print(f"Checking for already processed files in '{args.output_dir}'...")

    for html_file_path in html_files:
        # Calculate the *expected* output filename for the current HTML file
        html_base_name = os.path.basename(html_file_path)
        expected_output_filename = f"{html_base_name}.txt"

        if expected_output_filename in processed_output_filenames:
            skipped_extractions += 1
            # Using tqdm.write to print without disturbing the progress bar
            tqdm.write(f"Skipped {html_base_name}: Output file '{expected_output_filename}' already exists.")
        else:
            html_files_to_process.append(html_file_path)

    print(f"Skipped {skipped_extractions} files that appear to be already processed.")
    print(f"Will process {len(html_files_to_process)} files.")
    print(f"Extracting text to '{args.output_dir}'...")

    successful_extractions = 0
    failed_extractions = []

    # Process files with a progress bar using the filtered list
    for html_file in tqdm(html_files_to_process, desc="Processing HTML files"):
        success, message = extract_text_from_html(
            html_file,
            args.output_dir,
            args.main_css_selector,
            args.chrome_path,
            args.webdriver_path,
            args.author_css,
            args.date_css,
            args.title_css
        )
        if success:
            successful_extractions += 1
        else:
            failed_extractions.append((html_file, message))
            tqdm.write(message) # Print error messages immediately for failures

    print("\n--- Summary ---")
    print(f"Total HTML files found: {len(html_files)}")
    print(f"Skipped (already processed): {skipped_extractions}")
    print(f"Successfully extracted in this run: {successful_extractions}")
    print(f"Failed extractions: {len(failed_extractions)}")

    if failed_extractions:
        print("\n--- Failed Files ---")
        for html_file, reason in failed_extractions:
            print(f"  - {os.path.basename(html_file)}: {reason}")

if __name__ == "__main__":
    main()