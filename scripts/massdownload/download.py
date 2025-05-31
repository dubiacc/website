import os
import requests
from urllib.parse import urlparse
import re
from concurrent.futures import ThreadPoolExecutor, as_completed
import sys
import time
import random

# --- Configuration ---
URL_FILE = 'urls.txt'  # Path to the file containing URLs, one per line
OUTPUT_DIR = 'downloaded_files'  # Directory to save downloaded files
MAX_WORKERS = 10       # Number of concurrent downloads (adjust based on your network and target server)
REQUEST_TIMEOUT = 15   # Timeout for each request in seconds
USER_AGENT = "Python URL Downloader/1.0 (https://github.com/yourusername/yourproject)" # Identify your script

# Delay between downloads to be polite (randomized for less predictable pattern)
MIN_DELAY_SECONDS = 0.5 # Minimum delay after each download task completes
MAX_DELAY_SECONDS = 2.0 # Maximum delay after each download task completes

# --- Helper Function for Filename Sanitization ---
def sanitize_filename(url, counter, max_length=150):
    """
    Creates a valid and unique base filename from a URL, prepending a counter.
    Attempts to retain some original path information. Does NOT include final extension.
    """
    parsed_url = urlparse(url)
    
    # Try to get a sensible filename from the path
    filename_hint = os.path.basename(parsed_url.path)
    
    # If path is just a domain or ends with '/', use 'index'
    if not filename_hint:
        filename_hint = 'index'
    
    # Remove query parameters from the hint (e.g., ?id=123)
    if '?' in filename_hint:
        filename_hint = filename_hint.split('?')[0]
        
    # Clean invalid characters for filenames (platform-independent)
    # Allow alphanumeric, underscore, hyphen, and period
    filename_hint = re.sub(r'[^\w\.\-_]', '_', filename_hint)
    
    # Prepend counter for uniqueness and order
    # Format counter with leading zeros (e.g., 00001, 00002)
    prefix = f"{counter:05d}_" 
    
    # Ensure total filename length doesn't exceed common OS limits (e.g., 255 chars)
    # We leave room for the prefix and a potential extension
    available_length = max_length - len(prefix)
    if len(filename_hint) > available_length:
        # Truncate from the middle, keep beginning and end
        # This helps retain both file type and some context
        half_len = available_length // 2
        filename_hint = filename_hint[:half_len] + "..." + filename_hint[-half_len:]
        # Remove multiple '...' if they appeared from truncation
        filename_hint = filename_hint.replace("....", "...") 

    return f"{prefix}{filename_hint}"

# --- Main Download Function ---
def download_url(url, output_path, counter):
    """
    Downloads a single URL and saves it to the specified output_path.
    Returns (True, message) on success or skip, (False, error_message) on failure.
    Includes a check for existing files before downloading.
    """
    headers = {'User-Agent': USER_AGENT}
    base_filename = sanitize_filename(url, counter)
    
    # --- Pre-check for existing file ---
    # We try to determine a likely extension from the URL path for the pre-check.
    # This is a heuristic; the actual extension might differ based on Content-Type.
    parsed_url = urlparse(url)
    path_basename = os.path.basename(parsed_url.path)
    
    # Get a guessed extension from the URL path if available
    guessed_extension = ''
    if '.' in path_basename:
        name, ext = os.path.splitext(path_basename)
        if len(ext) > 1 and len(ext) <= 5 and all(c.isalnum() for c in ext[1:]):
            guessed_extension = ext.lower()
    
    # If no specific extension guessed from URL, try common defaults for pre-check
    # This list can be expanded based on your expected content
    pre_check_extensions = [guessed_extension, '.html', '.pdf', '.jpg', '.png', '.json', '.txt', '.xml', '.bin']
    
    found_existing_file = False
    existing_filename = None

    for ext in pre_check_extensions:
        if not ext: continue # Skip empty strings if guessed_extension was empty
        
        # Build path with base filename and potential extension
        potential_filename_with_ext = f"{base_filename}{ext}"
        full_potential_path = os.path.join(output_path, potential_filename_with_ext)

        # Check for direct existence or existence with collision suffix (e.g., _1, _2)
        if os.path.exists(full_potential_path):
            found_existing_file = True
            existing_filename = os.path.basename(full_potential_path)
            break
        else:
            # Also check for collision suffixes if the exact match wasn't found
            # This is a bit more robust but still a heuristic.
            # We iterate a few times for common collision counts.
            for i in range(1, 5): # Check for _1, _2, _3, _4
                potential_filename_with_collision_ext = f"{os.path.splitext(potential_filename_with_ext)[0]}_{i}{os.path.splitext(potential_filename_with_ext)[1]}"
                full_potential_path_collision = os.path.join(output_path, potential_filename_with_collision_ext)
                if os.path.exists(full_potential_path_collision):
                    found_existing_file = True
                    existing_filename = os.path.basename(full_potential_path_collision)
                    break
            if found_existing_file:
                break # Break from outer loop too if found with collision

    if found_existing_file:
        time.sleep(random.uniform(MIN_DELAY_SECONDS, MAX_DELAY_SECONDS)) # Still add a small delay
        return True, f"Skipped {url}: File already exists at {existing_filename}"

    # --- Proceed with download if file does not exist ---
    try:
        response = requests.get(url, stream=True, timeout=REQUEST_TIMEOUT, headers=headers)
        response.raise_for_status()  # Raise HTTPError for bad responses (4xx or 5xx)

        # --- Determine Final File Extension (more robustly after response) ---
        actual_extension = ''
        content_type = response.headers.get('Content-Type', '').split(';')[0].strip().lower()

        # Common MIME type to extension mapping
        if 'image/jpeg' in content_type or 'image/jpg' in content_type: actual_extension = '.jpg'
        elif 'image/png' in content_type: actual_extension = '.png'
        elif 'image/gif' in content_type: actual_extension = '.gif'
        elif 'application/pdf' in content_type: actual_extension = '.pdf'
        elif 'application/json' in content_type: actual_extension = '.json'
        elif 'text/html' in content_type: actual_extension = '.html'
        elif 'text/plain' in content_type: actual_extension = '.txt'
        elif 'application/xml' in content_type or 'text/xml' in content_type: actual_extension = '.xml'
        
        # Fallback: Use guessed_extension if no clear MIME type or if it's more specific
        if not actual_extension and guessed_extension:
            actual_extension = guessed_extension
        
        # Default extension if none found
        if not actual_extension:
            actual_extension = '.bin' # Generic binary file

        # --- Create Final Filename ---
        final_filename = f"{base_filename}{actual_extension}"
        full_file_path = os.path.join(output_path, final_filename)

        # Handle potential filename collisions (even after pre-check, due to dynamic extension)
        original_full_file_path = full_file_path
        collision_count = 1
        while os.path.exists(full_file_path):
            name, ext = os.path.splitext(original_full_file_path)
            full_file_path = f"{name}_{collision_count}{ext}"
            collision_count += 1

        # --- Save the content ---
        with open(full_file_path, 'wb') as f:
            for chunk in response.iter_content(chunk_size=8192):
                f.write(chunk)
        
        return True, f"Downloaded {url} to {os.path.basename(full_file_path)}"

    except requests.exceptions.HTTPError as e:
        return False, f"HTTP Error for {url}: {e.response.status_code} - {e.response.reason}"
    except requests.exceptions.ConnectionError as e:
        return False, f"Connection Error for {url}: {e}"
    except requests.exceptions.Timeout:
        return False, f"Timeout Error for {url} after {REQUEST_TIMEOUT} seconds"
    except requests.exceptions.RequestException as e:
        return False, f"Request Error for {url}: {e}"
    except Exception as e:
        return False, f"An unexpected error occurred for {url}: {e}"
    finally:
        # Add a random delay after each download attempt (success or failure)
        time.sleep(random.uniform(MIN_DELAY_SECONDS, MAX_DELAY_SECONDS))

# --- Main Script Logic ---
def main():
    # Create the output directory if it doesn't exist
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    urls_to_download = []
    try:
        with open(URL_FILE, 'r') as f:
            urls_to_download = [line.strip() for line in f if line.strip()]
    except FileNotFoundError:
        print(f"Error: URL file '{URL_FILE}' not found. Please create it and add URLs.")
        sys.exit(1)

    total_urls = len(urls_to_download)
    if total_urls == 0:
        print("No URLs found in the file. Exiting.")
        sys.exit(0)

    print(f"Attempting to download {total_urls} URLs to '{OUTPUT_DIR}' using {MAX_WORKERS} workers.")
    print(f"Delay between requests: {MIN_DELAY_SECONDS:.1f}-{MAX_DELAY_SECONDS:.1f} seconds per worker.")


    # Try to import tqdm for progress bar
    try:
        from tqdm import tqdm
        progress_bar = tqdm(total=total_urls, unit="URL", desc="Downloading")
        use_tqdm = True
    except ImportError:
        print("tqdm not found. Install with 'pip install tqdm' for a progress bar.")
        use_tqdm = False

    downloaded_count = 0
    skipped_count = 0
    failed_urls = []

    # Use ThreadPoolExecutor for concurrent downloads
    with ThreadPoolExecutor(max_workers=MAX_WORKERS) as executor:
        # Submit tasks and map futures back to original URLs
        # Enumerate gives us a counter for unique filenames
        futures = {executor.submit(download_url, url, OUTPUT_DIR, i + 1): url for i, url in enumerate(urls_to_download)}

        for future in as_completed(futures):
            url = futures[future]  # Get the original URL for this completed future
            try:
                success, message = future.result()
                if success:
                    if "Skipped" in message: # Check if the message indicates a skip
                        skipped_count += 1
                    else:
                        downloaded_count += 1
                else:
                    failed_urls.append((url, message))
                
                # Print message if not using tqdm, or if it's an error/skip
                if not use_tqdm or not success or "Skipped" in message:
                    print(message)
                
            except Exception as e:
                # Catch any unexpected errors from the future itself
                failed_urls.append((url, f"An unexpected error processing future for {url}: {e}"))
                if not use_tqdm:
                    print(f"An unexpected error processing future for {url}: {e}")

            if use_tqdm:
                progress_bar.update(1) # Increment progress bar

    if use_tqdm:
        progress_bar.close()

    # --- Summary ---
    print("\n--- Download Summary ---")
    print(f"Total URLs processed: {total_urls}")
    print(f"Successfully downloaded: {downloaded_count}")
    print(f"Skipped (already exists): {skipped_count}")
    print(f"Failed downloads: {len(failed_urls)}")

    if failed_urls:
        print("\n--- Failed URLs ---")
        for url, reason in failed_urls:
            print(f"  - {url}: {reason}")

if __name__ == "__main__":
    main()