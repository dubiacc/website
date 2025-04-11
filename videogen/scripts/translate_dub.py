#!/usr/bin/env python3
"""
translate_dub.py - Automatically translate and dub video using Claude and ElevenLabs

This script takes the subtitles JSON produced by preprocessing.py and:
1. Translates the text using Claude API (if target language is specified)
2. Adds timing breaks to match the original pacing
3. Generates TTS audio using ElevenLabs API
4. Combines all audio segments into one file matching the cut video
"""

import json
import os
import argparse
import subprocess
import http.client
import urllib.request
import urllib.parse
import time
import random
import ssl
from typing import Dict, Any, List, Optional, Tuple
from pathlib import Path


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(description='Translate and dub video using Claude and ElevenLabs')
    parser.add_argument('--subtitles', required=True, help='Path to the subtitles JSON file')
    parser.add_argument('--output-dir', default='output', help='Directory to store output files')
    parser.add_argument('--output-audio', default='dubbed_audio.mp3', help='Output audio file name')
    parser.add_argument('--target-language', default=None, help='Target language for translation (e.g., "German")')
    parser.add_argument('--claude-api-key', required=True, help='API key for Claude')
    parser.add_argument('--eleven-api-key', required=True, help='API key for ElevenLabs')
    parser.add_argument('--voice-id', required=True, help='ElevenLabs voice ID to use')
    parser.add_argument('--speaking-rate', type=float, default=1.0, help='Speaking rate for the TTS voice (0.5-2.0)')
    parser.add_argument('--max-retries', type=int, default=3, help='Maximum number of retries for API calls')
    return parser.parse_args()


def load_json(file_path: str) -> Dict[str, Any]:
    """Load a JSON file."""
    with open(file_path, 'r', encoding='utf-8') as f:
        return json.load(f)


def translate_text(text: str, target_language: str, api_key: str, max_retries: int = 3) -> str:
    """Translate text using Claude API."""
    # Set up the connection
    conn = http.client.HTTPSConnection("api.anthropic.com")
    
    # Request headers
    headers = {
        "Content-Type": "application/json",
        "X-API-Key": api_key,
        "anthropic-version": "2023-06-01"
    }
    
    # Request payload
    payload = json.dumps({
        "model": "claude-3-opus-20240229",
        "max_tokens": 4000,
        "messages": [
            {
                "role": "user", 
                "content": f"""Translate the following text to {target_language}. 
Preserve the original meaning, tone, and style as closely as possible.
Only provide the translation with no additional text:

{text}"""
            }
        ]
    })
    
    # Try the request with retries
    for attempt in range(max_retries):
        try:
            conn.request("POST", "/v1/messages", payload, headers)
            response = conn.getresponse()
            response_data = response.read().decode("utf-8")
            
            if response.status == 200:
                result = json.loads(response_data)
                translated_text = result["content"][0]["text"]
                return translated_text.strip()
            else:
                print(f"Translation error (attempt {attempt+1}/{max_retries}): {response.status} - {response_data}")
                if attempt < max_retries - 1:
                    # Wait with exponential backoff
                    time.sleep(2 ** attempt + random.random())
                    continue
                return text  # Fallback to original text after max retries
        except Exception as e:
            print(f"Error during translation (attempt {attempt+1}/{max_retries}): {e}")
            if attempt < max_retries - 1:
                time.sleep(2 ** attempt + random.random())
                continue
            return text  # Fallback to original
    
    return text  # Fallback to original text


def analyze_timing(text: str, duration: float) -> List[Tuple[str, float]]:
    """
    Analyze the text and duration to determine appropriate break points and timing.
    Returns a list of (text_chunk, pause_duration) tuples.
    """
    # Count words, characters, and sentences
    words = text.split()
    word_count = len(words)
    char_count = len(text)
    sentences = [s.strip() for s in text.replace('!', '.').replace('?', '.').split('.') if s.strip()]
    sentence_count = len(sentences)
    
    # Simple heuristic for text chunks
    if word_count <= 5 or duration < 2.0:
        # Short text, no breaks needed
        return [(text, 0)]
    
    chunks = []
    
    # Use sentence boundaries if available
    if sentence_count > 1:
        current_sentence = ""
        accumulated_sentences = []
        
        for i, sentence in enumerate(sentences):
            # Add period back except for last sentence if original doesn't end with period
            if i < len(sentences) - 1 or text.strip().endswith('.'):
                sentence = sentence + "."
            
            current_sentence += " " + sentence if current_sentence else sentence
            accumulated_sentences.append(sentence)
            
            # Decide if we should break here
            if (len(accumulated_sentences) >= sentence_count / 2) or \
               (i == len(sentences) - 1):
                # Calculate pause duration based on position
                if i == len(sentences) - 1:
                    # No pause after last chunk
                    pause = 0
                else:
                    # Proportional pause based on position
                    position_factor = (i + 1) / sentence_count
                    pause = min(0.8, duration * 0.15 * position_factor)
                
                chunks.append((current_sentence.strip(), pause))
                current_sentence = ""
                accumulated_sentences = []
    else:
        # No sentence boundaries, use word count
        words_per_chunk = max(3, word_count // 3)
        current_chunk = []
        
        for i, word in enumerate(words):
            current_chunk.append(word)
            
            # Decide if we should break here
            if len(current_chunk) >= words_per_chunk or i == len(words) - 1:
                chunk_text = " ".join(current_chunk)
                
                # Calculate pause
                if i == len(words) - 1:
                    pause = 0  # No pause after last chunk
                else:
                    position_factor = (i + 1) / word_count
                    pause = min(0.5, duration * 0.1 * position_factor)
                
                chunks.append((chunk_text, pause))
                current_chunk = []
    
    return chunks


def format_text_with_breaks(text: str, duration: float) -> str:
    """
    Add timing breaks to match the original audio duration.
    Uses intelligent analysis to place breaks at natural points.
    """
    chunks = analyze_timing(text, duration)
    
    # Format the text with break tags
    result = ""
    for i, (chunk, pause) in enumerate(chunks):
        result += chunk
        if pause > 0:
            result += f" <break time=\"{pause:.2f}s\" /> "
    
    return result


def generate_audio(text: str, voice_id: str, api_key: str, output_path: str, 
                  speaking_rate: float = 1.0, max_retries: int = 3) -> bool:
    """Generate audio using ElevenLabs API."""
    # Set up the connection
    conn = http.client.HTTPSConnection("api.elevenlabs.io")
    
    # Request headers
    headers = {
        "Accept": "audio/mpeg",
        "Content-Type": "application/json",
        "xi-api-key": api_key
    }
    
    # Ensure speaking rate is within bounds
    speaking_rate = max(0.5, min(2.0, speaking_rate))
    
    # Request payload
    payload = json.dumps({
        "text": text,
        "model_id": "eleven_multilingual_v2",
        "voice_settings": {
            "stability": 0.75,
            "similarity_boost": 0.75,
            "speaking_rate": speaking_rate
        }
    })
    
    # Try the request with retries
    for attempt in range(max_retries):
        try:
            conn.request("POST", f"/v1/text-to-speech/{voice_id}", payload, headers)
            response = conn.getresponse()
            
            if response.status == 200:
                # Read the audio data
                audio_data = response.read()
                
                # Write to file
                with open(output_path, "wb") as f:
                    f.write(audio_data)
                
                return True
            else:
                error_data = response.read().decode("utf-8")
                print(f"Audio generation error (attempt {attempt+1}/{max_retries}): {response.status} - {error_data}")
                
                if attempt < max_retries - 1:
                    # Wait with exponential backoff
                    time.sleep(2 ** attempt + random.random())
                    continue
                return False
        
        except Exception as e:
            print(f"Error during audio generation (attempt {attempt+1}/{max_retries}): {e}")
            if attempt < max_retries - 1:
                time.sleep(2 ** attempt + random.random())
                continue
            return False
    
    return False


def get_audio_duration(audio_path: str) -> float:
    """Get the duration of an audio file in seconds."""
    cmd = [
        'ffprobe', '-v', 'error',
        '-show_entries', 'format=duration',
        '-of', 'default=noprint_wrappers=1:nokey=1',
        audio_path
    ]
    
    try:
        result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=True)
        return float(result.stdout.strip())
    except (subprocess.CalledProcessError, ValueError) as e:
        print(f"Error getting audio duration: {e}")
        return 0.0


def create_silence(duration: float, output_path: str) -> bool:
    """Create a silent audio file of specified duration."""
    cmd = [
        'ffmpeg', '-y',
        '-f', 'lavfi',
        '-i', 'anullsrc=r=44100:cl=stereo',
        '-t', str(duration),
        '-q:a', '0',
        '-c:a', 'libmp3lame',
        output_path
    ]
    
    try:
        subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
        return True
    except subprocess.CalledProcessError as e:
        print(f"Error creating silence: {e}")
        return False


def combine_audio_files(file_list: List[str], output_path: str) -> bool:
    """Combine multiple audio files into one."""
    # Create a file listing all the audio files
    list_file = "temp_audio_list.txt"
    with open(list_file, 'w', encoding='utf-8') as f:
        for audio_file in file_list:
            f.write(f"file '{os.path.abspath(audio_file)}'\n")
    
    # Use ffmpeg to concatenate the audio files
    cmd = [
        'ffmpeg', '-y',
        '-f', 'concat',
        '-safe', '0',
        '-i', list_file,
        '-c:a', 'libmp3lame',
        '-q:a', '0',
        output_path
    ]
    
    try:
        subprocess.run(cmd, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        # Remove the temporary file
        os.remove(list_file)
        return True
    except subprocess.CalledProcessError as e:
        print(f"Error combining audio files: {e}")
        # Remove the temporary file
        if os.path.exists(list_file):
            os.remove(list_file)
        return False


def adjust_audio_timing(audio_path: str, target_duration: float, output_path: str) -> bool:
    """
    Adjust audio timing to match target duration.
    Uses atempo filter to speed up or slow down without changing pitch.
    """
    # Get current duration
    current_duration = get_audio_duration(audio_path)
    
    if current_duration <= 0:
        print(f"Error: Could not determine duration of {audio_path}")
        return False
    
    # Calculate tempo factor
    tempo_factor = current_duration / target_duration
    
    # Limit tempo factor to ffmpeg's supported range
    tempo_factor = max(0.5, min(2.0, tempo_factor))
    
    # Use ffmpeg to adjust timing
    cmd = [
        'ffmpeg', '-y',
        '-i', audio_path,
        '-filter:a', f'atempo={tempo_factor}',
        '-c:a', 'libmp3lame',
        '-q:a', '0',
        output_path
    ]
    
    try:
        subprocess.run(cmd, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        
        # Verify the new duration
        new_duration = get_audio_duration(output_path)
        print(f"Adjusted audio duration: {current_duration:.2f}s → {new_duration:.2f}s (target: {target_duration:.2f}s)")
        
        return True
    except subprocess.CalledProcessError as e:
        print(f"Error adjusting audio timing: {e}")
        return False


def main():
    """Main function."""
    args = parse_args()
    
    # Create output directory
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Create directories for audio segments
    audio_segments_dir = output_dir / "audio_segments"
    audio_segments_dir.mkdir(exist_ok=True)
    
    # Create a directory for silence files
    silence_dir = output_dir / "silence"
    silence_dir.mkdir(exist_ok=True)
    
    # Create a directory for adjusted audio
    adjusted_audio_dir = output_dir / "adjusted_audio"
    adjusted_audio_dir.mkdir(exist_ok=True)
    
    # Load subtitles
    data = load_json(args.subtitles)
    subtitles = data['subtitles']
    
    # Sort subtitles by start time
    subtitles.sort(key=lambda x: x['start'])
    
    # Process each subtitle
    audio_files = []
    total_subtitles = len(subtitles)
    
    print(f"Processing {total_subtitles} subtitle segments...")
    
    for i, subtitle in enumerate(subtitles):
        print(f"\nProcessing subtitle {i+1}/{total_subtitles}")
        print(f"  Original text: {subtitle['text']}")
        
        # Get the text to process (use dub text if available, otherwise use subtitle text)
        text = subtitle.get('dub', subtitle['text'])
        
        # Calculate duration for this segment
        duration = subtitle['end'] - subtitle['start']
        print(f"  Duration: {duration:.2f}s")
        
        # Translate if target language is specified
        if args.target_language:
            print(f"  Translating to {args.target_language}...")
            text = translate_text(text, args.target_language, args.claude_api_key, args.max_retries)
            print(f"  Translated text: {text}")
        
        # Add timing breaks
        text_with_breaks = format_text_with_breaks(text, duration)
        print(f"  Text with breaks: {text_with_breaks}")
        
        # Generate audio
        audio_path = audio_segments_dir / f"segment_{i:04d}.mp3"
        print(f"  Generating audio with ElevenLabs...")
        
        if generate_audio(text_with_breaks, args.voice_id, args.eleven_api_key, 
                          str(audio_path), args.speaking_rate, args.max_retries):
            
            # Adjust timing if needed
            generated_duration = get_audio_duration(str(audio_path))
            print(f"  Generated audio duration: {generated_duration:.2f}s")
            
            # If there's a significant difference, adjust timing
            if abs(generated_duration - duration) > 0.5:
                print(f"  Adjusting timing to match original duration...")
                adjusted_path = adjusted_audio_dir / f"adjusted_{i:04d}.mp3"
                
                if adjust_audio_timing(str(audio_path), duration, str(adjusted_path)):
                    audio_files.append(str(adjusted_path))
                else:
                    print(f"  Timing adjustment failed, using original generated audio")
                    audio_files.append(str(audio_path))
            else:
                audio_files.append(str(audio_path))
        else:
            print(f"  Failed to generate audio for segment {i+1}")
        
        # Add silence between segments if needed
        if i < total_subtitles - 1:
            next_start = subtitles[i+1]['start']
            current_end = subtitle['end']
            
            silence_duration = next_start - current_end
            
            if silence_duration > 0.1:  # Only add if gap is significant
                print(f"  Adding silence of {silence_duration:.2f}s")
                silence_path = silence_dir / f"silence_{i:04d}.mp3"
                
                if create_silence(silence_duration, str(silence_path)):
                    audio_files.append(str(silence_path))
    
    # Combine all audio files
    if audio_files:
        output_path = output_dir / args.output_audio
        print(f"\nCombining {len(audio_files)} audio segments...")
        
        if combine_audio_files(audio_files, str(output_path)):
            print(f"Successfully created dubbed audio: {output_path}")
            
            # Get final duration
            final_duration = get_audio_duration(str(output_path))
            print(f"Final audio duration: {final_duration:.2f}s")
            
            # Calculate original full duration
            if subtitles:
                original_duration = subtitles[-1]['end'] - subtitles[0]['start']
                print(f"Original duration: {original_duration:.2f}s")
                print(f"Difference: {final_duration - original_duration:.2f}s")
        else:
            print("Failed to combine audio files")
    else:
        print("No audio files were generated")


if __name__ == '__main__':
    main()