# Sourced by render.sh: a browser for Remotion, a bundle, and the checks.
# Sourced, not run.

# A browser: $REMOTION_BROWSER, else a Playwright headless shell if this
# machine has one, else Remotion downloads its own.
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

# Fails the script if the first and last frames of a master sequence differ.
seamless() {
  local masters=("$1"/*.png)
  pixels() { ffmpeg -v error -i "$1" -f rawvideo -pix_fmt rgb24 - | md5sum; }
  if [[ "$(pixels "${masters[0]}")" != "$(pixels "${masters[-1]}")" ]]; then
    echo "error: $1 does not loop — the first and last frames differ" >&2
    exit 1
  fi
}

# Prints a file's size and fails the script if it is over the budget in bytes.
within() {
  local size
  size=$(stat -c %s "$1")
  printf '  %-30s %7.1f KB\n' "$1" "$(echo "$size / 1000" | bc -l)"
  if ((size > $2)); then
    echo "error: $1 is over its $(($2 / 1000)) KB budget" >&2
    exit 1
  fi
}
