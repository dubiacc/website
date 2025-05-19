#!/usr/bin/env python3
"""
OGG Transcription Tool

This script takes all .ogg files in a given directory, transcribes them using 
the ElevenLabs API directly, and outputs the transcripts to a target directory.
"""

import os
import json
import requests
import argparse
from datetime import datetime
from pathlib import Path
from typing import Optional, Dict, Any, List

def log(message: str) -> None:
    """Timestamped logging function"""
    timestamp = datetime.now().strftime("%H:%M:%S")
    print(f"[{timestamp}] {message}")

def transcribe_audio_with_elevenlabs(
    audio_path: str, 
    api_key: str,
    model_id: str,
    output_dir: str
) -> Optional[Dict[str, Any]]:
    """Transcribe audio using ElevenLabs API directly"""
    log(f"Transcribing audio file: {audio_path}")
    
    API_URL = "https://api.elevenlabs.io/v1/speech-to-text"
    
    headers = {
        "xi-api-key": api_key
    }
    
    # Prepare the multipart form data
    with open(audio_path, "rb") as audio_file:
        files = {
            "file": (os.path.basename(audio_path), audio_file, "audio/ogg")
        }
        
        data = {
            "model_id": model_id,
            "diarize": "true",  # Enable speaker identification
            "tag_audio_events": "true",  # Tag audio events like laughter, etc.
            "timestamps_granularity": "word"  # Get word-level timestamps
        }
        
        try:
            log("Sending request to ElevenLabs API...")
            response = requests.post(API_URL, headers=headers, data=data, files=files)
            
            if response.status_code == 200:
                result = response.json()
                
                # Save raw JSON response
                base_filename = os.path.basename(audio_path)
                filename_no_ext = os.path.splitext(base_filename)[0]
                json_path = os.path.join(output_dir, f"{filename_no_ext}_transcription.json")
                
                with open(json_path, 'w', encoding='utf-8') as f:
                    json.dump(result, f, indent=2, ensure_ascii=False)
                
                # Save plain text transcription
                text_path = os.path.join(output_dir, f"{filename_no_ext}_transcription.txt")
                with open(text_path, 'w', encoding='utf-8') as f:
                    f.write(result.get("text", ""))
                
                log(f"Transcription successful, saved to {json_path} and {text_path}")
                return result
            else:
                log(f"Transcription failed: {response.status_code} - {response.text}")
                return None
                
        except Exception as e:
            log(f"Exception during transcription: {str(e)}")
            return None

def find_ogg_files(directory: str) -> List[str]:
    """Find all .ogg files in the given directory"""
    ogg_files = []
    for file in os.listdir(directory):
        if file.lower().endswith('.ogg'):
            ogg_files.append(os.path.join(directory, file))
    return ogg_files

def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(description="OGG Transcription Tool")
    
    parser.add_argument(
        "--input-dir", "-i",
        required=True,
        help="Directory containing .ogg files"
    )
    
    parser.add_argument(
        "--output-dir", "-o",
        required=True,
        help="Directory to save transcriptions"
    )
    
    parser.add_argument(
        "--api-key", "-k",
        required=True,
        help="ElevenLabs API key"
    )
    
    parser.add_argument(
        "--model-id", "-m",
        default="scribe_v1",
        help="Model ID to use for transcription (default: scribe_v1)"
    )
    
    args = parser.parse_args()
    
    # Create output directory if it doesn't exist
    os.makedirs(args.output_dir, exist_ok=True)
    
    # Find all .ogg files
    ogg_files = find_ogg_files(args.input_dir)
    
    if not ogg_files:
        log(f"No .ogg files found in {args.input_dir}")
        return
    
    log(f"Found {len(ogg_files)} .ogg files to transcribe")
    
    # Process each file
    successful = 0
    failed = 0
    
    for audio_path in ogg_files:
        log(f"Processing {os.path.basename(audio_path)}...")
        result = transcribe_audio_with_elevenlabs(
            audio_path,
            args.api_key,
            args.model_id,
            args.output_dir
        )
        
        if result:
            successful += 1
        else:
            failed += 1
    
    log(f"Transcription complete: {successful} successful, {failed} failed")

if __name__ == "__main__":
    main()