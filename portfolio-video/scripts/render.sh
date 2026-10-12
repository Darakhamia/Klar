#!/usr/bin/env bash
# Renders the portfolio loop into out/:
#
#   klar-16x9.mp4 / .webm    1920 × 1080, 30 fps, H.264 / VP9, silent
#   klar-1x1.mp4  / .webm    1080 × 1080, the same story recomposed
#   klar-poster.png          the "Done" moment, 16:9
#   klar-poster-1x1.png      the same moment, square
#
# Remotion draws lossless PNG masters; ffmpeg encodes them, so the colour
# conversion is stated rather than left to defaults: BT.709, tagged, or the
# #f3f2f2 ground drifts once a browser decodes it.

set -euo pipefail
cd "$(dirname "$0")/.."

# Rate factors, lowered until each file sits just inside the budget. Lower is
# sharper and larger. Override from the environment to experiment; with
# ENCODE_ONLY=1 the masters already in build/ are reused rather than redrawn.
H264_CRF=${H264_CRF:-16}
VP9_CRF=${VP9_CRF:-20}
BUDGET=5000000 # bytes, per file
POSTER=270     # T.poster in src/timeline.ts

# A browser for Remotion: $REMOTION_BROWSER, else a Playwright headless shell
# if this machine has one, else Remotion downloads its own.
browser=()
if [[ -n "${REMOTION_BROWSER:-}" ]]; then
  browser=(--browser-executable="$REMOTION_BROWSER")
else
  shell=$(compgen -G "${PLAYWRIGHT_BROWSERS_PATH:-/opt/pw-browsers}/chromium_headless_shell-*/*/headless_shell" | head -1 || true)
  if [[ -n "$shell" ]]; then browser=(--browser-executable="$shell"); fi
fi

# Bundled outside the project: the app's `eslint .` would otherwise lint
# Remotion's generated JavaScript.
bundle=$(mktemp -d)
trap 'rm -rf "$bundle"' EXIT
npx remotion bundle src/index.ts --out-dir "$bundle" --log=error >/dev/null

colour=(-colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv)
convert="scale=out_color_matrix=bt709:out_range=tv:flags=lanczos+accurate_rnd+full_chroma_int,format=yuv420p"

mkdir -p build out
for pair in "Klar16x9 klar-16x9 klar-poster" "Klar1x1 klar-1x1 klar-poster-1x1"; do
  read -r id name poster <<<"$pair"
  frames="build/$name"
  if [[ "${ENCODE_ONLY:-}" != 1 || ! -d "$frames" ]]; then
    rm -rf "$frames"
    echo "▸ $id: rendering masters"
    npx remotion render "$bundle" "$id" "$frames" --sequence --image-format=png \
      --image-sequence-pattern='[frame].[ext]' "${browser[@]}" --log=error
  fi

  masters=("$frames"/*.png)
  pixels() { ffmpeg -v error -i "$1" -f rawvideo -pix_fmt rgb24 - | md5sum; }
  if [[ "$(pixels "${masters[0]}")" != "$(pixels "${masters[-1]}")" ]]; then
    echo "error: $id does not loop — the first and last frames differ" >&2
    exit 1
  fi
  cp "${masters[$POSTER]}" "out/$poster.png"

  echo "▸ $id: encoding"
  input=(-framerate 30 -pattern_type glob -i "$frames/*.png" -vf "$convert")
  ffmpeg -v error -y "${input[@]}" -c:v libx264 -preset veryslow -tune animation \
    -crf "$H264_CRF" -profile:v high "${colour[@]}" -movflags +faststart -an "out/$name.mp4"
  vp9=(-c:v libvpx-vp9 -b:v 0 -crf "$VP9_CRF" -deadline good -cpu-used 1 -row-mt 1
    -tile-columns 2 -auto-alt-ref 1 -lag-in-frames 25 -passlogfile "build/$name-vp9")
  ffmpeg -v error -y "${input[@]}" "${vp9[@]}" -pass 1 -an -f webm /dev/null
  ffmpeg -v error -y "${input[@]}" "${vp9[@]}" -pass 2 "${colour[@]}" -an "out/$name.webm"

  for file in "out/$name.mp4" "out/$name.webm"; do
    size=$(stat -c %s "$file")
    printf '  %-22s %5.2f MB\n' "$file" "$(echo "$size / 1000000" | bc -l)"
    if ((size > BUDGET)); then
      echo "error: $file is over the 5 MB budget; raise the CRF" >&2
      exit 1
    fi
  done
done
