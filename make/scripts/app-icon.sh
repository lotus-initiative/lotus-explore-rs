#!/usr/bin/env bash
# The macOS application icon, generated from the same artwork the web app uses.
#
# `dx` writes `CFBundleIconFile = icon.icns` into every macOS bundle whether or
# not one is configured, so a bundle built without this file has a plist naming
# a file that is not there, and Finder shows a generic icon. `iconutil` needs a
# full iconset -- ten sizes, half of them at 2x -- so it is generated rather than
# committed, and the mistake it prevents is an `.icns` that looks right and is
# missing a size.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
if [ "$(uname -s)" != "Darwin" ]; then
  echo "skip: .icns is a macOS icon format and this is $(uname -s)"
  exit 0
fi
src=apps/lotus-explore-rs/public/apple-touch-icon.png
iconset=$(mktemp -d)/lotus.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z $size $size "$src" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) "$src" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o apps/lotus-explore-rs/public/icon.icns
echo "wrote apps/lotus-explore-rs/public/icon.icns"
