#!/usr/bin/env python3
"""
Multilingual Dubbing Pipeline

This script extends the German Rosary Generator to create a complete pipeline for:
1. Downloading YouTube videos
2. Transcribing audio with ElevenLabs
3. Cleaning up transcriptions with Claude
4. Translating to multiple languages
5. Generating dubbed audio in target languages
6. Creating videos with translated audio and configurable backgrounds
"""

import os
import json
import time
import requests
import subprocess
import re
import platform
import argparse
import tempfile
import shutil
import base64
from pathlib import Path
from datetime import datetime
from urllib.parse import urlparse, parse_qs
from typing import Dict, List, Optional, Tuple, Any, Union, Callable

# ======= CONFIGURATION =======
DEFAULT_CONFIG = {
    "elevenlabs": {
        "api_key": "YOUR_ELEVENLABS_API_KEY",
        "voice_ids": {
            "default": "DEFAULT_VOICE_ID",
            "en": "ENGLISH_VOICE_ID",
            "de": "GERMAN_VOICE_ID",
            "fr": "FRENCH_VOICE_ID",
            "es": "SPANISH_VOICE_ID",
            "it": "ITALIAN_VOICE_ID",
            "tr": "TURKISH_VOICE_ID",
            "el": "GREEK_VOICE_ID",
            "ru": "RUSSIAN_VOICE_ID",
            "hr": "CROATIAN_VOICE_ID",
            "bg": "BULGARIAN_VOICE_ID",
            "cs": "CZECH_VOICE_ID",
            "pt": "PORTUGUESE_VOICE_ID",
            "pl": "POLISH_VOICE_ID",
            "hu": "HUNGARIAN_VOICE_ID",
            "ar": "ARABIC_VOICE_ID",
            "fil": "FILIPINO_VOICE_ID",
            "ta": "TAMIL_VOICE_ID",
            "da": "DANISH_VOICE_ID",
            "nl": "DUTCH_VOICE_ID",
            "ja": "JAPANESE_VOICE_ID",
            "ko": "KOREAN_VOICE_ID",
            "zh": "CHINESE_VOICE_ID",
            "ro": "ROMANIAN_VOICE_ID",
            "sv": "SWEDISH_VOICE_ID",
            "uk": "UKRAINIAN_VOICE_ID"
        },
        "model_id": "eleven_multilingual_v2"
    },
    "claude": {
        "api_key": "YOUR_ANTHROPIC_API_KEY",
        "model": "claude-3-opus-20240229"
    },
    "video": {
        "width": 1280,
        "height": 720,
        "bg_color": "#2F2F2F",
        "text_color": "#FFFFFF",
        "highlight_color": "#FFCC66",
        "use_original_video": False,
        "watermark": "a dubia.cc production",
        "subtitles": {
            "enabled": False,
            "font_size": 30,
            "font_family": "Arial, sans-serif",
            "background_color": "rgba(0, 0, 0, 0.7)",
            "text_color": "#FFFFFF",
            "max_chars_per_line": 50,
            "padding": 15,
            "position": "bottom"  # Options: 'bottom', 'top', 'middle'
        }
    },
    "languages": [
        {"code": "de", "name": "German"},
        {"code": "fr", "name": "French"},
        {"code": "en", "name": "English"},
        {"code": "it", "name": "Italian"},
        {"code": "es", "name": "Spanish"},
        {"code": "tr", "name": "Turkish"},
        {"code": "el", "name": "Greek"},
        {"code": "ru", "name": "Russian"},
        {"code": "hr", "name": "Croatian"},
        {"code": "bg", "name": "Bulgarian"},
        {"code": "cs", "name": "Czech"},
        {"code": "pt", "name": "Portuguese"},
        {"code": "pl", "name": "Polish"},
        {"code": "hu", "name": "Hungarian"},
        {"code": "ar", "name": "Arabic"},
        {"code": "fil", "name": "Filipino"},
        {"code": "ta", "name": "Tamil"},
        {"code": "da", "name": "Danish"},
        {"code": "nl", "name": "Dutch"},
        {"code": "ja", "name": "Japanese"},
        {"code": "ko", "name": "Korean"},
        {"code": "zh", "name": "Chinese"},
        {"code": "ro", "name": "Romanian"},
        {"code": "sv", "name": "Swedish"},
        {"code": "uk", "name": "Ukrainian"}
    ]
}

# ======= UTILITY FUNCTIONS =======

def check_youtube_dl_installed() -> bool:
    """Check if youtube-dl is installed"""
    try:
        result = subprocess.run(['youtube-dl', '--version'], 
                               stdout=subprocess.PIPE, 
                               stderr=subprocess.PIPE,
                               text=True)
        return result.returncode == 0
    except FileNotFoundError:
        return False

def is_youtube_url(url: str) -> bool:
    """Check if the URL is a YouTube URL"""
    return 'youtube.com' in url or 'youtu.be' in url

def is_markdown_file(path: str) -> bool:
    """Check if the path is a markdown file"""
    return path.endswith('.md') or path.endswith('.markdown')

def load_config(config_path: Optional[str] = None) -> Dict[str, Any]:
    """Load configuration from JSON file or use defaults"""
    config = DEFAULT_CONFIG.copy()
    
    if config_path and os.path.exists(config_path):
        try:
            with open(config_path, 'r') as f:
                user_config = json.loads(f)
                
            # Deep merge the configurations
            for key, value in user_config.items():
                if isinstance(value, dict) and key in config and isinstance(config[key], dict):
                    config[key].update(value)
                else:
                    config[key] = value
                    
            return config
        except Exception as e:
            log(f"Error loading config from {config_path}: {e}")
            log("Using default configuration")
            return config
    
    return config

def log(message: str) -> None:
    """Timestamped logging function"""
    timestamp = datetime.now().strftime("%H:%M:%S")
    print(f"[{timestamp}] {message}")

def create_directories(dirs: List[str]) -> None:
    """Create necessary directories"""
    for directory in dirs:
        os.makedirs(directory, exist_ok=True)

def extract_video_id(url: str) -> Optional[str]:
    """Extract YouTube video ID from URL"""
    if not url:
        return None
        
    parsed_url = urlparse(url)
    
    if parsed_url.netloc in ('youtu.be', 'www.youtu.be'):
        return parsed_url.path[1:]
    
    if parsed_url.netloc in ('youtube.com', 'www.youtube.com'):
        if parsed_url.path == '/watch':
            query = parse_qs(parsed_url.query)
            return query.get('v', [None])[0]
        elif parsed_url.path.startswith('/embed/'):
            return parsed_url.path.split('/')[2]
        elif parsed_url.path.startswith('/v/'):
            return parsed_url.path.split('/')[2]
    
    return None

def slugify(text: str) -> str:
    """Convert text to a URL/filesystem-safe slug"""
    # Remove special characters and replace spaces with hyphens
    text = re.sub(r'[^\w\s-]', '', text.lower())
    return re.sub(r'[-\s]+', '-', text).strip('-')

def get_safe_filename(video_id: str, video_title: Optional[str] = None) -> str:
    """Generate a safe filename slug from video ID and optional title"""
    if video_title:
        # Combine ID and slugified title
        return f"{video_id}-{slugify(video_title)}"
    return video_id

def run_command(command: List[str], desc: str = "command") -> Tuple[bool, Optional[str], Optional[str]]:
    """Run a command and return success flag, stdout, and stderr"""
    log(f"Running {desc}: {' '.join(command)}")
    
    try:
        result = subprocess.run(command, check=True, capture_output=True, text=True)
        return True, result.stdout, result.stderr
    except subprocess.CalledProcessError as e:
        log(f"Error running {desc}: {e}")
        log(f"STDERR: {e.stderr}")
        return False, None, e.stderr

# ======= VIDEO DOWNLOAD =======

def download_youtube_video(url: str, output_dir: str) -> Optional[Dict[str, str]]:
    """Download YouTube video and return paths to video and audio files"""
    video_id = extract_video_id(url)
    if not video_id:
        log(f"Could not extract video ID from URL: {url}")
        return None
    
    os.makedirs(output_dir, exist_ok=True)
    
    # First, get video info to retrieve title
    info_cmd = [
        'youtube-dl', 
        '--dump-json',
        '--no-playlist',
        url
    ]
    
    success, info_json, _ = run_command(info_cmd, "youtube-dl info")
    if not success or not info_json:
        log("Failed to get video info")
        return None
    
    try:
        video_info = json.loads(info_json)
        video_title = video_info.get('title', '')
        
        # Create safe filename
        filename_base = get_safe_filename(video_id, video_title)
        video_output = os.path.join(output_dir, f"{filename_base}.mp4")
        audio_output = os.path.join(output_dir, f"{filename_base}.wav")
        
        # Download video
        video_cmd = [
            'youtube-dl',
            '-f', 'bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best',
            '--merge-output-format', 'mp4',
            '-o', video_output,
            '--no-playlist',
            url
        ]
        
        video_success, _, _ = run_command(video_cmd, "video download")
        if not video_success:
            log("Failed to download video")
            return None
        
        # Extract audio to WAV for transcription
        audio_cmd = [
            'ffmpeg', '-y', '-i', video_output, 
            '-vn', '-acodec', 'pcm_s16le', '-ar', '44100', '-ac', '2',
            audio_output
        ]
        
        audio_success, _, _ = run_command(audio_cmd, "audio extraction")
        if not audio_success:
            log("Failed to extract audio")
            return None
        
        return {
            'video_id': video_id,
            'title': video_title,
            'video_path': video_output,
            'audio_path': audio_output,
            'filename_base': filename_base
        }
        
    except json.JSONDecodeError:
        log("Failed to parse video info JSON")
        return None

# ======= TRANSCRIPTION =======

def get_audio_length(audio_path: str) -> Optional[float]:
    """Get length of audio file in seconds"""
    cmd = [
        'ffprobe', '-v', 'error', '-show_entries', 'format=duration',
        '-of', 'default=noprint_wrappers=1:nokey=1', audio_path
    ]
    
    success, stdout, _ = run_command(cmd, "audio length check")
    if success and stdout:
        try:
            return float(stdout.strip())
        except ValueError:
            return None
    return None

def split_audio(audio_path: str, output_dir: str, chunk_length: int = 600) -> List[str]:
    """Split audio into chunks of specified length in seconds"""
    audio_length = get_audio_length(audio_path)
    if not audio_length:
        log(f"Could not determine length of audio file: {audio_path}")
        return []
    
    chunks_dir = os.path.join(output_dir, "chunks")
    os.makedirs(chunks_dir, exist_ok=True)
    
    chunk_files = []
    
    # If audio is shorter than chunk_length, just use it directly
    if audio_length <= chunk_length:
        # Create a symbolic link or copy the file
        chunk_path = os.path.join(chunks_dir, f"chunk_001.wav")
        shutil.copy(audio_path, chunk_path)
        chunk_files.append(chunk_path)
        return chunk_files
    
    # Calculate number of chunks
    num_chunks = int(audio_length / chunk_length) + (1 if audio_length % chunk_length > 0 else 0)
    
    for i in range(num_chunks):
        start_time = i * chunk_length
        
        # Use shorter length for last chunk if needed
        if i == num_chunks - 1:
            duration = audio_length - start_time
        else:
            duration = chunk_length
        
        chunk_path = os.path.join(chunks_dir, f"chunk_{i+1:03d}.wav")
        
        cmd = [
            'ffmpeg', '-y', '-i', audio_path, 
            '-ss', str(start_time), '-t', str(duration),
            '-acodec', 'pcm_s16le', '-ar', '44100', '-ac', '2',
            chunk_path
        ]
        
        success, _, _ = run_command(cmd, f"audio split chunk {i+1}/{num_chunks}")
        if success:
            chunk_files.append(chunk_path)
    
    return chunk_files

def transcribe_audio_with_elevenlabs(
    audio_path: str, 
    api_key: str, 
    output_dir: str
) -> Optional[Dict[str, Any]]:
    """Transcribe audio using ElevenLabs Scribe API"""
    log(f"Transcribing audio file: {audio_path}")
    
    API_URL = "https://api.elevenlabs.io/v1/speech-to-text/detailed"
    
    headers = {
        "xi-api-key": api_key,
        "Accept": "application/json"
    }
    
    with open(audio_path, "rb") as f:
        files = {"audio_file": f}
        params = {"detect_chapters": "true"}
        
        try:
            log("Sending request to ElevenLabs Scribe API...")
            response = requests.post(API_URL, headers=headers, params=params, files=files)
            
            if response.status_code == 200:
                result = response.json()
                
                # Save raw JSON response
                json_path = os.path.join(output_dir, f"{os.path.basename(audio_path)}_transcription.json")
                with open(json_path, 'w') as f:
                    json.dump(result, f, indent=2)
                
                log(f"Transcription successful, saved to {json_path}")
                return result
            else:
                log(f"Transcription failed: {response.status_code} - {response.text}")
                return None
                
        except Exception as e:
            log(f"Exception during transcription: {str(e)}")
            return None

def process_transcription_segments(
    transcription: Dict[str, Any]
) -> Dict[str, Any]:
    """Process transcription segments into format suitable for processing"""
    if not transcription or "segments" not in transcription:
        return {"text": "", "segments": []}
    
    # Extract the full text
    full_text = transcription.get("text", "")
    
    # Process segments with start/end timestamps
    processed_segments = []
    for segment in transcription.get("segments", []):
        processed_segments.append({
            "text": segment.get("text", ""),
            "start": segment.get("start", 0),
            "end": segment.get("end", 0)
        })
    
    # Extract any chapter information
    chapters = transcription.get("chapters", [])
    
    return {
        "text": full_text,
        "segments": processed_segments,
        "chapters": chapters
    }

def combine_transcription_chunks(
    transcription_chunks: List[Dict[str, Any]], 
    chunk_length: int = 600
) -> Dict[str, Any]:
    """Combine multiple transcription chunks into a single transcription"""
    if not transcription_chunks:
        return {"text": "", "segments": [], "chapters": []}
    
    # If only one chunk, return it as is
    if len(transcription_chunks) == 1:
        return transcription_chunks[0]
    
    combined_text = ""
    combined_segments = []
    combined_chapters = []
    time_offset = 0
    
    for i, chunk in enumerate(transcription_chunks):
        # Add text with separator if not the first chunk
        if i > 0:
            combined_text += " "
        combined_text += chunk.get("text", "")
        
        # Adjust segment timestamps with offset
        for segment in chunk.get("segments", []):
            adjusted_segment = segment.copy()
            adjusted_segment["start"] += time_offset
            adjusted_segment["end"] += time_offset
            combined_segments.append(adjusted_segment)
        
        # Adjust chapter timestamps with offset
        for chapter in chunk.get("chapters", []):
            adjusted_chapter = chapter.copy()
            adjusted_chapter["start"] += time_offset
            adjusted_chapter["end"] += time_offset
            combined_chapters.append(adjusted_chapter)
        
        # Update time offset for next chunk
        time_offset += chunk_length
    
    return {
        "text": combined_text,
        "segments": combined_segments,
        "chapters": combined_chapters
    }

def full_transcription_pipeline(
    audio_path: str,
    api_key: str,
    output_dir: str,
    chunk_length: int = 600
) -> Optional[Dict[str, Any]]:
    """Run full transcription pipeline on audio file"""
    # Split audio into chunks if needed
    audio_chunks = split_audio(audio_path, output_dir, chunk_length)
    
    if not audio_chunks:
        log("Failed to split audio into chunks")
        return None
    
    log(f"Split audio into {len(audio_chunks)} chunks")
    
    # Transcribe each chunk
    transcription_chunks = []
    
    for i, chunk_path in enumerate(audio_chunks):
        log(f"Transcribing chunk {i+1}/{len(audio_chunks)}")
        transcription = transcribe_audio_with_elevenlabs(chunk_path, api_key, output_dir)
        
        if not transcription:
            log(f"Failed to transcribe chunk {i+1}")
            continue
            
        processed = process_transcription_segments(transcription)
        transcription_chunks.append(processed)
    
    if not transcription_chunks:
        log("No chunks were successfully transcribed")
        return None
    
    # Combine chunks if multiple
    if len(transcription_chunks) > 1:
        combined = combine_transcription_chunks(transcription_chunks, chunk_length)
    else:
        combined = transcription_chunks[0]
    
    # Save combined transcription
    combined_path = os.path.join(output_dir, "combined_transcription.json")
    with open(combined_path, 'w') as f:
        json.dump(combined, f, indent=2)
    
    log(f"Combined transcription saved to {combined_path}")
    
    return combined

# ======= CLAUDE PROCESSING =======

def format_claude_transcript_cleanup_prompt(
    transcription: Dict[str, Any]
) -> str:
    """Format a prompt for Claude to clean up the transcript"""
    full_text = transcription.get("text", "")
    
    # Calculate approximate length and segment if needed
    text_length = len(full_text)
    
    prompt = """
I have a transcription of speech that needs to be cleaned up. Please perform the following:
1. Fix any grammatical errors
2. Remove filler words (um, uh, like, you know, etc.)
3. Fix any obvious transcription errors
4. Maintain the original meaning and style
5. Keep paragraphs intact
6. Preserve any important terminology
7. DO NOT summarize or shorten the content

Here is the transcription:

"""
    prompt += full_text
    
    prompt += """

Please output ONLY the cleaned text without any explanations or comments.
"""
    
    return prompt

def format_claude_translation_prompt(
    text: str,
    source_language: str,
    target_language: str,
    target_language_name: str
) -> str:
    """Format a prompt for Claude to translate text"""
    prompt = f"""
You are a professional translator with deep expertise in {source_language} and {target_language_name}.

Please translate the following text from {source_language} to {target_language_name}.
Guidelines:
1. Maintain the original meaning, tone, and style
2. Preserve paragraph breaks and structure
3. Adapt idioms and cultural references appropriately
4. Use natural, fluent {target_language_name}
5. Preserve specialized terminology
6. DO NOT add or remove information
7. DO NOT explain your translation choices

Here is the text to translate:

{text}

Provide ONLY the translated text in {target_language_name} without any explanations or comments.
"""
    
    return prompt

def call_claude_api(
    prompt: str,
    api_key: str,
    model: str,
    max_tokens: int = 100000
) -> Optional[str]:
    """Call Claude API with a prompt and return the response"""
    API_URL = "https://api.anthropic.com/v1/messages"
    
    headers = {
        "anthropic-version": "2023-06-01",
        "content-type": "application/json",
        "x-api-key": api_key
    }
    
    data = {
        "model": model,
        "max_tokens": max_tokens,
        "messages": [
            {"role": "user", "content": prompt}
        ]
    }
    
    try:
        log(f"Calling Claude API with model {model}...")
        response = requests.post(API_URL, headers=headers, json=data)
        
        if response.status_code == 200:
            result = response.json()
            content = result.get("content", [])
            
            # Extract text from content blocks
            text_parts = []
            for block in content:
                if block.get("type") == "text":
                    text_parts.append(block.get("text", ""))
            
            return "".join(text_parts)
        else:
            log(f"Claude API call failed: {response.status_code} - {response.text}")
            return None
            
    except Exception as e:
        log(f"Exception during Claude API call: {str(e)}")
        return None

def parse_segmented_text(text: str) -> List[Dict[str, str]]:
    """Parse segmented text with numbering into a list of segments"""
    segments = []
    
    # Extract numbered segments
    pattern = r'(\d+)\.\s+(.*?)(?=\n\d+\.|\Z)'
    matches = re.finditer(pattern, text, re.DOTALL)
    
    for match in matches:
        segment_num = int(match.group(1))
        text = match.group(2).strip()
        segments.append({
            "id": segment_num,
            "text": text
        })
    
    # Sort by ID to ensure correct order
    segments.sort(key=lambda x: x["id"])
    
    return segments

def clean_transcript_chunks_with_claude(
    transcription: Dict[str, Any],
    api_key: str,
    model: str,
    output_dir: str
) -> List[Dict[str, Any]]:
    """Clean transcript chunks using Claude API"""
    # Extract segments from transcription
    segments = transcription.get("segments", [])
    if not segments:
        log("No segments found in transcription")
        return []
    
    # Format segments for Claude
    prompt = format_claude_chunk_cleanup_prompt(segments)
    
    # Call Claude API
    cleaned_text = call_claude_api(prompt, api_key, model)
    if not cleaned_text:
        log("Failed to clean transcript chunks with Claude")
        return []
    
    # Parse cleaned segments
    cleaned_segments = parse_segmented_text(cleaned_text)
    
    # Map back to original timing
    result_segments = []
    for i, segment in enumerate(segments):
        if i < len(cleaned_segments):
            result_segments.append({
                "id": i + 1,
                "text": cleaned_segments[i]["text"],
                "start": segment.get("start", 0),
                "end": segment.get("end", 0)
            })
        else:
            # If parsing missed some segments, use original
            result_segments.append({
                "id": i + 1,
                "text": segment.get("text", ""),
                "start": segment.get("start", 0),
                "end": segment.get("end", 0)
            })
    
    # Save cleaned segments
    cleaned_segments_path = os.path.join(output_dir, "cleaned_segments.json")
    with open(cleaned_segments_path, 'w', encoding='utf-8') as f:
        json.dump(result_segments, f, indent=2, ensure_ascii=False)
    
    log(f"Cleaned {len(result_segments)} segments, saved to {cleaned_segments_path}")
    
    return result_segments

def translate_segments_with_claude(
    segments: List[Dict[str, Any]],
    source_language: str,
    target_language_code: str,
    target_language_name: str,
    api_key: str,
    model: str,
    output_dir: str
) -> List[Dict[str, Any]]:
    """Translate segment chunks using Claude API"""
    # Format prompt for Claude
    prompt = format_claude_segment_translation_prompt(
        segments, source_language, target_language_code, target_language_name
    )
    
    # Call Claude API
    translated_text = call_claude_api(prompt, api_key, model)
    if not translated_text:
        log(f"Failed to translate segments to {target_language_name}")
        return []
    
    # Parse translated segments
    translated_segments = parse_segmented_text(translated_text)
    
    # Map back to original timing
    result_segments = []
    for i, segment in enumerate(segments):
        if i < len(translated_segments):
            result_segments.append({
                "id": i + 1,
                "original_text": segment.get("text", ""),
                "translated_text": translated_segments[i]["text"],
                "start": segment.get("start", 0),
                "end": segment.get("end", 0)
            })
        else:
            # If parsing missed some segments, mark as untranslated
            result_segments.append({
                "id": i + 1,
                "original_text": segment.get("text", ""),
                "translated_text": "[Translation missing]",
                "start": segment.get("start", 0),
                "end": segment.get("end", 0)
            })
    
    # Save translated segments
    translated_segments_path = os.path.join(output_dir, f"translated_segments_{target_language_code}.json")
    with open(translated_segments_path, 'w', encoding='utf-8') as f:
        json.dump(result_segments, f, indent=2, ensure_ascii=False)
    
    log(f"Translated {len(result_segments)} segments to {target_language_name}")
    
    return result_segments

def generate_video_summary_with_claude(
    text: str,
    api_key: str,
    model: str,
    output_dir: str
) -> Optional[Dict[str, str]]:
    """Generate video summary and title with Claude"""
    prompt = f"""
Based on the following transcript, please generate:
1. A compelling video title (max 100 characters)
2. A concise video description (3-5 sentences)
3. A list of 10 relevant keywords or tags separated by commas

Transcript:
{text}

Please format your response in JSON with keys "title", "description", and "tags".
"""
    
    response = call_claude_api(prompt, api_key, model)
    if not response:
        log("Failed to generate video summary")
        return None
    
    try:
        # Extract JSON from the response
        json_match = re.search(r'```json\s*(.*?)\s*```', response, re.DOTALL)
        if json_match:
            json_str = json_match.group(1)
        else:
            json_str = response
        
        summary = json.loads(json_str)
        
        # Save summary
        summary_path = os.path.join(output_dir, "video_summary.json")
        with open(summary_path, 'w') as f:
            json.dump(summary, f, indent=2)
        
        log(f"Video summary saved to {summary_path}")
        
        return summary
        
    except json.JSONDecodeError as e:
        log(f"Failed to parse summary JSON: {e}")
        log(f"Raw response: {response}")
        return None

# ======= TEXT TO SPEECH =======

# ======= SEGMENTED TTS PROCESSING =======

def generate_segment_audio(
    segment: Dict[str, Any],
    voice_id: str,
    api_key: str,
    model_id: str,
    output_path: str
) -> bool:
    """Generate audio for a single segment"""
    return text_to_speech_with_elevenlabs(
        segment["translated_text"],
        voice_id,
        api_key,
        model_id,
        output_path
    )

def generate_segment_subtitle(
    segment: Dict[str, Any],
    output_html_path: str,
    output_png_path: str,
    subtitle_config: Dict[str, Any],
    width: int = 1280,
    height: int = 720
) -> bool:
    """Generate subtitle image for a single segment"""
    # Create HTML for subtitle
    html_content = create_subtitle_html(
        segment["translated_text"],
        subtitle_config.get("font_size", 30),
        subtitle_config.get("font_family", "Arial, sans-serif"),
        subtitle_config.get("background_color", "rgba(0, 0, 0, 0.7)"),
        subtitle_config.get("text_color", "#FFFFFF"),
        subtitle_config.get("padding", 15),
        width,
        height,
        subtitle_config.get("position", "bottom")
    )
    
    # Save HTML
    with open(output_html_path, 'w', encoding='utf-8') as f:
        f.write(html_content)
    
    # Convert to PNG using Chrome
    abs_html_path = os.path.abspath(output_html_path)
    abs_png_path = os.path.abspath(output_png_path)
    
    # Find Chrome executable
    chrome_cmd = None
    system = platform.system()
    
    if system == "Darwin":  # macOS
        mac_chrome_paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "/Applications/Chromium.app/Contents/MacOS/Chromium"
        ]
        
        for path in mac_chrome_paths:
            if os.path.exists(path):
                chrome_cmd = path
                break
    
    if not chrome_cmd:
        chrome_cmd = 'google-chrome'
    
    command = [
        chrome_cmd, '--headless', '--disable-gpu',
        '--window-size=' + str(width) + ',' + str(height),
        '--screenshot=' + abs_png_path,
        '--background-color=transparent',
        'file://' + abs_html_path
    ]
    
    success, _, _ = run_command(command, f"subtitle screenshot for segment {segment['id']}")
    return success

def process_segment(
    segment: Dict[str, Any],
    lang_code: str,
    voice_id: str,
    elevenlabs_config: Dict[str, Any],
    video_config: Dict[str, Any],
    build_dir: str
) -> Dict[str, Any]:
    """Process a single segment, generating audio and subtitle files"""
    segment_id = segment["id"]
    segment_id_str = f"{segment_id:04d}"
    
    result = {
        "id": segment_id,
        "start": segment["start"],
        "end": segment["end"],
        "text": segment["translated_text"]
    }
    
    # Generate audio file
    audio_path = os.path.join(build_dir, f"{segment_id_str}.mp3")
    audio_success = generate_segment_audio(
        segment,
        voice_id,
        elevenlabs_config["api_key"],
        elevenlabs_config["model_id"],
        audio_path
    )
    
    if audio_success:
        result["audio_path"] = audio_path
        
        # Get audio duration
        audio_duration = get_audio_length(audio_path)
        if audio_duration:
            result["audio_duration"] = audio_duration
    
    # Generate subtitle files
    html_path = os.path.join(build_dir, f"{segment_id_str}.subtitle.html")
    png_path = os.path.join(build_dir, f"{segment_id_str}.subtitle.png")
    
    subtitle_success = generate_segment_subtitle(
        segment,
        html_path,
        png_path,
        video_config["subtitles"],
        video_config["width"],
        video_config["height"]
    )
    
    if subtitle_success:
        result["subtitle_html_path"] = html_path
        result["subtitle_png_path"] = png_path
    
    return result

def process_all_segments(
    segments: List[Dict[str, Any]],
    lang_code: str,
    voice_id: str,
    elevenlabs_config: Dict[str, Any],
    video_config: Dict[str, Any],
    output_dir: str
) -> List[Dict[str, Any]]:
    """Process all segments and generate a configuration file"""
    # Create build directory
    build_dir = os.path.join(output_dir, "build")
    os.makedirs(build_dir, exist_ok=True)
    
    processed_segments = []
    
    for i, segment in enumerate(segments):
        log(f"Processing segment {i+1}/{len(segments)} for {lang_code}...")
        
        result = process_segment(
            segment,
            lang_code,
            voice_id,
            elevenlabs_config,
            video_config,
            build_dir
        )
        
        processed_segments.append(result)
    
    # Create configuration file
    config_path = os.path.join(output_dir, "segments_config.json")
    with open(config_path, 'w', encoding='utf-8') as f:
        json.dump(processed_segments, f, indent=2, ensure_ascii=False)
    
    log(f"Processed {len(processed_segments)} segments, config saved to {config_path}")
    
    return processed_segments

# ======= SUBTITLE GENERATION =======

def extract_chunks_for_subtitles(
    transcription: Dict[str, Any],
    max_chars_per_line: int = 50
) -> List[Dict[str, Any]]:
    """Extract text chunks for subtitles from transcription segments"""
    subtitle_chunks = []
    
    if not transcription or "segments" not in transcription:
        log("No segments found in transcription for subtitle generation")
        return subtitle_chunks
    
    for segment in transcription["segments"]:
        text = segment.get("text", "").strip()
        if not text:
            continue
            
        start_time = float(segment.get("start", 0))
        end_time = float(segment.get("end", 0))
        
        # Skip if duration is too short
        if end_time - start_time < 0.3:
            continue
        
        # Split long segments into multiple lines
        if len(text) > max_chars_per_line:
            words = text.split()
            lines = []
            current_line = []
            current_length = 0
            
            for word in words:
                word_length = len(word)
                if current_length + word_length + len(current_line) <= max_chars_per_line:
                    current_line.append(word)
                    current_length += word_length
                else:
                    lines.append(" ".join(current_line))
                    current_line = [word]
                    current_length = word_length
            
            if current_line:
                lines.append(" ".join(current_line))
                
            # For multi-line subtitles, use the same timing
            subtitle_chunks.append({
                "text": "\n".join(lines),
                "start": start_time,
                "end": end_time
            })
        else:
            subtitle_chunks.append({
                "text": text,
                "start": start_time,
                "end": end_time
            })
    
    return subtitle_chunks

def format_claude_chunk_translation_prompt(
    chunks: List[Dict[str, Any]],
    source_language: str,
    target_language_code: str,
    target_language_name: str
) -> str:
    """Format a prompt for Claude to translate subtitle chunks"""
    chunk_texts = []
    for i, chunk in enumerate(chunks):
        chunk_texts.append(f"{i+1}. {chunk['text']}")
    
    source_text = "\n\n".join(chunk_texts)
    
    prompt = f"""
You are a professional translator with deep expertise in {source_language} and {target_language_name}.

Please translate the following numbered text segments from {source_language} to {target_language_name}.
Guidelines:
1. Maintain the original meaning and tone
2. Keep translations concise and suitable for subtitles
3. Preserve the numbered format exactly (1., 2., etc.)
4. Translate each segment independently
5. DO NOT add or remove information
6. DO NOT add any explanations or comments

Here are the text segments to translate:

{source_text}

Provide ONLY the translated text segments with their original numbers in {target_language_name}.
"""
    
    return prompt

def parse_translated_chunks(
    translated_text: str,
    original_chunks: List[Dict[str, Any]]
) -> List[Dict[str, Any]]:
    """Parse translated chunks from Claude's response"""
    translated_chunks = []
    
    # Extract numbered lines using regex
    pattern = r'(\d+)\.\s+(.*?)(?=\n\d+\.|\Z)'
    matches = re.finditer(pattern, translated_text, re.DOTALL)
    
    translations_dict = {}
    for match in matches:
        chunk_num = int(match.group(1))
        text = match.group(2).strip()
        translations_dict[chunk_num] = text
    
    # Map back to original chunks with timing
    for i, original in enumerate(original_chunks):
        chunk_num = i + 1
        if chunk_num in translations_dict:
            translated_chunks.append({
                "text": translations_dict[chunk_num],
                "start": original["start"],
                "end": original["end"]
            })
        else:
            # If translation missing, use original (shouldn't happen)
            log(f"Warning: Missing translation for chunk {chunk_num}")
            translated_chunks.append(original.copy())
    
    return translated_chunks

def create_subtitle_html(
    text: str,
    font_size: int = 30,
    font_family: str = "Arial, sans-serif",
    background_color: str = "rgba(0, 0, 0, 0.7)",
    text_color: str = "#FFFFFF",
    padding: int = 15,
    width: int = 1280,
    height: int = 720,
    position: str = "bottom"
) -> str:
    """Create HTML for subtitle text"""
    position_style = ""
    if position == "bottom":
        position_style = "bottom: 50px;"
    elif position == "top":
        position_style = "top: 50px;"
    elif position == "middle":
        position_style = "top: 50%; transform: translateY(-50%);"
    
    html = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{
            margin: 0;
            padding: 0;
            width: {width}px;
            height: {height}px;
            background-color: transparent;
            overflow: hidden;
        }}
        .subtitle {{
            position: absolute;
            left: 50%;
            transform: translateX(-50%);
            {position_style}
            text-align: center;
            font-family: {font_family};
            font-size: {font_size}px;
            color: {text_color};
            background-color: {background_color};
            padding: {padding}px;
            border-radius: 10px;
            max-width: 80%;
            line-height: 1.4;
            white-space: pre-wrap;
        }}
    </style>
</head>
<body>
    <div class="subtitle">{text}</div>
</body>
</html>
"""
    return html

def create_subtitle_image(
    text: str,
    output_path: str,
    font_size: int = 30,
    font_family: str = "Arial, sans-serif",
    background_color: str = "rgba(0, 0, 0, 0.7)",
    text_color: str = "#FFFFFF",
    padding: int = 15,
    width: int = 1280,
    height: int = 720,
    position: str = "bottom"
) -> bool:
    """Create a subtitle image using Chrome"""
    # Create temporary HTML file
    temp_html_file = os.path.join(os.path.dirname(output_path), f"temp_subtitle_{os.path.basename(output_path)}.html")
    
    html_content = create_subtitle_html(
        text, font_size, font_family, background_color, text_color, 
        padding, width, height, position
    )
    
    try:
        with open(temp_html_file, 'w', encoding='utf-8') as f:
            f.write(html_content)
        
        # Convert HTML to PNG using Chrome
        abs_html_path = os.path.abspath(temp_html_file)
        abs_output_path = os.path.abspath(output_path)
        
        # Find Chrome executable
        chrome_cmd = None
        system = platform.system()
        
        if system == "Darwin":  # macOS
            mac_chrome_paths = [
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
                "/Applications/Chromium.app/Contents/MacOS/Chromium"
            ]
            
            for path in mac_chrome_paths:
                if os.path.exists(path):
                    chrome_cmd = path
                    break
        
        if not chrome_cmd:
            chrome_cmd = 'google-chrome'
        
        command = [
            chrome_cmd, '--headless', '--disable-gpu',
            '--window-size=' + str(width) + ',' + str(height),
            '--screenshot=' + abs_output_path,
            '--background-color=transparent',
            'file://' + abs_html_path
        ]
        
        success, _, _ = run_command(command, "subtitle screenshot")
        return success
        
    except Exception as e:
        log(f"Error creating subtitle image: {e}")
        return False
    finally:
        # Clean up temp HTML file
        if os.path.exists(temp_html_file):
            os.unlink(temp_html_file)

def generate_subtitle_images_for_chunks(
    chunks: List[Dict[str, Any]],
    output_dir: str,
    subtitle_config: Dict[str, Any],
    width: int = 1280,
    height: int = 720
) -> Dict[str, str]:
    """Generate subtitle images for all chunks"""
    subtitle_images = {}
    os.makedirs(output_dir, exist_ok=True)
    
    for i, chunk in enumerate(chunks):
        output_path = os.path.join(output_dir, f"subtitle_{i+1:03d}.png")
        
        success = create_subtitle_image(
            chunk["text"],
            output_path,
            subtitle_config.get("font_size", 30),
            subtitle_config.get("font_family", "Arial, sans-serif"),
            subtitle_config.get("background_color", "rgba(0, 0, 0, 0.7)"),
            subtitle_config.get("text_color", "#FFFFFF"),
            subtitle_config.get("padding", 15),
            width,
            height,
            subtitle_config.get("position", "bottom")
        )
        
        if success:
            subtitle_images[f"{chunk['start']}-{chunk['end']}"] = output_path
    
    return subtitle_images

def create_subtitle_filter_complex(
    chunks: List[Dict[str, Any]],
    subtitle_images: Dict[str, str]
) -> str:
    """Create ffmpeg filter complex for overlaying subtitles"""
    filters = []
    
    for i, chunk in enumerate(chunks):
        key = f"{chunk['start']}-{chunk['end']}"
        if key in subtitle_images:
            image_path = subtitle_images[key]
            start_time = chunk["start"]
            end_time = chunk["end"]
            duration = end_time - start_time
            
            # Add filter to overlay subtitle at the right time
            # Format: [previous];[in][subtitle]overlay=0:0:enable='between(t,start,end)'[out]
            if i == 0:
                input_ref = "0:v"
            else:
                input_ref = f"sub{i-1}"
            
            filters.append(f"[{input_ref}][s{i}]overlay=0:0:enable='between(t,{start_time},{end_time})'[sub{i}]")
    
    return ";".join(filters)

# ======= VIDEO GENERATION =======

def extract_still_frame(
    video_path: str,
    output_path: str,
    time_position: str = "00:00:30"
) -> bool:
    """Extract a still frame from the video"""
    cmd = [
        'ffmpeg', '-y', '-i', video_path,
        '-ss', time_position, '-frames:v', '1',
        output_path
    ]
    
    success, _, _ = run_command(cmd, "frame extraction")
    return success

def create_video_with_audio(
    audio_path: str,
    background_path: str,
    output_path: str,
    is_image: bool = True,
    watermark_text: Optional[str] = None,
    width: int = 1280,
    height: int = 720,
    subtitle_chunks: Optional[List[Dict[str, Any]]] = None,
    subtitle_images: Optional[Dict[str, str]] = None
) -> bool:
    """Create video from audio and background (image or video)"""
    # Get audio duration
    audio_duration = get_audio_length(audio_path)
    if not audio_duration:
        log(f"Could not determine length of audio file: {audio_path}")
        return False
    
    ffmpeg_cmd = ['ffmpeg', '-y']
    
    if is_image:
        # If using an image background
        ffmpeg_cmd.extend(['-loop', '1', '-i', background_path])
    else:
        # If using a video background
        ffmpeg_cmd.extend(['-i', background_path])
    
    # Add audio input
    ffmpeg_cmd.extend(['-i', audio_path])
    
    # Add subtitle images as inputs if provided
    if subtitle_chunks and subtitle_images:
        for i, chunk in enumerate(subtitle_chunks):
            key = f"{chunk['start']}-{chunk['end']}"
            if key in subtitle_images:
                ffmpeg_cmd.extend(['-i', subtitle_images[key]])
    
    # Prepare filter complex
    filter_complex = []
    
    # Add watermark text if specified
    if watermark_text:
        if subtitle_chunks and subtitle_images:
            # We'll add the watermark after subtitles
            pass
        else:
            filter_complex.append(
                f"drawtext=text='{watermark_text}':fontcolor=white:fontsize=24:"
                f"box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)-20:y=(h-text_h)-20"
            )
    
    # Add subtitle overlays if provided
    if subtitle_chunks and subtitle_images:
        # Create a chain of overlay filters
        last_output = "0:v"  # Start with main video
        
        for i, chunk in enumerate(subtitle_chunks):
            key = f"{chunk['start']}-{chunk['end']}"
            if key in subtitle_images:
                input_idx = i + 2  # First inputs are video and audio
                start_time = chunk["start"]
                end_time = chunk["end"]
                
                # Add overlay filter
                filter_complex.append(
                    f"[{last_output}][{input_idx}:v]overlay=0:0:enable='between(t,{start_time},{end_time})'[v{i}]"
                )
                last_output = f"v{i}"
        
        # Add watermark to the final output if needed
        if watermark_text:
            filter_complex.append(
                f"[{last_output}]drawtext=text='{watermark_text}':fontcolor=white:fontsize=24:"
                f"box=1:boxcolor=black@0.5:boxborderw=5:x=(w-text_w)-20:y=(h-text_h)-20[vout]"
            )
            last_output = "vout"
        
        # Map the last output to video output
        ffmpeg_cmd.extend(['-filter_complex', ';'.join(filter_complex)])
        ffmpeg_cmd.extend(['-map', f'[{last_output}]', '-map', '1:a'])
        
    elif filter_complex:
        # Simple case with just watermark
        ffmpeg_cmd.extend(['-vf', ','.join(filter_complex)])
    
    # Add output options
    ffmpeg_cmd.extend([
        '-c:v', 'libx264', '-preset', 'medium', '-crf', '22',
        '-c:a', 'aac', '-b:a', '192k',
        '-shortest', '-pix_fmt', 'yuv420p',
        output_path
    ])
    
    success, _, _ = run_command(ffmpeg_cmd, "video creation")
    return success

def create_intro_outro_videos(
    output_dir: str,
    watermark_text: str,
    width: int = 1280,
    height: int = 720,
    bg_color: str = "#2F2F2F",
    text_color: str = "#FFFFFF"
) -> Dict[str, str]:
    """Create intro and outro videos"""
    results = {}
    
    # Create intro HTML
    intro_html = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{
            background-color: {bg_color};
            margin: 0;
            padding: 0;
            width: {width}px;
            height: {height}px;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            text-align: center;
            font-family: Arial, sans-serif;
        }}
        .logo {{
            font-size: 48px;
            color: {text_color};
            margin-bottom: 40px;
        }}
        .watermark {{
            font-size: 24px;
            color: {text_color};
            margin-top: 40px;
            font-style: italic;
        }}
    </style>
</head>
<body>
    <div class="logo">Multilingual Dubbing</div>
    <div class="watermark">{watermark_text}</div>
</body>
</html>
"""
    
    intro_html_path = os.path.join(output_dir, "intro.html")
    with open(intro_html_path, 'w') as f:
        f.write(intro_html)
    
    # Create outro HTML (similar to intro)
    outro_html = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{
            background-color: {bg_color};
            margin: 0;
            padding: 0;
            width: {width}px;
            height: {height}px;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            text-align: center;
            font-family: Arial, sans-serif;
        }}
        .message {{
            font-size: 48px;
            color: {text_color};
            margin-bottom: 40px;
        }}
        .watermark {{
            font-size: 24px;
            color: {text_color};
            margin-top: 40px;
            font-style: italic;
        }}
    </style>
</head>
<body>
    <div class="message">Thank You For Watching</div>
    <div class="watermark">{watermark_text}</div>
</body>
</html>
"""
    
    outro_html_path = os.path.join(output_dir, "outro.html")
    with open(outro_html_path, 'w') as f:
        f.write(outro_html)
    
    # Convert HTML to PNG
    intro_png_path = os.path.join(output_dir, "intro.png")
    outro_png_path = os.path.join(output_dir, "outro.png")
    
    # Use Chrome headless if available
    def try_html_to_png(html_path, png_path):
        try:
            # Construct absolute paths for Chrome to use
            abs_html_path = os.path.abspath(html_path)
            abs_png_path = os.path.abspath(png_path)
            
            chrome_cmd = None
            system = platform.system()
            
            if system == "Darwin":  # macOS
                mac_chrome_paths = [
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                    "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
                    "/Applications/Chromium.app/Contents/MacOS/Chromium"
                ]
                
                for path in mac_chrome_paths:
                    if os.path.exists(path):
                        chrome_cmd = path
                        break
            
            if not chrome_cmd:
                chrome_cmd = 'google-chrome'
            
            command = [
                chrome_cmd, '--headless', '--disable-gpu',
                f'--window-size={width},{height}',
                f'--screenshot={abs_png_path}',
                f'file://{abs_html_path}'
            ]
            
            return run_command(command, "Chrome screenshot")[0]
            
        except Exception as e:
            log(f"Error converting HTML to PNG: {e}")
            return False
    
    intro_success = try_html_to_png(intro_html_path, intro_png_path)
    outro_success = try_html_to_png(outro_html_path, outro_png_path)
    
    # Create videos from images
    if intro_success:
        intro_video_path = os.path.join(output_dir, "intro.mp4")
        cmd = [
            'ffmpeg', '-y', '-loop', '1', '-i', intro_png_path, 
            '-c:v', 'libx264', '-t', '3', '-pix_fmt', 'yuv420p',
            '-vf', 'fade=in:0:30',
            intro_video_path
        ]
        
        if run_command(cmd, "intro video creation")[0]:
            results['intro'] = intro_video_path
    
    if outro_success:
        outro_video_path = os.path.join(output_dir, "outro.mp4")
        cmd = [
            'ffmpeg', '-y', '-loop', '1', '-i', outro_png_path, 
            '-c:v', 'libx264', '-t', '3', '-pix_fmt', 'yuv420p',
            '-vf', 'fade=out:60:30',
            outro_video_path
        ]
        
        if run_command(cmd, "outro video creation")[0]:
            results['outro'] = outro_video_path
    
    return results

def concatenate_videos(
    video_paths: List[str],
    output_path: str
) -> bool:
    """Concatenate multiple videos into one"""
    # Create a temporary file list
    list_file = tempfile.NamedTemporaryFile(mode='w', delete=False, suffix='.txt')
    
    try:
        # Write file paths to list file
        for path in video_paths:
            if os.path.exists(path):
                list_file.write(f"file '{os.path.abspath(path)}'\n")
        
        list_file.close()
        
        # Concatenate videos
        cmd = [
            'ffmpeg', '-y', '-f', 'concat', '-safe', '0',
            '-i', list_file.name, '-c', 'copy', output_path
        ]
        
        success, _, _ = run_command(cmd, "video concatenation")
        return success
    
    finally:
        # Clean up temp file
        if os.path.exists(list_file.name):
            os.unlink(list_file.name)

# ======= MAIN PIPELINE =======

def process_language(
    cleaned_segments: List[Dict[str, Any]],
    source_language: str,
    language_config: Dict[str, str],
    elevenlabs_config: Dict[str, Any],
    claude_config: Dict[str, Any],
    video_config: Dict[str, Any],
    background_path: str,
    output_base_dir: str,
    video_id: str,
    intro_outro_videos: Dict[str, str] = None
) -> Dict[str, Any]:
    """Process a single target language using segmented approach"""
    lang_code = language_config["code"]
    lang_name = language_config["name"]
    
    log(f"Processing {lang_name} translation...")
    
    # Create language-specific output directory
    lang_dir = os.path.join(output_base_dir, lang_code)
    os.makedirs(lang_dir, exist_ok=True)
    
    # Get appropriate voice ID for this language
    voice_id = elevenlabs_config["voice_ids"].get(lang_code)
    if not voice_id:
        log(f"No voice configured for {lang_name}, using default")
        voice_id = elevenlabs_config["voice_ids"].get("default")
    
    # Translate segments with Claude
    translated_segments = translate_segments_with_claude(
        cleaned_segments,
        source_language,
        lang_code,
        lang_name,
        claude_config["api_key"],
        claude_config["model"],
        lang_dir
    )
    
    if not translated_segments:
        log(f"Failed to translate segments to {lang_name}")
        return {"success": False, "error": "Translation failed"}
    
    # Process all segments (generate audio and subtitles)
    processed_segments = process_all_segments(
        translated_segments,
        lang_code,
        voice_id,
        elevenlabs_config,
        video_config,
        lang_dir
    )
    
    if not processed_segments:
        log(f"Failed to process segments for {lang_name}")
        return {"success": False, "error": "Segment processing failed"}
    
    # Generate final video with all segments
    video_path = os.path.join(lang_dir, f"{video_id}_{lang_code}.mp4")
    
    video_result = create_segmented_video(
        processed_segments,
        background_path,
        video_path,
        not video_config["use_original_video"],
        video_config["watermark"],
        video_config["width"],
        video_config["height"]
    )
    
    if not video_result:
        log(f"Failed to create final video for {lang_name}")
        return {"success": False, "error": "Video creation failed"}
    
    # Generate video summary and title from all segments
    all_text = " ".join([segment["text"] for segment in processed_segments])
    summary = generate_video_summary_with_claude(
        all_text,
        claude_config["api_key"],
        claude_config["model"],
        lang_dir
    )
    
    # Add intro and outro if available
    if intro_outro_videos and 'intro' in intro_outro_videos and 'outro' in intro_outro_videos:
        final_video_path = os.path.join(lang_dir, f"{video_id}_{lang_code}_complete.mp4")
        
        concat_success = concatenate_videos(
            [intro_outro_videos['intro'], video_path, intro_outro_videos['outro']],
            final_video_path
        )
        
        if concat_success:
            log(f"Created complete video for {lang_name}: {final_video_path}")
            return {
                "success": True,
                "language": lang_name,
                "language_code": lang_code,
                "video_path": final_video_path,
                "segments_config": os.path.join(lang_dir, "segments_config.json"),
                "segment_count": len(processed_segments)
            }
    
    log(f"Created video for {lang_name}: {video_path}")
    return {
        "success": True,
        "language": lang_name,
        "language_code": lang_code,
        "video_path": video_path,
        "segments_config": os.path.join(lang_dir, "segments_config.json"),
        "segment_count": len(processed_segments)
    }

def process_markdown_file(
    markdown_path: str,
    output_dir: str
) -> Dict[str, Any]:
    """Process a markdown file and return content for translation"""
    try:
        with open(markdown_path, 'r', encoding='utf-8') as f:
            content = f.read()
            
        # Extract basic info from markdown file
        filename = os.path.basename(markdown_path)
        file_id = os.path.splitext(filename)[0]
        
        # Try to extract a title from the markdown
        title_match = re.search(r'^#\s+(.*?)$', content, re.MULTILINE)
        title = title_match.group(1) if title_match else file_id
        
        # Create a placeholder for transcription with timing
        # This is to maintain compatibility with the rest of the pipeline
        # We'll create evenly spaced segments for subtitle generation
        segments = []
        paragraphs = [p.strip() for p in content.split('\n\n')]
        
        # Filter out empty paragraphs
        paragraphs = [p for p in paragraphs if p and not p.startswith('#')]
        
        # Create segments for each paragraph
        current_time = 0
        for i, paragraph in enumerate(paragraphs):
            # Estimate segment duration based on length (adjust as needed)
            segment_duration = max(2, len(paragraph) * 0.05)  # Min 2 seconds
            
            segments.append({
                "id": i + 1,
                "text": paragraph,
                "start": current_time,
                "end": current_time + segment_duration
            })
            
            current_time += segment_duration
        
        # Create a transcription-like structure
        transcription = {
            "text": content,
            "segments": segments
        }
        
        # Save as JSON for consistency
        json_path = os.path.join(output_dir, f"{file_id}_transcription.json")
        with open(json_path, 'w', encoding='utf-8') as f:
            json.dump(transcription, f, indent=2)
        
        return {
            "file_id": file_id,
            "title": title,
            "content": content,
            "transcription": transcription,
            "path": markdown_path
        }
        
    except Exception as e:
        log(f"Error processing markdown file: {e}")
        return {}

# ======= COMMAND LINE INTERFACE =======

def parse_arguments():
    """Parse command line arguments"""
    parser = argparse.ArgumentParser(description="Multilingual Dubbing Pipeline")
    
    parser.add_argument(
        "--input", "-i",
        required=True,
        help="Input source: YouTube video URL or path to markdown file"
    )
    
    parser.add_argument(
        "--config", "-c",
        help="Path to JSON configuration file"
    )
    
    parser.add_argument(
        "--elevenlabs-key", "-e",
        help="ElevenLabs API key (overrides config file)"
    )
    
    parser.add_argument(
        "--claude-key", "-a",
        help="Anthropic Claude API key (overrides config file)"
    )
    
    parser.add_argument(
        "--source-language", "-s",
        default="English",
        help="Source language of the video (default: English)"
    )
    
    parser.add_argument(
        "--target-languages", "-t",
        nargs="+",
        help="List of target language codes to process (default: all configured languages)"
    )
    
    parser.add_argument(
        "--use-original-video", "-v",
        action="store_true",
        help="Use the original video as background instead of a still image"
    )
    
    parser.add_argument(
        "--add-subtitles", "-sub",
        action="store_true",
        help="Add subtitles to the videos"
    )
    
    parser.add_argument(
        "--subtitle-size", "-ss",
        type=int,
        default=30,
        help="Font size for subtitles (default: 30)"
    )
    
    return parser.parse_args()

def multilingual_dubbing_pipeline(
    input_path: str,
    config: Dict[str, Any],
    source_language: str = "English",
    target_languages: Optional[List[str]] = None
) -> Dict[str, Any]:
    """Run the complete multilingual dubbing pipeline with segment-based processing"""
    # 1. Create base directories
    base_dir = "multilingual_dubbing_output"
    temp_dir = os.path.join(base_dir, "temp")
    
    create_directories([base_dir, temp_dir])
    
    # 2. Determine the input type and process accordingly
    is_youtube = is_youtube_url(input_path)
    is_markdown = is_markdown_file(input_path)
    
    video_id = None
    video_title = None
    video_path = None
    audio_path = None
    filename_base = None
    transcription = None
    content = None
    
    # Process YouTube video
    if is_youtube:
        log("Detected YouTube URL as input")
        
        # Check if youtube-dl is installed
        if not check_youtube_dl_installed():
            log("Error: youtube-dl is not installed. Please install it first.")
            return {"success": False, "error": "youtube-dl not installed"}
        
        log("Step 1: Downloading YouTube video...")
        video_info = download_youtube_video(input_path, temp_dir)
        
        if not video_info:
            log("Failed to download video, aborting")
            return {"success": False, "error": "Video download failed"}
        
        video_id = video_info["video_id"]
        video_title = video_info["title"]
        video_path = video_info["video_path"]
        audio_path = video_info["audio_path"]
        filename_base = video_info["filename_base"]
        
        log(f"Downloaded video: {video_title} ({video_id})")
        
        # 3. Transcribe audio
        log("Step 2: Transcribing audio...")
        transcription = full_transcription_pipeline(
            audio_path,
            config["elevenlabs"]["api_key"],
            temp_dir
        )
        
        if not transcription:
            log("Failed to transcribe audio, aborting")
            return {"success": False, "error": "Transcription failed"}
        
    # Process markdown file
    elif is_markdown:
        log("Detected markdown file as input")
        
        log("Step 1: Processing markdown file...")
        markdown_info = process_markdown_file(input_path, temp_dir)
        
        if not markdown_info:
            log("Failed to process markdown file, aborting")
            return {"success": False, "error": "Markdown processing failed"}
        
        video_id = markdown_info["file_id"]
        video_title = markdown_info["title"]
        content = markdown_info["content"]
        transcription = markdown_info["transcription"]
        filename_base = markdown_info["file_id"]
        
        log(f"Processed markdown file: {video_title} ({video_id})")
        
    else:
        log(f"Unsupported input format: {input_path}")
        return {"success": False, "error": "Unsupported input format"}
    
    # 4. Clean up transcript segments with Claude
    log("Step 3: Cleaning up transcript segments...")
    
    cleaned_segments = clean_transcript_chunks_with_claude(
        transcription,
        config["claude"]["api_key"],
        config["claude"]["model"],
        temp_dir
    )
    
    if not cleaned_segments:
        log("Failed to clean transcript segments, aborting")
        return {"success": False, "error": "Transcript cleanup failed"}
    
    # 5. Prepare background
    log("Step 4: Preparing background...")
    
    background_path = ""
    
    if is_youtube and config["video"]["use_original_video"]:
        background_path = video_path
    elif is_youtube:
        # Extract a frame from the middle of the video
        background_path = os.path.join(temp_dir, f"{filename_base}_background.jpg")
        
        # Calculate middle frame position
        video_length = get_audio_length(video_path)
        if video_length:
            middle_position = str(int(video_length / 2))
            extract_still_frame(video_path, background_path, middle_position)
        else:
            # Default to 30 seconds in if length can't be determined
            extract_still_frame(video_path, background_path, "00:00:30")
    else:
        # For markdown, create a solid color background
        background_path = os.path.join(temp_dir, f"{filename_base}_background.jpg")
        
        # Create a solid color background using ffmpeg
        cmd = [
            'ffmpeg', '-y', '-f', 'lavfi', '-i', 
            f"color=c={config['video']['bg_color'].replace('#', '0x')}:s={config['video']['width']}x{config['video']['height']}", 
            '-frames:v', '1', background_path
        ]
        
        success, _, _ = run_command(cmd, "background creation")
        if not success:
            log("Failed to create background image, using fallback")
            # Create a fallback background using HTML and Chrome
            bg_html = f"""<!DOCTYPE html>
<html>
<head>
    <style>
        body {{
            margin: 0;
            padding: 0;
            width: {config['video']['width']}px;
            height: {config['video']['height']}px;
            background-color: {config['video']['bg_color']};
        }}
    </style>
</head>
<body></body>
</html>"""
            
            bg_html_path = os.path.join(temp_dir, "background.html")
            with open(bg_html_path, 'w') as f:
                f.write(bg_html)
            
            # Try to convert HTML to PNG using Chrome
            chrome_cmd = None
            system = platform.system()
            
            if system == "Darwin":
                mac_chrome_paths = [
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                    "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
                    "/Applications/Chromium.app/Contents/MacOS/Chromium"
                ]
                
                for path in mac_chrome_paths:
                    if os.path.exists(path):
                        chrome_cmd = path
                        break
            
            if not chrome_cmd:
                chrome_cmd = 'google-chrome'
            
            command = [
                chrome_cmd, '--headless', '--disable-gpu',
                f'--window-size={config["video"]["width"]},{config["video"]["height"]}',
                f'--screenshot={os.path.abspath(background_path)}',
                f'file://{os.path.abspath(bg_html_path)}'
            ]
            
            run_command(command, "background screenshot")
    
    # 6. Create intro/outro videos
    log("Step 5: Creating intro and outro videos...")
    intro_outro_videos = create_intro_outro_videos(
        temp_dir,
        config["video"]["watermark"],
        config["video"]["width"],
        config["video"]["height"],
        config["video"]["bg_color"],
        config["video"]["text_color"]
    )
    
    # 7. Process target languages
    log("Step 6: Processing target languages...")
    
    # Filter target languages if specified
    if target_languages:
        languages = [lang for lang in config["languages"] if lang["code"] in target_languages]
    else:
        languages = config["languages"]
    
    results = {}
    
    for language in languages:
        lang_code = language["code"]
        lang_name = language["name"]
        
        # Skip source language if it's in the list
        if lang_code.lower() == source_language.lower():
            log(f"Skipping source language: {lang_name}")
            continue
        
        log(f"Processing language: {lang_name}")
        
        output_dir = os.path.join(base_dir, f"target/{lang_code}/{filename_base}")
        os.makedirs(output_dir, exist_ok=True)
        
        result = process_language(
            cleaned_segments,
            source_language,
            language,
            config["elevenlabs"],
            config["claude"],
            config["video"],
            background_path,
            output_dir,
            filename_base,
            intro_outro_videos
        )
        
        if result and result.get("success", False):
            results[lang_code] = result
    
    # Return results
    return {
        "success": True,
        "id": video_id,
        "title": video_title,
        "source_language": source_language,
        "processed_languages": results,
        "input_type": "youtube" if is_youtube else "markdown"
    }

# ======= COMMAND LINE INTERFACE =======

def parse_arguments():
    """Parse command line arguments"""
    parser = argparse.ArgumentParser(description="Multilingual Dubbing Pipeline")
    
    parser.add_argument(
        "--input", "-i",
        required=True,
        help="Input source: YouTube video URL or path to markdown file"
    )
    
    parser.add_argument(
        "--config", "-c",
        help="Path to JSON configuration file"
    )
    
    parser.add_argument(
        "--elevenlabs-key", "-e",
        help="ElevenLabs API key (overrides config file)"
    )
    
    parser.add_argument(
        "--claude-key", "-a",
        help="Anthropic Claude API key (overrides config file)"
    )
    
    parser.add_argument(
        "--source-language", "-s",
        default="English",
        help="Source language of the video (default: English)"
    )
    
    parser.add_argument(
        "--target-languages", "-t",
        nargs="+",
        help="List of target language codes to process (default: all configured languages)"
    )
    
    parser.add_argument(
        "--use-original-video", "-v",
        action="store_true",
        help="Use the original video as background instead of a still image"
    )
    
    parser.add_argument(
        "--add-subtitles", "-sub",
        action="store_true",
        help="Add subtitles to the videos"
    )
    
    parser.add_argument(
        "--subtitle-size", "-ss",
        type=int,
        default=30,
        help="Font size for subtitles (default: 30)"
    )
    
    return parser.parse_args()

def main():
    """Main entry point"""
    args = parse_arguments()
    
    # Load configuration
    config = load_config(args.config)
    
    # Override config with command line arguments if provided
    if args.use_original_video:
        config["video"]["use_original_video"] = True
    
    # Run the pipeline
    result = multilingual_dubbing_pipeline(
        args.url,
        config,
        args.source_language,
        args.target_languages
    )
    
    if result["success"]:
        log("Multilingual dubbing completed successfully!")
        log(f"Processed {len(result['processed_languages'])} languages")
        
        for lang_code, video_path in result["processed_languages"].items():
            log(f"- {lang_code}: {video_path}")
    else:
        log(f"Pipeline failed: {result.get('error', 'Unknown error')}")

if __name__ == "__main__":
    main()
