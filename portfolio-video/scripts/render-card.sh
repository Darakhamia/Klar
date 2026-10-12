#!/usr/bin/env bash
# Renders the portfolio's portrait card into out/card/:
#
#   klar-card.png / .webp           1200 × 1600 still — the Done moment
#   klar-card-loop.mp4 / .webm      900 × 1200, 8 s, silent, seamless loop
#
# The carousel shows an unfocused card's top half only (object-position
# 50% 26%, so y 208–1008 of the still); src/scenes/CardScene.tsx keeps
# everything that matters inside that band.

set -euo pipefail
cd "$(dirname "$0")/.."

H264_CRF=${H264_CRF:-14}
VP9_CRF=${VP9_CRF:-18}
STILL=190 # CARD_STILL in src/timeline.ts

source scripts/common.sh

mkdir -p build out/card

echo "▸ KlarCard: still"
npx remotion still "$bundle" KlarCard out/card/klar-card.png --frame="$STILL" \
  --image-format=png "${browser[@]}" --log=error
# Lossless keeps #f3f2f2 exact; lossy only if lossless will not fit.
ffmpeg -v error -y -i out/card/klar-card.png -c:v libwebp -lossless 1 \
  -compression_level 6 out/card/klar-card.webp
if (($(stat -c %s out/card/klar-card.webp) > 300000)); then
  ffmpeg -v error -y -i out/card/klar-card.png -c:v libwebp -quality 92 \
    -preset picture out/card/klar-card.webp
fi
within out/card/klar-card.png 100000000
within out/card/klar-card.webp 300000

echo "▸ KlarCard: loop"
frames=build/klar-card
rm -rf "$frames"
npx remotion render "$bundle" KlarCard "$frames" --sequence --image-format=png --scale=0.75 \
  --image-sequence-pattern='[frame].[ext]' "${browser[@]}" --log=error
seamless "$frames"

input=(-framerate 30 -pattern_type glob -i "$frames/*.png" -vf "$convert")
ffmpeg -v error -y "${input[@]}" -c:v libx264 -preset veryslow -tune animation \
  -crf "$H264_CRF" -profile:v high "${colour[@]}" -movflags +faststart -an out/card/klar-card-loop.mp4
vp9=(-c:v libvpx-vp9 -b:v 0 -crf "$VP9_CRF" -deadline good -cpu-used 1 -row-mt 1
  -tile-columns 1 -auto-alt-ref 1 -lag-in-frames 25 -passlogfile build/klar-card-vp9)
ffmpeg -v error -y "${input[@]}" "${vp9[@]}" -pass 1 -an -f webm /dev/null
ffmpeg -v error -y "${input[@]}" "${vp9[@]}" -pass 2 "${colour[@]}" -an out/card/klar-card-loop.webm
within out/card/klar-card-loop.mp4 2000000
within out/card/klar-card-loop.webm 2000000
