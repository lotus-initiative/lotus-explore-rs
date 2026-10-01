#!/usr/bin/env bash
# Recompile the application stylesheet.
#
# The compiled sheet is committed, because `asset!` reads it at compile time and
# a checkout without it cannot build the app. That makes it possible for it to go
# stale, so a test in `document_head.rs` fails if a theme token is missing from
# it. Run this after editing `tailwind/styles.css`, and commit the result.
#
# Uses the Tailwind binary dx installed, not npm: dx owns the version, and a
# second copy of the compiler is a second answer to which utilities exist.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
tw=$(ls -d "$HOME"/.dx/tools/tailwindcss-*/tailwindcss 2>/dev/null | sort -V | tail -1 || true)
if [ -z "$tw" ] || [ ! -x "$tw" ]; then
  echo "no tailwind binary under ~/.dx/tools; run 'cargo make web-build' once to install it" >&2
  exit 1
fi
cd apps/lotus-explore-rs
"$tw" --input tailwind/styles.css --output public/assets/lotus-explore.css
