#!/usr/bin/env bash
# Check that a built macOS bundle can actually answer everything the app asks
# it for.
#
# `dioxus-desktop` serves a request out of the bundle only when the path starts
# with `/assets/`, joining it onto `Contents/Resources`. Anything else is looked
# up on disk relative to the working directory and missed. That rule is why
# RDKit and Ketcher 404ed in the window while working in a browser, and nothing
# in the test suite could see it: the assets were absent, and the paths were
# only ever assembled at runtime.
#
# So this walks the real bundle: the icon the plist names, every folder asset
# the app can request, and Ketcher's own relative references.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

profile="${1:-debug}"
root="target/dx/lotus-explore-rs/$profile"
if [ ! -d "$root" ]; then
  echo "no build at $root -- run './mk desktop' first" >&2
  exit 1
fi

# The bundle layout is the platform's, and it is not worth guessing: a macOS
# build is `macos/App.app/Contents/Resources/assets`, a Linux build is
# `linux/<name>/assets`, and the name is whatever the package is called.
# Guessing is what made this check useless -- it reported "the assets are
# missing" on every Linux run while the assets were present, in a directory it
# had decided not to look in.
#
# So: find the directory that actually holds the editor, which is the only thing
# this script is really about -- that the URLs the app can request resolve. And
# say which one it used, so a failure is diagnosable from the log alone.
resources=""
for candidate in "$root"/macos/*.app/Contents/Resources \
                 $(find "$root" -maxdepth 3 -type d -name assets 2>/dev/null | sed 's|/assets$||' | sort); do
  if [ -f "$candidate/assets/ketcher/index.html" ]; then
    resources="$candidate"
    break
  fi
done
if [ -z "$resources" ]; then
  echo "FAIL  no bundle under $root contains assets/ketcher/index.html" >&2
  echo "      dx put the build at: $(find "$root" -maxdepth 2 -type d 2>/dev/null | head -20)" >&2
  echo "      and these assets exist: $(find "$root" -maxdepth 4 -name 'RDKit_minimal.js' 2>/dev/null | head -3)" >&2
  exit 1
fi
echo "ok    bundle resources: $resources"

# The plist names an icon whether or not one was configured, and Finder shows a
# generic icon when the named file is not there. Only macOS has a plist.
case "$(uname -s)" in
Darwin)
  bundle="$(dirname "$(dirname "$resources")")"
  icon=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIconFile' "$bundle/Contents/Info.plist" 2>/dev/null || true)
  if [ -n "$icon" ]; then
    if [ -f "$resources/$icon" ]; then
      echo "ok    icon: $icon"
    else
      echo "FAIL  icon: plist names $icon, which is not in Contents/Resources"
      fail=1
    fi
  fi
  ;;
*)
  echo "skip  icon: $(uname -s) has no bundle plist to check it against"
  ;;
esac
fail=0

# The URLs the app actually requests, not just the files that exist. These two
# were different for a long time: the bundle had every file, and `bundled_path`
# still produced `.../index.html/rdkit/RDKit_minimal.js`, which the resolver does
# not serve. Checking the filesystem could not see that; checking the shape of
# the URL can.
for f in assets/rdkit/RDKit_minimal.js \
         assets/rdkit/RDKit_minimal.wasm \
         assets/ketcher/index.html; do
  if [ -f "$resources/$f" ]; then
    echo "ok    $f"
  else
    echo "FAIL  $f is requested at runtime and is not in the bundle"
    fail=1
  fi
done

# The resolver serves out of the bundle only for a rooted `/assets/` path.
# `vendor_assets::asset_url_for` is what guarantees that, and its test pins the
# shape; this pins that the shape still matches the real bundle.
if [ -d "$resources/assets/rdkit" ] && [ -d "$resources/assets/ketcher" ]; then
  echo "ok    assets/ layout matches the URLs vendor_assets generates"
else
  echo "FAIL  the bundle is not laid out as /assets/rdkit and /assets/ketcher"
  fail=1
fi

# Ketcher loads its chunks by relative path, so the bundle is only correct if
# those resolve inside it too. The paths are rewritten at runtime for a desktop
# frame, so this is about the files, which is what the rewrite resolves against.
while read -r ref; do
  [ -n "$ref" ] || continue
  if [ -f "$resources/assets/ketcher/${ref#./}" ]; then
    echo "ok    ketcher ref $ref"
  else
    echo "FAIL  ketcher/index.html references $ref, which is not in the bundle"
    fail=1
  fi
done < <(grep -oE '(src|href)="\./[^"]+"' "$resources/assets/ketcher/index.html" \
         | sed -E 's/^[a-z]+="//; s/"$//' | sort -u)

exit "$fail"
