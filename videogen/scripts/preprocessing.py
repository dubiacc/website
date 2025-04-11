#!/usr/bin/env python3
"""
preprocessing.py - Pre-process video timestamps and audio for subtitle rendering

This script takes two JSON files:
1. An outline of the video (start/end/dub)
2. A word-by-word transcript with timestamps

It produces:
1. A JSON file with subtitles grouped (max 4 words each)
2. Audio files extracted from the original video for each segment
"""

import json
import os
import argparse
import subprocess
from typing import List, Dict, Any, Optional
from pathlib import Path


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(description='Pre-process video timestamps and audio for subtitle rendering')
    parser.add_argument('--outline', required=True, help='Path to the outline JSON file (start/end/dub)')
    parser.add_argument('--transcript', required=True, help='Path to the word-by-word transcript JSON file')
    parser.add_argument('--video', required=True, help='Path to the original video file')
    parser.add_argument('--output-dir', default='output', help='Directory to store output files')
    parser.add_argument('--output-json', default='subtitles.json', help='Output JSON file name')
    return parser.parse_args()


def load_json(file_path: str) -> Dict[str, Any]:
    """Load a JSON file."""
    with open(file_path, 'r', encoding='utf-8') as f:
        return json.load(f)


def convert_time_to_seconds(time_str: str) -> float:
    """Convert a time string (HH:MM:SS) to seconds."""
    h, m, s = time_str.split(':')
    return int(h) * 3600 + int(m) * 60 + float(s)


def create_subtitle_groups(transcript: Dict[str, Any]) -> List[Dict[str, Any]]:
    """Create subtitle groups of max 4 words each."""
    subtitles = []
    
    # Process each segment in the transcript
    for segment in transcript.get('segments', []):
        words = []
        current_group = []
        current_start = None
        
        # First collect valid word objects from the segment
        for word_obj in segment.get('words', []):
            if word_obj.get('type') == 'word':
                words.append(word_obj)
        
        # Now group them into subtitles of max 4 words
        for word in words:
            if len(current_group) == 0:
                current_start = word['start']
                
            current_group.append(word)
            
            # If we have 4 words or this is the last word, create a subtitle
            if len(current_group) == 4 or word == words[-1]:
                subtitle_text = ' '.join(w['text'] for w in current_group)
                end_time = current_group[-1]['end']
                
                # Add the subtitle
                subtitles.append({
                    'start': current_start,
                    'end': end_time,
                    'text': subtitle_text
                })
                
                # Reset for next group
                current_group = []
                current_start = None
    
    return subtitles


def match_subtitles_with_outline(subtitles: List[Dict[str, Any]], outline: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Match subtitles with the outline sections."""
    matched_subtitles = []
    
    for section in outline:
        # Convert start/end times to seconds
        section_start = convert_time_to_seconds(section['start'])
        section_end = convert_time_to_seconds(section['end'])
        
        # Find subtitles within this section
        section_subtitles = [
            sub for sub in subtitles
            if section_start <= sub['start'] < section_end
        ]
        
        # Add section info to matched subtitles
        for sub in section_subtitles:
            matched_subtitles.append({
                'start': sub['start'],
                'end': sub['end'],
                'text': sub['text'],
                'section_start': section_start,
                'section_end': section_end,
                'dub': section.get('dub', '')
            })
    
    return matched_subtitles


def extract_audio(video_path: str, output_dir: str, subtitles: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Extract audio segments from the video file."""
    audio_dir = os.path.join(output_dir, 'audio')
    os.makedirs(audio_dir, exist_ok=True)
    
    for i, subtitle in enumerate(subtitles):
        # Calculate duration
        duration = subtitle['end'] - subtitle['start']
        
        # Output audio file path
        audio_file = os.path.join(audio_dir, f'audio_{i:04d}.mp3')
        
        # Use ffmpeg to extract the audio segment
        cmd = [
            'ffmpeg', '-y',
            '-i', video_path,
            '-ss', str(subtitle['start']),
            '-t', str(duration),
            '-q:a', '0',
            '-map', 'a',
            audio_file
        ]
        
        try:
            subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
            # Add audio file path to subtitle
            subtitle['audio_file'] = audio_file
        except subprocess.CalledProcessError as e:
            print(f"Error extracting audio for subtitle {i}: {e}")
            subtitle['audio_file'] = ''
    
    return subtitles


def main():
    """Main function."""
    args = parse_args()
    
    # Create output directory
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Load input files
    outline = load_json(args.outline)
    transcript = load_json(args.transcript)
    
    # Create subtitle groups
    subtitles = create_subtitle_groups(transcript)
    
    # Match subtitles with outline
    matched_subtitles = match_subtitles_with_outline(subtitles, outline)
    
    # Extract audio for each subtitle
    subtitles_with_audio = extract_audio(args.video, str(output_dir), matched_subtitles)
    
    # Write output JSON
    output_path = output_dir / args.output_json
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump({'subtitles': subtitles_with_audio}, f, indent=2)
    
    print(f'Processing complete. Output written to {output_path}')
    print(f'Total subtitles: {len(subtitles_with_audio)}')
    print(f'Audio files extracted to: {output_dir}/audio/')


if __name__ == '__main__':
    main()