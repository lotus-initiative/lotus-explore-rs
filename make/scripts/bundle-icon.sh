#!/usr/bin/env bash
# Copy the icon into a built macOS bundle.
#
# Done here rather than through `Dioxus.toml` because `dx` accepts a `resources`
# entry and does not put it where the plist looks, so a configured icon and a
# Finder icon were two different files. Only macOS has a plist, and only macOS
# has an `.icns`; the other platforms take their icon from the desktop file,
# which `dx` writes itself.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
case "$(uname -s)" in
Darwin)
  for profile in debug release; do
    bundle="target/dx/lotus-explore-rs/$profile/macos/LotusExploreRs.app"
    if [ -d "$bundle" ]; then
      cp apps/lotus-explore-rs/public/icon.icns "$bundle/Contents/Resources/icon.icns"
      echo "installed icon into $bundle"
    fi
  done
  ;;
*)
  echo "skip: $(uname -s) takes its icon from the desktop file dx writes"
  ;;
esac
