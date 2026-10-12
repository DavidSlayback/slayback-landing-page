#!/usr/bin/env bash
# Encode recorded frames into GIF (600px/20fps; temporal denoise strips per-frame grain so GIF compresses) and MP4 (full 960x540, H.264).
set -euo pipefail
src=${1:-frames}; out=${2:-out}
mkdir -p "$out"
for d in "$src"/*/; do
  name=$(basename "$d")
  ffmpeg -loglevel error -y -framerate 30 -i "$d/f%04d.png" \
    -vf "fps=20,scale=600:-1:flags=lanczos,hqdn3d=0:0:8:8,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
    "$out/$name.gif"
  ffmpeg -loglevel error -y -framerate 30 -i "$d/f%04d.png" \
    -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p -movflags +faststart "$out/$name.mp4"
  echo "$name: $(du -h "$out/$name.gif" | cut -f1) gif, $(du -h "$out/$name.mp4" | cut -f1) mp4"
done
