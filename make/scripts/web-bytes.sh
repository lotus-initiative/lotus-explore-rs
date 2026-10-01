#!/usr/bin/env bash
# One number per transfer encoding, so a profile experiment is comparable with
# the one before it instead of eyeballed. `raw` is what the linker emitted,
# `br` is the sibling dx pre-compresses, `gzip -9` is the fallback for hosts
# without brotli. Only the bundle we actually ship is measured: no source maps.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
out="target/dx/lotus-explore-rs/release/web/public"
if [ ! -f "$out/index.html" ]; then
  echo "no build at $out -- run 'cargo make web-build' first" >&2
  exit 1
fi
# `dx build` leaves a superseded bundle beside the current one, which inflates
# the table. Prune before measuring, not after: a number that needs a second run
# to become correct eventually gets misread.
for asset in "$out"/assets/*.js "$out"/assets/*.wasm "$out"/assets/*.css; do
  [ -f "$asset" ] || continue
  case "$asset" in *.br) continue ;; esac
  base=$(basename "$asset")
  grep -qF "$base" "$out/index.html" || rm -f "$asset" "$asset.br"
done
kb() { echo $(( $1 / 1024 )); }
# `stat` is spelled differently on macOS and Linux; this is a macOS-first
# workspace but CI measures the bundle on Linux too. `-f %z` first because
# GNU `stat` accepts `-f` too, with unrelated meaning, so the order matters.
size() { stat -f%z "$1" 2>/dev/null || stat -c%s "$1"; }
printf '%-34s %10s %10s %10s\n' asset raw_KiB br_KiB gzip_KiB
tot_raw=0; tot_br=0; tot_gz=0
for file in "$out"/assets/*.wasm "$out"/assets/*.js "$out"/assets/*.css "$out/index.html"; do
  [ -f "$file" ] || continue
  raw=$(size "$file")
  if [ -f "$file.br" ]; then br=$(size "$file.br"); else br=$raw; fi
  gz=$(gzip -c9 "$file" | wc -c | tr -d ' ')
  printf '%-34s %10s %10s %10s\n' "$(basename "$file")" "$(kb $raw)" "$(kb $br)" "$(kb $gz)"
  tot_raw=$(( tot_raw + raw )); tot_br=$(( tot_br + br )); tot_gz=$(( tot_gz + gz ))
done
printf '%-34s %10s %10s %10s\n' TOTAL "$(kb $tot_raw)" "$(kb $tot_br)" "$(kb $tot_gz)"
printf '\nwasm raw bytes: %s\n' "$(size "$out"/assets/*.wasm)"
