#!/bin/bash
# From the Hyperframes 4K master to the files to post: poster baked in as frame 0, the mix
# brought to about -16 LUFS, one encode per output (1080p is a Lanczos downscale of the 4K).
set -euo pipefail
cd "$(dirname "$0")/.."
AUD="volume=8.5dB,alimiter=limit=0.8:attack=2:release=60:level=false"
mkdir -p work/check
ffmpeg -v error -y -ss 2.9 -i work/master-4k.mp4 -frames:v 1 work/check/poster-4k.png
ffmpeg -v error -y -i work/check/poster-4k.png -vf "scale=1920:1080:flags=lanczos" -q:v 2 brag.jpg
ffmpeg -v error -y -i work/master-4k.mp4 -i work/check/poster-4k.png \
  -filter_complex "[0:v][1:v]overlay=0:0:enable='eq(n,0)',scale=1920:1080:flags=lanczos+accurate_rnd+full_chroma_int[v];[0:a]${AUD}[a]" \
  -map "[v]" -map "[a]" -c:v libx264 -preset slower -crf 12 -profile:v high -level 4.2 -pix_fmt yuv420p \
  -x264-params aq-mode=3 -c:a aac -b:a 320k -movflags +faststart brag.mp4
ffmpeg -v error -y -i work/master-4k.mp4 -i work/check/poster-4k.png \
  -filter_complex "[0:v][1:v]overlay=0:0:enable='eq(n,0)'[v];[0:a]${AUD}[a]" \
  -map "[v]" -map "[a]" -c:v libx264 -preset slower -crf 14 -profile:v high -pix_fmt yuv420p \
  -x264-params aq-mode=3 -c:a aac -b:a 320k -movflags +faststart brag-4k.mp4
