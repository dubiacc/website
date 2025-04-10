#!/usr/bin/env python3
"""
German Rosary Generator - Direct Method

This script generates a complete audio-visual version of the German Rosary using a direct method:
1. Generate silent videos from image sequences
2. Generate complete audio tracks separately
3. Combine them in a final step without using concat
"""

import os
import json
import time
import requests
import subprocess
import re
import platform
import argparse
import sys
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional, Tuple, Union, Any, NamedTuple

# ======= CONFIGURATION =======
WIDTH = 1280
HEIGHT = 720
BG_COLOR = "#181818"  # Dark gray background
TEXT_COLOR = "#FFFFFF"  # White text
HIGHLIGHT_COLOR = "#FFCC66"  # Light yellow for highlighting

class Config(NamedTuple):
    """Configuration object for the script"""
    api_key: str
    voice_id: str
    fail_on_error: bool
    width: int = WIDTH
    height: int = HEIGHT
    bg_color: str = BG_COLOR
    text_color: str = TEXT_COLOR
    highlight_color: str = HIGHLIGHT_COLOR


def parse_arguments() -> Config:
    """Parse command line arguments"""
    parser = argparse.ArgumentParser(description="German Rosary Generator - Direct Method")
    parser.add_argument("--api-key", required=True, help="ElevenLabs API key")
    parser.add_argument("--voice-id", required=True, help="ElevenLabs voice ID")
    parser.add_argument("--fail-on-error", action="store_true", 
                        help="Exit on first error encountered")
    parser.add_argument("--width", type=int, default=WIDTH, 
                        help=f"Video width in pixels (default: {WIDTH})")
    parser.add_argument("--height", type=int, default=HEIGHT, 
                        help=f"Video height in pixels (default: {HEIGHT})")
    
    args = parser.parse_args()
    
    return Config(
        api_key=args.api_key,
        voice_id=args.voice_id,
        fail_on_error=args.fail_on_error,
        width=args.width,
        height=args.height
    )


def log(message: str) -> None:
    """Log a message with timestamp"""
    timestamp = datetime.now().strftime("%H:%M:%S")
    print(f"[{timestamp}] {message}")


def exit_on_error(config: Config, message: str) -> None:
    """Exit the script if fail_on_error is True"""
    if config.fail_on_error:
        log(f"ERROR: {message}")
        log("Exiting due to --fail-on-error flag")
        sys.exit(1)
    log(f"WARNING: {message}")


def ensure_dirs() -> None:
    """Create necessary directories if they don't exist"""
    dirs = [
        'target', 
        'target/html_files', 
        'target/png_files', 
        'target/audio_files', 
        'target/silent_videos',
        'target/final_audio',
        'target/final_output'
    ]
    for directory in dirs:
        os.makedirs(directory, exist_ok=True)


# === PRAYERS AND HTML/PNG GENERATION (from original script) ===
# [Include all the prayer text functions and HTML/PNG generation from original]
def get_prayers() -> Dict[str, str]:
    """Return a dictionary of prayer texts"""
    prayers = {
        "kreuzzeichen": "Im Namen des Vaters und des Sohnes und des Heiligen Geistes. Amen.",
        
        # Split the Credo into two parts due to length
        "apostolisches_glaubensbekenntnis_1": """Ich glaube an Gott, den Vater, den Allmächtigen, den Schöpfer des Himmels und der Erde, und an Jesus Christus, seinen eingeborenen Sohn, unsern Herrn, empfangen durch den Heiligen Geist, geboren von der Jungfrau Maria, gelitten unter Pontius Pilatus, gekreuzigt, gestorben und begraben, hinabgestiegen in das Reich des Todes, am dritten Tage auferstanden von den Toten.""",
        
        "apostolisches_glaubensbekenntnis_2": """Aufgefahren in den Himmel; er sitzt zur Rechten Gottes, des allmächtigen Vaters; von dort wird er kommen, zu richten die Lebenden und die Toten. <pause />
Ich glaube an den Heiligen Geist, die heilige katholische Kirche, Gemeinschaft der Heiligen, Vergebung der Sünden, Auferstehung der Toten und das ewige Leben. Amen.""",
        
        "vaterunser": """Vater unser im Himmel, geheiligt werde dein Name. Dein Reich komme. Dein Wille geschehe, wie im Himmel so auf Erden. <pause />
Unser tägliches Brot gib uns heute. Und vergib uns unsere Schuld, wie auch wir vergeben unsern Schuldigern. Und führe uns nicht in Versuchung, sondern erlöse uns von dem Bösen. Amen.""",
        
        "avemaria_glauben": """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der in uns den christlichen Glauben vermehre. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen.""",
        
        "avemaria_hoffnung": """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der in uns die christliche Hoffnung stärke. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen.""",
        
        "avemaria_liebe": """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der in uns die christliche Liebe entzünde. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen.""",
        
        "ehresei": """Ehre sei dem Vater und dem Sohn und dem Heiligen Geist, wie im Anfang, so auch jetzt und alle Zeit und in Ewigkeit. Amen.""",
        
        "fatimagebet": """O mein Jesus, verzeih uns unsere Sünden, bewahre uns vor dem Feuer der Hölle, führe alle Seelen in den Himmel, besonders jene, die deiner Barmherzigkeit am meisten bedürfen.""",
        
        "salveregina": """Sei gegrüßt, o Königin, Mutter der Barmherzigkeit; unser Leben, unsere Wonne und unsere Hoffnung, sei gegrüßt! <pause />
Zu dir rufen wir verbannte Kinder Evas; zu dir seufzen wir trauernd und weinend in diesem Tal der Tränen. <pause />
Wohlan denn, unsere Fürsprecherin, wende deine barmherzigen Augen uns zu, und nach diesem Elend zeige uns Jesus, die gebenedeite Frucht deines Leibes! O gütige, o milde, o süße Jungfrau Maria!"""
    }

    # Freudenreiche Geheimnisse (Joyful Mysteries)
    prayers["avemaria_freudenreich_1"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, den du, o Jungfrau, vom Heiligen Geist empfangen hast. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_freudenreich_2"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, den du, o Jungfrau, zu Elisabeth getragen hast. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_freudenreich_3"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, den du, o Jungfrau, geboren hast. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_freudenreich_4"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, den du, o Jungfrau, im Tempel aufgeopfert hast. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_freudenreich_5"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, den du, o Jungfrau, im Tempel wiedergefunden hast. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    # Schmerzhafte Geheimnisse (Sorrowful Mysteries)
    prayers["avemaria_schmerzhaft_1"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der für uns Blut geschwitzt hat. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_schmerzhaft_2"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der für uns gegeißelt worden ist. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_schmerzhaft_3"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der für uns mit Dornen gekrönt worden ist. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_schmerzhaft_4"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der für uns das schwere Kreuz getragen hat. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_schmerzhaft_5"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der für uns gekreuzigt worden ist. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unsdes Todes. Amen."""

    # Glorreiche Geheimnisse (Glorious Mysteries)
    prayers["avemaria_glorreich_1"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der von den Toten auferstanden ist. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_glorreich_2"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der in den Himmel aufgefahren ist. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_glorreich_3"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der uns den Heiligen Geist gesandt hat. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_glorreich_4"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der dich, o Jungfrau, in den Himmel aufgenommen hat. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    prayers["avemaria_glorreich_5"] = """Gegrüßet seist du, Maria, voll der Gnade, der Herr ist mit dir. Du bist gebenedeit unter den Weibern, und gebenedeit ist die Frucht deines Leibes, Jesus, der dich, o Jungfrau, im Himmel gekrönt hat. <pause />
Heilige Maria, Mutter Gottes, bitte für uns Sünder jetzt und in der Stunde unseres Todes. Amen."""

    # Add title cards
    prayers["titel_freudenreich"] = "Freudenreiche Geheimnisse\n(Montag und Samstag)"
    prayers["titel_schmerzhaft"] = "Schmerzhafte Geheimnisse\n(Dienstag und Freitag)"
    prayers["titel_glorreich"] = "Glorreiche Geheimnisse\n(Mittwoch und Sonntag)"

    return prayers


def get_mystery_displays() -> Dict[str, str]:
    """Return a dictionary of mystery display texts"""
    return {
        "freudenreich_1": "den du, o Jungfrau, vom Heiligen Geist empfangen hast",
        "freudenreich_2": "den du, o Jungfrau, zu Elisabeth getragen hast",
        "freudenreich_3": "den du, o Jungfrau, geboren hast",
        "freudenreich_4": "den du, o Jungfrau, im Tempel aufgeopfert hast",
        "freudenreich_5": "den du, o Jungfrau, im Tempel wiedergefunden hast",
        
        "schmerzhaft_1": "der für uns Blut geschwitzt hat",
        "schmerzhaft_2": "der für uns gegeißelt worden ist",
        "schmerzhaft_3": "der für uns mit Dornen gekrönt worden ist",
        "schmerzhaft_4": "der für uns das schwere Kreuz getragen hat",
        "schmerzhaft_5": "der für uns gekreuzigt worden ist",
        
        "glorreich_1": "der von den Toten auferstanden ist",
        "glorreich_2": "der in den Himmel aufgefahren ist",
        "glorreich_3": "der uns den Heiligen Geist gesandt hat",
        "glorreich_4": "der dich, o Jungfrau, in den Himmel aufgenommen hat",
        "glorreich_5": "der dich, o Jungfrau, im Himmel gekrönt hat"
    }


# Custom font CSS
CUSTOM_FONT_CSS = """
@font-face {
    font-family: 'Source Serif Pro';
    font-weight: 400;
    font-style: italic;
    src: url('https://dubia.cc/static/font/ssfp/SourceSerifPro-BASIC-RegularItalic.woff2') format('woff2');
    font-display: swap;
    unicode-range: U+0020-007E, U+00A0-00FF, U+2010, U+2013-2014, U+2018-2019, U+201C-201D, U+2212;
}
@font-face {
    font-family: 'Source Serif Pro';
    font-weight: 400;
    font-style: normal;
    src: url('https://dubia.cc/static/font/ssfp/SourceSerifPro-BASIC-Regular.woff2') format('woff2');
    font-display: swap;
    unicode-range: U+0020-007E, U+00A0-00FF, U+2010, U+2013-2014, U+2018-2019, U+201C-201D, U+2212;
}
@font-face {
    font-family: 'Source Serif Pro';
    font-weight: 600;
    font-style: normal;
    src: url('https://dubia.cc/static/font/ssfp/SourceSerifPro-BASIC-Semibold.woff2') format('woff2');
    font-display: swap;
    unicode-range: U+0020-007E, U+00A0-00FF, U+2010, U+2013-2014, U+2018-2019, U+201C-201D, U+2212;
}
"""


def create_html(text: str, filename: str, config: Config) -> str:
    """Create an HTML file with centered text"""
    html_path = f"target/html_files/{filename}.html"
    
    # If HTML already exists, return the path
    if os.path.exists(html_path):
        return html_path
        
    log(f"Creating HTML for: {filename}")
    
    # Split text into lines
    lines = text.split('\n')
    
    # Process text for HTML (escape special characters)
    processed_lines = []
    for line in lines:
        # Remove pause tags for display
        line = re.sub(r'<pause />', '', line)
        # Basic HTML escaping
        line = line.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;').replace('"', '&quot;')
        processed_lines.append(f"<p>{line}</p>")
    
    # Create the HTML content
    html_content = f'''<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{filename}</title>
    <style>
        {CUSTOM_FONT_CSS}
        
        body {{
            background-color: {config.bg_color};
            font-family: 'Source Serif Pro', serif;
            font-size: 40px;
            color: {config.text_color};
            margin: 0;
            padding: 0;
            width: {config.width}px;
            height: {config.height}px;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            text-align: center;
        }}
        
        .container {{
            max-width: 80%;
            padding: 20px;
        }}
        
        p {{
            margin: 15px 0;
            line-height: 1.5;
        }}
    </style>
</head>
<body>
    <div class="container">
        {"".join(processed_lines)}
    </div>
</body>
</html>'''
    
    # Write the HTML file
    with open(html_path, 'w', encoding='utf-8') as f:
        f.write(html_content)
    
    return html_path


def create_special_html(special_type: str, config: Config, mystery_type: Optional[str] = None) -> str:
    """Create special HTML files (intro, credits) with optional mystery type customization"""
    
    # Determine the file path based on whether a mystery type is provided
    if mystery_type:
        html_path = f"target/html_files/{special_type}_{mystery_type}.html"
    else:
        html_path = f"target/html_files/{special_type}.html"
    
    # If HTML already exists, return the path
    if os.path.exists(html_path):
        return html_path
        
    log(f"Creating {special_type} HTML" + (f" for {mystery_type}" if mystery_type else ""))
    
    if special_type == "intro_title":
        # Create different intro content based on mystery type
        title = "Der Rosenkranz"
        subtitle = "Eine Gebetsform der katholischen Tradition"
        
        if mystery_type == "freudenreich":
            subtitle = "Die Freudenreichen Geheimnisse"
        elif mystery_type == "schmerzhaft":
            subtitle = "Die Schmerzhaften Geheimnisse"
        elif mystery_type == "glorreich":
            subtitle = "Die Glorreichen Geheimnisse"

        html_content = f'''<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Rosenkranz Intro</title>
    <style>
        {CUSTOM_FONT_CSS}
        
        body {{
            background-color: {config.bg_color};
            font-family: 'Source Serif Pro', serif;
            margin: 0;
            padding: 0;
            width: {config.width}px;
            height: {config.height}px;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            text-align: center;
        }}
        
        .title {{
            font-size: 60px;
            color: {config.text_color};
            margin-bottom: 20px;
            font-weight: 600;
        }}
        
        .subtitle {{
            font-size: 30px;
            color: {config.text_color};
            margin-top: 20px;
        }}
        
        .credit {{
            font-size: 24px;
            color: {config.highlight_color};
            margin-top: 40px;
            font-style: italic;
        }}
    </style>
</head>
<body>
    <h1 class="title">{title}</h1>
    <p class="subtitle">{subtitle}</p>
    <p class="credit">dubia.cc</p>
</body>
</html>'''
    elif special_type == "credits":
        # Create different credits content based on mystery type
        title = "Der Rosenkranz"
        text1 = "Erstellt mit ElevenLabs Text-to-Speech"
        text2 = "Texte aus der katholischen Tradition"
        
        if mystery_type:
            if mystery_type == "freudenreich":
                text2 = "Die Freudenreichen Geheimnisse (Montag und Samstag)"
            elif mystery_type == "schmerzhaft":
                text2 = "Die Schmerzhaften Geheimnisse (Dienstag und Freitag)"
            elif mystery_type == "glorreich":
                text2 = "Die Glorreichen Geheimnisse (Mittwoch und Sonntag)"

        html_content = f'''<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Rosenkranz Credits</title>
    <style>
        {CUSTOM_FONT_CSS}
        
        body {{
            background-color: {config.bg_color};
            font-family: 'Source Serif Pro', serif;
            margin: 0;
            padding: 0;
            width: {config.width}px;
            height: {config.height}px;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            text-align: center;
        }}
        
        .title {{
            font-size: 40px;
            color: {config.text_color};
            margin-bottom: 30px;
            font-weight: 600;
        }}
        
        .text {{
            font-size: 30px;
            color: {config.text_color};
            margin: 15px 0;
        }}
        
        .links {{
            font-size: 25px;
            color: {config.highlight_color};
            margin-top: 40px;
        }}
    </style>
</head>
<body>
    <h1 class="title">{title}</h1>
    <p class="text">{text1}</p>
    <p class="text">{text2}</p>
    <p class="links">dubia.cc</p>
</body>
</html>'''
    else:
        log(f"Unknown special HTML type: {special_type}")
        return ""
    
    with open(html_path, 'w', encoding='utf-8') as f:
        f.write(html_content)
    
    return html_path


def create_mystery_html(mystery_type: str, number: int, config: Config) -> str:
    """Create HTML for a mystery with Ave Maria text and the mystery highlighted"""
    mystery_key = f"{mystery_type}_{number}"
    html_path = f"target/html_files/avemaria_{mystery_key}.html"
    
    # If HTML already exists, return the path
    if os.path.exists(html_path):
        return html_path
        
    log(f"Creating mystery HTML for: {mystery_key}")
    
    mystery_text = get_mystery_displays()[mystery_key]
    
    html_content = f'''<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Ave Maria - {mystery_key}</title>
    <style>
        {CUSTOM_FONT_CSS}
        
        body {{
            background-color: {config.bg_color};
            font-family: 'Source Serif Pro', serif;
            color: {config.text_color};
            margin: 0;
            padding: 0;
            width: {config.width}px;
            height: {config.height}px;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            text-align: center;
        }}
        
        .container {{
            max-width: 80%;
            padding: 20px;
        }}
        
        .prayer {{
            font-size: 38px;
            margin: 10px 0;
            line-height: 1.5;
        }}
        
        .mystery {{
            font-size: 42px;
            font-weight: 600;
            color: {config.highlight_color};
            margin: 25px 0;
        }}
    </style>
</head>
<body>
    <div class="container">
        <p class="prayer">Gegrüßet seist du, Maria, voll der Gnade,</p>
        <p class="prayer">der Herr ist mit dir.</p>
        <p class="prayer">Du bist gebenedeit unter den Weibern,</p>
        <p class="prayer">und gebenedeit ist die Frucht deines Leibes, Jesus,</p>
        <p class="mystery">{mystery_text}</p>
        <p class="prayer">Heilige Maria, Mutter Gottes,</p>
        <p class="prayer">bitte für uns Sünder jetzt und in der Stunde unseres Todes.</p>
        <p class="prayer">Amen.</p>
    </div>
</body>
</html>'''
    
    with open(html_path, 'w', encoding='utf-8') as f:
        f.write(html_content)
    
    return html_path


# ======= HTML TO PNG CONVERSION =======
def html_to_png(html_path: str, png_path: str, config: Config) -> bool:
    """Convert HTML file to PNG using available methods"""
    # If PNG already exists, early return
    if os.path.exists(png_path):
        log(f"PNG already exists: {os.path.basename(png_path)}")
        return True
        
    log(f"Converting HTML to PNG: {os.path.basename(html_path)} -> {os.path.basename(png_path)}")
    
    abs_html_path = os.path.abspath(html_path)
    abs_png_path = os.path.abspath(png_path)
    
    # Try different methods to convert HTML to PNG
    conversion_methods = [
        lambda: try_chrome_screenshot(find_chrome_executable(), abs_html_path, abs_png_path, config),
        lambda: try_puppeteer_screenshot(abs_html_path, abs_png_path, config),
        lambda: try_wkhtmltoimage(abs_html_path, abs_png_path, config)
    ]
    
    for method in conversion_methods:
        if method():
            return True
    
    error_msg = f"All methods failed to convert {html_path} to PNG."
    exit_on_error(config, error_msg)
    return False


def find_chrome_executable() -> str:
    """Find Chrome executable path based on platform"""
    system = platform.system()
    
    if system == "Darwin":  # macOS
        mac_chrome_paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "/Applications/Chromium.app/Contents/MacOS/Chromium"
        ]
        
        for path in mac_chrome_paths:
            if os.path.exists(path):
                log(f"Found Chrome on macOS at: {path}")
                return path
    
    # Default to 'google-chrome' on Linux/Windows or if macOS paths weren't found
    log("Using default Chrome command: google-chrome")
    return 'google-chrome'


def try_chrome_screenshot(chrome_cmd: str, abs_html_path: str, abs_png_path: str, config: Config) -> bool:
    """Try to use Chrome to create a screenshot"""
    command = [
        chrome_cmd, '--headless', '--disable-gpu',
        f'--window-size={config.width},{config.height}',
        f'--screenshot={abs_png_path}',
        f'file://{abs_html_path}'
    ]
    
    try:
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            stdout, stderr = process.communicate(timeout=3)  # 3 second timeout
            if process.returncode == 0:
                log("Chrome screenshot successful")
                return True
                
            log(f"Chrome screenshot failed with return code {process.returncode}")
            log(f"STDERR: {stderr.decode('utf-8', errors='replace')}")
            return False
        except subprocess.TimeoutExpired:
            process.kill()
            log(f"Chrome process timed out after 3 seconds")
            return False
    except Exception as e:
        log(f"Error running Chrome: {e}")
        return False


def try_puppeteer_screenshot(abs_html_path: str, abs_png_path: str, config: Config) -> bool:
    """Try to use Puppeteer to create a screenshot"""
    log("Trying with puppeteer-screenshot...")
    puppeteer_cmd = [
        'node', '-e',
        f'''
        const puppeteer = require('puppeteer');
        (async () => {{
          const browser = await puppeteer.launch();
          const page = await browser.newPage();
          await page.setViewport({{ width: {config.width}, height: {config.height} }});
          await page.goto('file://{abs_html_path}');
          await page.screenshot({{ path: '{abs_png_path}' }});
          await browser.close();
        }})();
        '''
    ]
    
    try:
        process = subprocess.Popen(puppeteer_cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            stdout, stderr = process.communicate(timeout=3)
            if process.returncode == 0:
                log("Puppeteer screenshot successful")
                return True
                
            log(f"Puppeteer failed with return code {process.returncode}")
            log(f"STDERR: {stderr.decode('utf-8', errors='replace')}")
            return False
        except subprocess.TimeoutExpired:
            process.kill()
            log(f"Puppeteer process timed out after 3 seconds")
            return False
    except Exception as e:
        log(f"Error running Puppeteer: {e}")
        return False


def try_wkhtmltoimage(abs_html_path: str, abs_png_path: str, config: Config) -> bool:
    """Try to use wkhtmltoimage to create a screenshot"""
    log("Trying with wkhtmltoimage...")
    wkhtml_cmd = ['wkhtmltoimage', abs_html_path, abs_png_path]
    
    try:
        process = subprocess.Popen(wkhtml_cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            stdout, stderr = process.communicate(timeout=3)
            if process.returncode == 0:
                log("wkhtmltoimage screenshot successful")
                return True
                
            log(f"wkhtmltoimage failed with return code {process.returncode}")
            log(f"STDERR: {stderr.decode('utf-8', errors='replace')}")
            return False
        except subprocess.TimeoutExpired:
            process.kill()
            log(f"wkhtmltoimage process timed out after 3 seconds")
            return False
    except Exception as e:
        log(f"Error running wkhtmltoimage: {e}")
        return False


# ======= AUDIO GENERATION =======
def create_silent_audio(duration: int = 3, config: Config = None) -> str:
    """Create a silent audio file of specified duration"""
    silent_audio = f"target/audio_files/silent_{duration}s.mp3"
    
    # If silent audio already exists, return the path
    if os.path.exists(silent_audio):
        return silent_audio
        
    log(f"Creating silent audio file ({duration}s)...")
    try:
        result = subprocess.run([
            'ffmpeg', '-y', '-f', 'lavfi', '-i', 'anullsrc=r=44100:cl=stereo', 
            '-t', str(duration), '-q:a', '9', '-acodec', 'libmp3lame', silent_audio
        ], check=True, capture_output=True)
        log("Created silent audio file")
        return silent_audio
    except subprocess.CalledProcessError as e:
        log(f"Could not create silent audio: {e}")
        if config and config.fail_on_error:
            exit_on_error(config, "Failed to create silent audio file")
        return ""


def generate_audio(text: str, filename: str, config: Config) -> str:
    """Generate audio from text using ElevenLabs API or fallback to silent audio"""
    # Check if file already exists
    audio_path = f"target/audio_files/{filename}.mp3"
    if os.path.exists(audio_path):
        log(f"Audio already exists: {audio_path}")
        return audio_path
    
    log(f"Generating audio for: {filename}")
    
    # Ensure the text ends with "Amen." if it should
    if text.rstrip().endswith("Todes."):
        text = text.rstrip() + " Amen."
        log(f"Fixed missing 'Amen' at the end of prayer")
    
    # Replace <pause /> tags with ElevenLabs break tags
    text_with_breaks = text.replace('<pause />', '<break time="0.7s" />')
    
    # Try to generate audio with ElevenLabs
    api_url = f"https://api.elevenlabs.io/v1/text-to-speech/{config.voice_id}"
    headers = {
        "Accept": "audio/mpeg",
        "Content-Type": "application/json",
        "xi-api-key": config.api_key
    }
    
    data = {
        "text": text_with_breaks,
        "model_id": "eleven_multilingual_v2",
        "voice_settings": {
            "stability": 0.75,
            "similarity_boost": 0.75,
            "style": 0.0,
            "use_speaker_boost": True,
            "speaking_rate": 0.85  # Slightly slower for prayers
        }
    }
    
    log(f"Sending API request to ElevenLabs for {filename}...")
    
    try:
        response = requests.post(api_url, json=data, headers=headers)
        
        if response.status_code == 200:
            with open(audio_path, "wb") as f:
                f.write(response.content)
            log(f"Successfully generated audio: {audio_path}")
            return audio_path
        else:
            error_message = ""
            try:
                error_data = response.json()
                error_message = str(error_data)
            except:
                error_message = response.text
                
            error_msg = f"Error generating {filename}: {response.status_code}\nError details: {error_message}"
            log(error_msg)
            
            if config.fail_on_error:
                exit_on_error(config, error_msg)
            
            # Fallback to silent audio if ElevenLabs fails
            silent_path = create_silent_audio(5, config)  # 5 seconds of silence
            if silent_path:
                log(f"Using silent audio as fallback for {filename}")
                os.makedirs(os.path.dirname(audio_path), exist_ok=True)
                subprocess.run(['cp', silent_path, audio_path], check=True)
                return audio_path
            return ""
            
    except Exception as e:
        error_msg = f"Exception while processing {filename}: {str(e)}"
        log(error_msg)
        
        if config.fail_on_error:
            exit_on_error(config, error_msg)
            
        # Fallback to silent audio if ElevenLabs fails
        silent_path = create_silent_audio(5, config)  # 5 seconds of silence
        if silent_path:
            log(f"Using silent audio as fallback for {filename}")
            os.makedirs(os.path.dirname(audio_path), exist_ok=True)
            subprocess.run(['cp', silent_path, audio_path], check=True)
            return audio_path
        return ""


def get_audio_duration(audio_path: str) -> float:
    """Get the duration of an audio file in seconds."""
    if not os.path.exists(audio_path):
        return 0
        
    try:
        result = subprocess.run(
            ['ffprobe', '-v', 'error', '-show_entries', 'format=duration', 
             '-of', 'default=noprint_wrappers=1:nokey=1', audio_path],
            capture_output=True, text=True, check=False
        )
        
        if result.returncode == 0:
            return float(result.stdout.strip())
        return 0
    except Exception:
        return 0


# ======= DIRECT VIDEO CREATION =======
class SegmentInfo(NamedTuple):
    """Information about a segment of the rosary"""
    name: str
    image_path: str
    audio_path: str
    duration: float


def prepare_segments(mystery_type: str, html_png_files: Dict[str, Tuple[str, str]], 
                      audio_files: Dict[str, str]) -> List[SegmentInfo]:
    """Prepare segment information for a mystery type"""
    segments = []
    
    # Add intro
    intro_key = f"intro_title_{mystery_type}" 
    if intro_key in html_png_files:
        image_path = html_png_files[intro_key][1]
        audio_path = "target/audio_files/silent_5s.mp3"
        duration = 5.0
        segments.append(SegmentInfo(intro_key, image_path, audio_path, duration))
    
    # Opening prayers
    opening_prayers = [
        "kreuzzeichen", 
        "apostolisches_glaubensbekenntnis_1",
        "apostolisches_glaubensbekenntnis_2", 
        "vaterunser", 
        "avemaria_glauben", 
        "avemaria_hoffnung", 
        "avemaria_liebe", 
        "ehresei"
    ]
    
    for prayer in opening_prayers:
        if prayer in html_png_files and prayer in audio_files:
            image_path = html_png_files[prayer][1]
            audio_path = audio_files[prayer]
            duration = get_audio_duration(audio_path)
            segments.append(SegmentInfo(prayer, image_path, audio_path, duration))
    
    # For each of the 5 mysteries
    for i in range(1, 6):
        mystery_key = f"{mystery_type}_{i}"
        
        # Add Our Father
        if "vaterunser" in html_png_files and "vaterunser" in audio_files:
            image_path = html_png_files["vaterunser"][1]
            audio_path = audio_files["vaterunser"]
            duration = get_audio_duration(audio_path)
            segments.append(SegmentInfo(f"vaterunser_{mystery_key}", image_path, audio_path, duration))
        
        # Add Ave Maria with mystery (10 times)
        ave_key = f"avemaria_{mystery_key}"
        if ave_key in html_png_files and ave_key in audio_files:
            image_path = html_png_files[ave_key][1]
            audio_path = audio_files[ave_key]
            duration = get_audio_duration(audio_path)
            
            for j in range(10):
                segments.append(SegmentInfo(f"{ave_key}_{j}", image_path, audio_path, duration))
        
        # Add Glory Be and Fatima Prayer
        for prayer in ["ehresei", "fatimagebet"]:
            if prayer in html_png_files and prayer in audio_files:
                image_path = html_png_files[prayer][1]
                audio_path = audio_files[prayer]
                duration = get_audio_duration(audio_path)
                segments.append(SegmentInfo(f"{prayer}_{mystery_key}", image_path, audio_path, duration))
    
    # Add Salve Regina
    if "salveregina" in html_png_files and "salveregina" in audio_files:
        image_path = html_png_files["salveregina"][1]
        audio_path = audio_files["salveregina"]
        duration = get_audio_duration(audio_path)
        segments.append(SegmentInfo("salveregina", image_path, audio_path, duration))
    
    # Add final Sign of the Cross
    if "kreuzzeichen" in html_png_files and "kreuzzeichen" in audio_files:
        image_path = html_png_files["kreuzzeichen"][1]
        audio_path = audio_files["kreuzzeichen"]
        duration = get_audio_duration(audio_path)
        segments.append(SegmentInfo("kreuzzeichen_final", image_path, audio_path, duration))
    
    # Add credits
    credits_key = f"credits_{mystery_type}"
    if credits_key in html_png_files:
        image_path = html_png_files[credits_key][1]
        audio_path = "target/audio_files/silent_7s.mp3"
        duration = 7.0
        segments.append(SegmentInfo(credits_key, image_path, audio_path, duration))
    
    return segments


def create_image_slideshow(segments: List[SegmentInfo], output_path: str, config: Config) -> str:
    """Create a silent video slideshow from images with precise timing"""
    if os.path.exists(output_path):
        log(f"Silent video already exists: {output_path}")
        return output_path
    
    log(f"Creating silent video slideshow: {output_path}")
    
    # Create a text file with image durations
    temp_dir = os.path.dirname(output_path)
    image_list_path = f"{temp_dir}/images_list_{os.path.basename(output_path).split('.')[0]}.txt"
    
    with open(image_list_path, 'w') as f:
        for segment in segments:
            f.write(f"file '{os.path.abspath(segment.image_path)}'\n")
            f.write(f"duration {segment.duration}\n")
        
        # Add the last image again (required by ffmpeg)
        if segments:
            f.write(f"file '{os.path.abspath(segments[-1].image_path)}'\n")
    
    # Create the slideshow
    cmd = [
        'ffmpeg', '-y', '-v', 'warning',
        '-f', 'concat', '-safe', '0', '-i', image_list_path,
        '-c:v', 'libx264', '-pix_fmt', 'yuv420p',
        '-r', '30',  # 30 fps
        output_path
    ]
    
    cmdc = " ".join(cmd)
    log(f"command: {cmdc}")

    try:
        subprocess.run(cmd, check=True, capture_output=True)
        log(f"Successfully created silent video: {output_path}")
        
        # Verify the video was created properly
        if not os.path.exists(output_path) or os.path.getsize(output_path) == 0:
            log("Silent video creation failed - file is empty or missing")
            return ""
        
        return output_path
    except subprocess.CalledProcessError as e:
        log(f"Error creating silent video: {e}")
        return ""


def create_audio_file(segments: List[SegmentInfo], output_path: str, config: Config) -> str:
    """Create a single audio file by concatenating all segment audios"""
    if os.path.exists(output_path):
        log(f"Audio file already exists: {output_path}")
        return output_path
    
    log(f"Creating concatenated audio: {output_path}")
    
    # Create a file listing all audio files
    temp_dir = os.path.dirname(output_path)
    audio_list_path = f"{temp_dir}/audio_list_{os.path.basename(output_path).split('.')[0]}.txt"
    
    with open(audio_list_path, 'w') as f:
        for segment in segments:
            f.write(f"file '{os.path.abspath(segment.audio_path)}'\n")
    
    # Create the concatenated audio
    cmd = [
        'ffmpeg', '-y', '-v', 'warning',
        '-f', 'concat', '-safe', '0', '-i', audio_list_path,
        '-c:a', 'libmp3lame', '-b:a', '192k',  # Use MP3 for broader compatibility
        output_path
    ]
    
    try:
        subprocess.run(cmd, check=True, capture_output=True)
        log(f"Successfully created audio: {output_path}")
        
        # Verify the audio was created properly
        if not os.path.exists(output_path) or os.path.getsize(output_path) == 0:
            log("Audio creation failed - file is empty or missing")
            return ""
        
        # Check audio duration
        total_expected_duration = sum(segment.duration for segment in segments)
        actual_duration = get_audio_duration(output_path)
        log(f"Expected audio duration: {total_expected_duration:.2f}s, Actual: {actual_duration:.2f}s")
        
        return output_path
    except subprocess.CalledProcessError as e:
        log(f"Error creating audio: {e}")
        return ""


def combine_video_audio(video_path: str, audio_path: str, output_path: str, config: Config) -> str:
    """Combine silent video with audio track"""
    if os.path.exists(output_path):
        log(f"Final video already exists: {output_path}")
        return output_path
    
    log(f"Combining video and audio into: {output_path}")
    
    # Verify input files exist
    if not os.path.exists(video_path):
        log(f"Video file not found: {video_path}")
        return ""
    
    if not os.path.exists(audio_path):
        log(f"Audio file not found: {audio_path}")
        return ""
    
    # Combine video and audio
    cmd = [
        'ffmpeg', '-y', '-v', 'warning',
        '-i', video_path,  # Video input
        '-i', audio_path,  # Audio input
        '-map', '0:v',     # Use video from first input
        '-map', '1:a',     # Use audio from second input
        '-c:v', 'copy',    # Copy video stream without re-encoding
        '-c:a', 'aac',     # Convert audio to AAC (good for mp4)
        '-b:a', '192k',    # Audio bitrate
        '-shortest',       # End when shortest input ends
        output_path
    ]
    
    try:
        subprocess.run(cmd, check=True, capture_output=True)
        log(f"Successfully created final video: {output_path}")
        return output_path
    except subprocess.CalledProcessError as e:
        log(f"Error combining video and audio: {e}")
        
        # Try alternative approach with re-encoding
        log("Trying alternative approach with video re-encoding...")
        alt_cmd = [
            'ffmpeg', '-y', '-v', 'warning',
            '-i', video_path,
            '-i', audio_path,
            '-map', '0:v', '-map', '1:a',
            '-c:v', 'libx264', '-pix_fmt', 'yuv420p',  # Re-encode video
            '-c:a', 'aac', '-b:a', '192k',
            '-shortest',
            output_path
        ]
        
        try:
            subprocess.run(alt_cmd, check=True, capture_output=True)
            log(f"Successfully created final video with re-encoding: {output_path}")
            return output_path
        except subprocess.CalledProcessError as e2:
            log(f"Error with alternative approach: {e2}")
            return ""


def create_mystery_video(mystery_type: str, html_png_files: Dict[str, Tuple[str, str]], 
                        audio_files: Dict[str, str], config: Config) -> str:
    """Create a complete video for one mystery set using the direct approach"""
    output_path = f"target/final_output/{mystery_type}_complete.mp4"
    
    # Prepare segment information
    segments = prepare_segments(mystery_type, html_png_files, audio_files)
    
    if not segments:
        log(f"No segments found for {mystery_type}")
        return ""
    
    # Step 1: Create silent video
    silent_video_path = f"target/silent_videos/{mystery_type}_silent.mp4"
    silent_video = create_image_slideshow(segments, silent_video_path, config)
    
    if not silent_video:
        log(f"Failed to create silent video for {mystery_type}")
        return ""
    
    # Step 2: Create audio file
    audio_path = f"target/final_audio/{mystery_type}_audio.mp3"
    audio_file = create_audio_file(segments, audio_path, config)
    
    if not audio_file:
        log(f"Failed to create audio for {mystery_type}")
        return ""
    
    # Step 3: Combine video and audio
    return combine_video_audio(silent_video, audio_file, output_path, config)


def combine_mystery_videos(mystery_videos: List[str], output_path: str, config: Config) -> str:
    """Combine all mystery videos into one complete rosary video"""
    if os.path.exists(output_path):
        log(f"Final video already exists: {output_path}")
        return output_path
    
    if not mystery_videos or len(mystery_videos) == 0:
        log("No mystery videos to combine")
        return ""
    
    log(f"Combining {len(mystery_videos)} mystery videos into {output_path}")
    
    # The simplest approach: extract all videos and audios, then recombine them
    # Use a temporary directory for extracted streams
    temp_dir = os.path.dirname(output_path)
    extracted_videos = []
    extracted_audios = []
    
    for i, video in enumerate(mystery_videos):
        video_file = f"{temp_dir}/video_{i}.mp4"
        audio_file = f"{temp_dir}/audio_{i}.mp3"
        
        # Extract video without audio
        video_cmd = [
            'ffmpeg', '-y', '-v', 'warning',
            '-i', video,
            '-c:v', 'copy', '-an',
            video_file
        ]
        
        # Extract audio without video
        audio_cmd = [
            'ffmpeg', '-y', '-v', 'warning',
            '-i', video,
            '-vn', '-c:a', 'libmp3lame', '-b:a', '192k',
            audio_file
        ]
        
        try:
            log(f"Extracting video from {os.path.basename(video)}...")
            subprocess.run(video_cmd, check=True, capture_output=True)
            extracted_videos.append(video_file)
            
            log(f"Extracting audio from {os.path.basename(video)}...")
            subprocess.run(audio_cmd, check=True, capture_output=True)
            extracted_audios.append(audio_file)
        except subprocess.CalledProcessError as e:
            log(f"Error extracting streams from {video}: {e}")
    
    if not extracted_videos or not extracted_audios:
        log("Failed to extract streams from mystery videos")
        return ""
    
    # Create file lists for concat
    video_list = f"{temp_dir}/video_list.txt"
    audio_list = f"{temp_dir}/audio_list.txt"
    
    with open(video_list, 'w') as f:
        for video in extracted_videos:
            f.write(f"file '{os.path.abspath(video)}'\n")
    
    with open(audio_list, 'w') as f:
        for audio in extracted_audios:
            f.write(f"file '{os.path.abspath(audio)}'\n")
    
    # Concatenate videos
    concat_video = f"{temp_dir}/concat_video.mp4"
    video_cmd = [
        'ffmpeg', '-y', '-v', 'warning',
        '-f', 'concat', '-safe', '0', '-i', video_list,
        '-c:v', 'copy',
        concat_video
    ]
    
    # Concatenate audios
    concat_audio = f"{temp_dir}/concat_audio.mp3"
    audio_cmd = [
        'ffmpeg', '-y', '-v', 'warning',
        '-f', 'concat', '-safe', '0', '-i', audio_list,
        '-c:a', 'libmp3lame', '-b:a', '192k',
        concat_audio
    ]
    
    try:
        log("Concatenating videos...")
        subprocess.run(video_cmd, check=True, capture_output=True)
        
        log("Concatenating audios...")
        subprocess.run(audio_cmd, check=True, capture_output=True)
    except subprocess.CalledProcessError as e:
        log(f"Error concatenating streams: {e}")
        return ""
    
    # Combine concatenated video and audio
    return combine_video_audio(concat_video, concat_audio, output_path, config)


# ======= MAIN FUNCTION =======
def main() -> None:
    """Main function to create the German Rosary videos using direct method"""
    log("=== German Rosary Generator - Direct Method ===")
    
    # Parse arguments
    config = parse_arguments()
    log(f"API Key: {'*' * 5}{config.api_key[-3:] if config.api_key else 'Not provided'}")
    log(f"Voice ID: {config.voice_id}")
    log(f"Fail on error: {config.fail_on_error}")
    
    # Ensure directories exist
    ensure_dirs()
    
    # Phase 1: Generate HTML and PNG files
    log("\n=== PHASE 1: Generating HTML and PNG files ===")
    html_png_files = {}
    prayers = get_prayers()
    
    # Generate common HTML and PNG files
    for name, text in prayers.items():
        log(f"Processing prayer: {name}")
        html_path = create_html(text, name, config)
        png_path = f"target/png_files/{name}.png"
        if html_to_png(html_path, png_path, config):
            html_png_files[name] = (html_path, png_path)
    
    # Generate mystery HTML and PNG files
    for mystery_type in ["freudenreich", "schmerzhaft", "glorreich"]:
        for i in range(1, 6):
            mystery_key = f"{mystery_type}_{i}"
            log(f"Processing mystery: {mystery_key}")
            html_path = create_mystery_html(mystery_type, i, config)
            png_path = f"target/png_files/avemaria_{mystery_key}.png"
            if html_to_png(html_path, png_path, config):
                html_png_files[f"avemaria_{mystery_key}"] = (html_path, png_path)
    
    # Create intro and credits for each mystery type
    for mystery_type in ["freudenreich", "schmerzhaft", "glorreich"]:
        for special_type in ["intro_title", "credits"]:
            html_path = create_special_html(special_type, config, mystery_type)
            png_path = f"target/png_files/{special_type}_{mystery_type}.png"
            if html_to_png(html_path, png_path, config):
                html_png_files[f"{special_type}_{mystery_type}"] = (html_path, png_path)
    
    # Create general intro and credits
    for special_type in ["intro_title", "credits"]:
        html_path = create_special_html(special_type, config)
        png_path = f"target/png_files/{special_type}.png"
        if html_to_png(html_path, png_path, config):
            html_png_files[special_type] = (html_path, png_path)
    
    log(f"Successfully generated {len(html_png_files)} HTML/PNG files")
    
    # Phase 2: Generate audio files
    log("\n=== PHASE 2: Generating audio files ===")
    audio_files = {}
    
    # Generate audio for common prayers
    common_prayers = [
        "kreuzzeichen", 
        "apostolisches_glaubensbekenntnis_1",
        "apostolisches_glaubensbekenntnis_2", 
        "vaterunser", 
        "avemaria_glauben", 
        "avemaria_hoffnung", 
        "avemaria_liebe", 
        "ehresei",
        "fatimagebet",
        "salveregina"
    ]
    
    for prayer in common_prayers:
        if prayer in prayers:
            audio_path = generate_audio(prayers[prayer], prayer, config)
            if audio_path:
                audio_files[prayer] = audio_path
    
    # Generate audio for all mysteries
    for mystery_type in ["freudenreich", "schmerzhaft", "glorreich"]:
        for i in range(1, 6):
            mystery_key = f"{mystery_type}_{i}"
            audio_key = f"avemaria_{mystery_key}"
            if audio_key in prayers:
                audio_path = generate_audio(prayers[audio_key], audio_key, config)
                if audio_path:
                    audio_files[audio_key] = audio_path
    
    # Create silent audio files for intro and credits
    create_silent_audio(5, config)  # 5 seconds for intro
    create_silent_audio(7, config)  # 7 seconds for credits
    
    # Phase 3: Create individual mystery videos
    log("\n=== PHASE 3: Creating individual mystery videos ===")
    mystery_videos = []
    
    # Create freudenreiche (joyful) mysteries video
    log("\n--- Creating Freudenreiche (Joyful) Mysteries Video ---")
    freudenreich_video = create_mystery_video("freudenreich", html_png_files, audio_files, config)
    if freudenreich_video:
        mystery_videos.append(freudenreich_video)
    
    # Create schmerzhafte (sorrowful) mysteries video
    log("\n--- Creating Schmerzhafte (Sorrowful) Mysteries Video ---")
    schmerzhaft_video = create_mystery_video("schmerzhaft", html_png_files, audio_files, config)
    if schmerzhaft_video:
        mystery_videos.append(schmerzhaft_video)
    
    # Create glorreiche (glorious) mysteries video
    log("\n--- Creating Glorreiche (Glorious) Mysteries Video ---")
    glorreich_video = create_mystery_video("glorreich", html_png_files, audio_files, config)
    if glorreich_video:
        mystery_videos.append(glorreich_video)
    
    # Phase 4: Combine all mystery videos into one complete rosary video
    log("\n=== PHASE 4: Creating complete rosary video ===")
    complete_path = "target/final_output/rosenkranz_komplett.mp4"
    complete_video = combine_mystery_videos(mystery_videos, complete_path, config)
    
    # Summary
    log("\n=== PROCESS COMPLETE ===")
    log("Output files:")
    
    if complete_video:
        log(f"1. Complete rosary: {complete_video}")
    
    if freudenreich_video:
        log(f"2. Joyful mysteries: {freudenreich_video}")
    
    if schmerzhaft_video:
        log(f"3. Sorrowful mysteries: {schmerzhaft_video}")
    
    if glorreich_video:
        log(f"4. Glorious mysteries: {glorreich_video}")


if __name__ == "__main__":
    main()