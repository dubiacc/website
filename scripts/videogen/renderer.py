#!/usr/bin/env python3
"""
renderer.py - Render subtitles over video using chrome-headless

This script takes the subtitles JSON file produced by preprocessing.py
and renders the subtitles over the video using chrome-headless.
It uses Chrome DevTools Protocol to capture screenshots and combine them into a video.
"""

import json
import os
import argparse
import subprocess
import time
import tempfile
import base64
import websocket
import requests
import threading
from typing import Dict, Any, List, Optional
from pathlib import Path


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(description='Render subtitles over video using chrome-headless')
    parser.add_argument('--subtitles', required=True, help='Path to the subtitles JSON file')
    parser.add_argument('--chrome-path', default='google-chrome', help='Path to Chrome executable')
    parser.add_argument('--output-dir', default='output', help='Directory to store output files')
    parser.add_argument('--output-video', default='output.mp4', help='Output video file name')
    parser.add_argument('--width', type=int, default=1280, help='Video width')
    parser.add_argument('--height', type=int, default=720, help='Video height')
    parser.add_argument('--fps', type=int, default=30, help='Frames per second for recording')
    parser.add_argument('--use-dub', action='store_true', help='Use dubbed audio instead of original audio')
    parser.add_argument('--dub-dir', default='dubs', help='Directory containing dubbed audio files')
    return parser.parse_args()


def load_json(file_path: str) -> Dict[str, Any]:
    """Load a JSON file."""
    with open(file_path, 'r', encoding='utf-8') as f:
        return json.load(f)


def create_html(subtitle: Dict[str, Any], output_dir: str, width: int, height: int) -> str:
    """Create an HTML file with the subtitle text."""
    html_dir = os.path.join(output_dir, 'html')
    os.makedirs(html_dir, exist_ok=True)
    
    index = subtitle.get('index', 0)
    html_path = os.path.join(html_dir, f'subtitle_{index:04d}.html')
    
    # Create HTML content
    html_content = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{
            margin: 0;
            padding: 0;
            width: {width}px;
            height: {height}px;
            background-color: black;
            display: flex;
            align-items: flex-end;  /* Align to bottom */
            justify-content: center;
            overflow: hidden;
        }}
        .subtitle {{
            color: white;
            font-family: Arial, sans-serif;
            font-size: 36px;
            text-shadow: 2px 2px 2px black;
            margin-bottom: 50px;  /* Space from bottom */
            padding: 10px;
            max-width: 80%;
            text-align: center;
        }}
        
        /* Add animation for fade-in and fade-out */
        @keyframes fadein {{
            from {{ opacity: 0; }}
            to {{ opacity: 1; }}
        }}
        
        .subtitle {{
            animation: fadein 0.5s forwards;
        }}
    </style>
</head>
<body>
    <div class="subtitle">{subtitle['text']}</div>
    
    <audio id="audio" src="{os.path.abspath(subtitle['audio_file'])}" autoplay></audio>
</body>
</html>
"""
    
    # Write HTML to file
    with open(html_path, 'w', encoding='utf-8') as f:
        f.write(html_content)
    
    return html_path


def start_chrome(chrome_path: str, debug_port: int, width: int, height: int) -> subprocess.Popen:
    """Start Chrome in headless mode with remote debugging enabled."""
    cmd = [
        chrome_path,
        '--headless',
        '--disable-gpu',
        f'--window-size={width},{height}',
        '--no-sandbox',
        f'--remote-debugging-port={debug_port}'
    ]
    
    # Start Chrome
    process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    
    # Give Chrome a moment to start
    time.sleep(2)
    
    return process


def get_ws_url(debug_port: int) -> str:
    """Get the WebSocket URL for Chrome DevTools Protocol."""
    response = requests.get(f'http://localhost:{debug_port}/json/list')
    data = response.json()
    if not data:
        raise Exception(f"No Chrome tabs found on port {debug_port}")
    return data[0]['webSocketDebuggerUrl']


def render_subtitle(subtitle: Dict[str, Any], chrome_path: str, output_dir: str, 
                   width: int, height: int, fps: int, 
                   use_dub: bool = False, dub_dir: str = 'dubs') -> Optional[str]:
    """Render a subtitle using chrome-headless and record it."""
    # Decide which audio file to use
    audio_file = subtitle['audio_file']
    if use_dub and 'dub' in subtitle and subtitle['dub']:
        # Generate a filename for the dubbed audio
        dub_filename = f"dub_{subtitle['index']:04d}.mp3"
        dub_path = os.path.join(dub_dir, dub_filename)
        
        # If the dubbed audio file exists, use it
        if os.path.exists(dub_path):
            audio_file = dub_path
    
    # Skip if no audio file
    if not audio_file or not os.path.exists(audio_file):
        print(f"Skipping subtitle {subtitle['index']}: No audio file found")
        return None
    
    # Update the subtitle with the correct audio file
    subtitle['audio_file'] = audio_file
    
    # Create HTML file
    html_path = create_html(subtitle, output_dir, width, height)
    
    # Create directories for frames and videos
    frames_dir = os.path.join(output_dir, 'frames', f'subtitle_{subtitle["index"]:04d}')
    os.makedirs(frames_dir, exist_ok=True)
    
    video_dir = os.path.join(output_dir, 'videos')
    os.makedirs(video_dir, exist_ok=True)
    
    # Output video file
    video_path = os.path.join(video_dir, f'video_{subtitle["index"]:04d}.mp4')
    
    # Start Chrome
    debug_port = 9222 + subtitle['index'] % 10  # Use different ports to avoid conflicts
    chrome_process = start_chrome(chrome_path, debug_port, width, height)
    
    try:
        # Get WebSocket URL
        ws_url = get_ws_url(debug_port)
        
        # Connect to Chrome DevTools Protocol
        ws = websocket.create_connection(ws_url)
        
        # Navigate to the HTML file
        msg_id = 1
        ws.send(json.dumps({
            'id': msg_id,
            'method': 'Page.navigate',
            'params': {'url': f'file://{os.path.abspath(html_path)}'}
        }))
        response = json.loads(ws.recv())
        
        # Wait for the page to load
        time.sleep(1)
        
        # Enable Page domain
        msg_id += 1
        ws.send(json.dumps({
            'id': msg_id,
            'method': 'Page.enable'
        }))
        response = json.loads(ws.recv())
        
        # Start screencast
        msg_id += 1
        ws.send(json.dumps({
            'id': msg_id,
            'method': 'Page.startScreencast',
            'params': {
                'format': 'jpeg',
                'quality': 90,
                'maxWidth': width,
                'maxHeight': height
            }
        }))
        response = json.loads(ws.recv())
        
        # Calculate duration
        duration = subtitle['end'] - subtitle['start']
        total_frames = int(duration * fps)
        
        # Record frames
        frames = []
        frame_count = 0
        
        def capture_frames():
            nonlocal frame_count
            while frame_count < total_frames:
                try:
                    response = json.loads(ws.recv())
                    if response.get('method') == 'Page.screencastFrame':
                        frame_data = response['params']['data']
                        frame_path = os.path.join(frames_dir, f'frame_{frame_count:04d}.jpg')
                        
                        # Save frame
                        with open(frame_path, 'wb') as f:
                            f.write(base64.b64decode(frame_data))
                        
                        frames.append(frame_path)
                        frame_count += 1
                        
                        # Acknowledge the frame
                        ws.send(json.dumps({
                            'id': msg_id + frame_count,
                            'method': 'Page.screencastFrameAck',
                            'params': {'sessionId': response['params']['sessionId']}
                        }))
                except Exception as e:
                    print(f"Error capturing frame: {e}")
                    break
        
        # Start capturing frames in a separate thread
        capture_thread = threading.Thread(target=capture_frames)
        capture_thread.start()
        
        # Sleep for the duration of the subtitle
        time.sleep(duration + 0.5)  # Add a small buffer
        
        # Stop screencast
        msg_id += 1
        ws.send(json.dumps({
            'id': msg_id,
            'method': 'Page.stopScreencast'
        }))
        
        # Close WebSocket
        ws.close()
        
        # Wait for the capture thread to finish
        capture_thread.join(timeout=5)
        
        # Combine frames into a video
        if frames:
            cmd = [
                'ffmpeg', '-y',
                '-framerate', str(fps),
                '-i', os.path.join(frames_dir, 'frame_%04d.jpg'),
                '-i', audio_file,
                '-c:v', 'libx264',
                '-pix_fmt', 'yuv420p',
                '-c:a', 'aac',
                '-shortest',
                video_path
            ]
            
            subprocess.run(cmd, check=True)
            return video_path
        else:
            print(f"No frames captured for subtitle {subtitle['index']}")
            return None
        
    except Exception as e:
        print(f"Error rendering subtitle {subtitle['index']}: {e}")
        return None
        
    finally:
        # Make sure Chrome is closed
        if chrome_process:
            chrome_process.terminate()
            try:
                chrome_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                chrome_process.kill()


def combine_videos(video_paths: List[str], output_path: str) -> None:
    """Combine multiple videos into one."""
    if not video_paths:
        print("No videos to combine")
        return
        
    # Create a file listing all the videos
    with tempfile.NamedTemporaryFile('w', suffix='.txt', delete=False) as f:
        for video_path in video_paths:
            f.write(f"file '{os.path.abspath(video_path)}'\n")
        file_list = f.name
    
    # Use ffmpeg to concatenate the videos
    cmd = [
        'ffmpeg', '-y',
        '-f', 'concat',
        '-safe', '0',
        '-i', file_list,
        '-c', 'copy',
        output_path
    ]
    
    try:
        subprocess.run(cmd, check=True)
        print(f"Combined {len(video_paths)} videos into {output_path}")
    except subprocess.CalledProcessError as e:
        print(f"Error combining videos: {e}")
    
    # Remove the temporary file
    os.unlink(file_list)


def main():
    """Main function."""
    args = parse_args()
    
    # Create output directory
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Create dub directory if using dubs
    if args.use_dub:
        dub_dir = Path(args.dub_dir)
        dub_dir.mkdir(parents=True, exist_ok=True)
        print(f"Using dubbed audio from {dub_dir}")
    
    # Load subtitles
    data = load_json(args.subtitles)
    subtitles = data['subtitles']
    
    # Add index to each subtitle
    for i, subtitle in enumerate(subtitles):
        subtitle['index'] = i
    
    # Render each subtitle
    video_paths = []
    total = len(subtitles)
    
    for i, subtitle in enumerate(subtitles):
        print(f"Rendering subtitle {i+1}/{total}: {subtitle['text']}")
        video_path = render_subtitle(
            subtitle, 
            args.chrome_path, 
            str(output_dir), 
            args.width, 
            args.height, 
            args.fps,
            args.use_dub,
            args.dub_dir
        )
        if video_path and os.path.exists(video_path):
            video_paths.append(video_path)
    
    # Combine videos
    if video_paths:
        output_path = output_dir / args.output_video
        combine_videos(video_paths, str(output_path))
        print(f'Rendering complete. Output written to {output_path}')
    else:
        print('No videos were created. Check for errors.')


if __name__ == '__main__':
    main()