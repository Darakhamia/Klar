#!/usr/bin/env bash
# Renders the promo into out/:
#
#   klar-16x9.mp4 / .webm    1920 × 1080, 30 fps, H.264 High / VP9, silent
#   klar-1x1.mp4  / .webm    1080 × 1080, the same beats recomposed
#   klar-poster.png          the pill at Done, 16:9
#   klar-poster-1x1.png      the same moment, square
#
# Needs the captures first (`npm run capture`). Remotion draws lossless PNG
# masters at full size; ffmpeg encodes them, with the colour conversion stated
# rather than left to defaults: BT.709, tagged, or the #f3f2f2 ground drifts
# once a browser decodes it. Every file is checked with ffprobe at the end.

set -euo pipefail
cd "$(dirname "$0")/.."

if [[ ! -f public/captures/manifest.json ]]; then
  echo "error: no captures — run \`npm run capture\` first" >&2
  exit 1
fi

# H.264 rate factor: 18 is the brief's sharpest; raise it (up to 20) only if a
# file comes out over budget. VP9 is two-pass constrained quality.
H264_CRF=${H264_CRF:-18}
VP9_CRF=${VP9_CRF:-28}
BUDGET=8000000 # bytes, per file
POSTER=300     # POSTER in src/timeline.ts

source scripts/common.sh

mkdir -p build out
for pair in "Klar16x9 klar-16x9 klar-poster" "Klar1x1 klar-1x1 klar-poster-1x1"; do
  read -r id name poster <<<"$pair"
  frames="build/$name"
  if [[ "${ENCODE_ONLY:-}" != 1 || ! -d "$frames" ]]; then
    rm -rf "$frames"
    echo "▸ $id: rendering masters"
    npx remotion render "$bundle" "$id" "$frames" --sequence --image-format=png \
      --image-sequence-pattern='[frame].[ext]' --scale=1 "${browser[@]}" --log=error
  fi

  seamless "$frames"
  masters=("$frames"/*.png)
  cp "${masters[$POSTER]}" "out/$poster.png"

  echo "▸ $id: encoding"
  input=(-framerate 30 -pattern_type glob -i "$frames/*.png" -vf "$convert")
  ffmpeg -v error -y "${input[@]}" -c:v libx264 -preset slow -tune animation \
    -crf "$H264_CRF" -profile:v high -pix_fmt yuv420p "${colour[@]}" \
    -movflags +faststart -an "out/$name.mp4"
  vp9=(-c:v libvpx-vp9 -b:v 0 -crf "$VP9_CRF" -deadline good -cpu-used 1 -row-mt 1
    -tile-columns 2 -auto-alt-ref 1 -lag-in-frames 25 -pix_fmt yuv420p
    -passlogfile "build/$name-vp9")
  ffmpeg -v error -y "${input[@]}" "${vp9[@]}" -pass 1 -an -f webm /dev/null
  ffmpeg -v error -y "${input[@]}" "${vp9[@]}" -pass 2 "${colour[@]}" -an "out/$name.webm"

  within "out/$name.mp4" "$BUDGET"
  within "out/$name.webm" "$BUDGET"
done

echo "▸ ffprobe"
for file in out/klar-16x9.mp4 out/klar-1x1.mp4 out/klar-16x9.webm out/klar-1x1.webm; do
  ffprobe -v error -select_streams v:0 \
    -show_entries stream=codec_name,profile,width,height,pix_fmt,r_frame_rate:format=duration,bit_rate,size \
    -of default=noprint_wrappers=1 "$file" | paste -sd' ' | sed "s|^|  $file: |"
done
for file in out/klar-poster.png out/klar-poster-1x1.png; do
  ffprobe -v error -show_entries stream=width,height -of csv=p=0 "$file" | sed "s|^|  $file: |"
done
