# video-cutter

Scripts to automatically cut and dub long speeches to the important parts.

```sh
# Analyze summarized subtitles and extract original speech pacing
python preprocessing.py \
    --outline outline.json \
    --transcript transcript.json \
    --video input.mp4 \
    --output-dir output

# Optional: translate audio into target language
python translate_dub.py \
  --subtitles output/subtitles.json \
  --output-dir output \
  --output-audio dubbed.mp3 \
  --target-language German \
  --claude-api-key YOUR_CLAUDE_API_KEY \
  --eleven-api-key YOUR_ELEVENLABS_API_KEY \
  --voice-id VOICE_ID \
  --speaking-rate 1.1

# Render video with subtitles and audio
python renderer.py \
  --subtitles output/subtitles.json \
  --output-dir output \
  --output-video final.mp4
```

The second step is optional and uses Claude for translating the
video and then ElevenLabs to dub the audio
