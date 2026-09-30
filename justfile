# Root task runner for the lotus-explore-rs repository.
# Run `just --list` to see available recipes.

# ── Workspace gate (mirrors .github/workflows/ci.yml) ─────────────────────────

fmt:
	cargo fmt --all -- --check

check:
	cargo check --workspace --all-targets --locked

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

# The app is three programs: a browser client, a native window and an HTTP API.
# `--workspace` only builds the first, because the other two need a feature. That
# gap is how `--features server` sat broken on main while this recipe passed: the
# API had a renamed request field its own tests still used, and nothing here
# compiled it. Each line below is also a job in CI; keep them in step.
test:
	cargo test --workspace --all-targets --locked --quiet
	cargo test -p lotus-explore-rs --features server --locked --quiet
	cargo test -p lotus-explore-rs --features desktop --all-targets --locked --quiet

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

# Mutation testing. `cargo mutants` rewrites one expression at a time and
# re-runs the tests: a mutant that survives is a behaviour the suite does not
# actually pin down, which a passing test run cannot tell you. Coverage counts
# executed lines, this checks that they are asserted on.
#
# Scoped to the three pure-logic crates. The full workspace is dominated by the
# app's rendering and IO plumbing, where a surviving mutant is usually a
# `write!` format string; these crates build the SPARQL and the answers, where a
# surviving mutant is a wrong query sent to Wikidata.
# Every library in the workspace. This started as three packages and was grown
# one missed mutant at a time, which is backwards: the point of mutation testing
# is that you do not know where the tests are thin until it says so.
#
# `apps/lotus-explore-rs` is excluded, deliberately and with a reason. It is 2,761
# mutants of Dioxus rendering -- a `match` arm on a `Signal`, a `rswap!`, a
# literal in a `class` attribute -- and killing those means rendering tests for
# every component. That is a large piece of work with a poor ratio, so it is
# `just mutants-ui`, opt-in, and the exclusion is written down in
# `mutants.toml` rather than being an omission.
mutants:
	cargo mutants --package lotus-model --package lotus-search --package lotus-query --package lotus-curation --package lotus-jsonld --package lotus-cli --package lotus-web-assets --jobs 8 --timeout 300

# The excluded UI crate, run on request. See the note above.
mutants-ui:
	cargo mutants --package lotus-explore-rs --jobs 8 --timeout 300

# Same scope, listing the mutants without running them. Use this to see what a
# change added before paying for the run.
mutants-list:
	cargo mutants --package lotus-query --package lotus-curation --package lotus-jsonld --list

# Every .rs file must carry both AGPL-3.0-only headers on lines 1 and 2.
# `target/`, `.opencode/` and `graphify-out/` are build output, vendored config
# and generated graph data respectively — none of them is source.
license-headers:
	#!/usr/bin/env bash
	set -euo pipefail
	missing=0
	while IFS= read -r f; do
	  if ! head -1 "$f" | grep -qxF '// SPDX-License-Identifier: AGPL-3.0-only' \
	     || ! head -2 "$f" | tail -1 | grep -qxF '// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project'; then
	    echo "missing or misplaced SPDX header: $f" >&2
	    missing=1
	  fi
	done < <(find . \( -name target -o -name .opencode -o -name graphify-out -o -name .git \) -prune -o -name '*.rs' -print)
	exit $missing

# ── Full CI gate (every check the pipeline runs, in order) ────────────────────
# `just ci`. Each step reuses a recipe above (single source of truth). Supply-chain
# tools that may be absent locally are skipped by their own recipes.

# The committed citation metadata must match what the code describes. A version
# bump that forgets to regenerate it would otherwise publish a stale citation.
metadata:
	cargo run --locked -q -p lotus-jsonld --bin emit-metadata -- --check

# Rewrite the committed citation metadata from the single description in
# `lotus-jsonld`.
metadata-write:
	cargo run --locked -q -p lotus-jsonld --bin emit-metadata

# The gate. Every recipe here is one CI job, in CI's order, and
# `tests/gate_consistency.rs` fails the build if that stops being true -- which is
# the only reason the two lists have not drifted apart already.
#
# Two jobs are deliberately not here, because they need a network, a toolchain
# this repo does not vendor, or minutes rather than seconds: `mutants` and
# `desktop`. They are `just ci-slow`, and CI runs both.
ci:
	just fmt
	just license-headers
	just check
	just clippy
	just clippy-native-builds
	just test
	just doc
	just wasm
	just clippy-wasm
	just machete
	just deny
	just audit
	just metadata
	just opt-levels

# The two CI jobs `just ci` leaves out, and why.
#
# `mutants` is non-blocking in CI for a documented reason -- survivors are a
# to-do list, not a gate -- and takes about four minutes. `desktop` needs the
# fetched assets and a `dx` build, so it cannot run on a checkout that has not
# fetched them.
ci-slow:
	just mutants
	just desktop

# The `desktop` job in CI: compile the one target nothing else builds, then
# prove the bundle can answer what the app asks of it. Every desktop-only bug so
# far -- an unstyled window, a no-op download, a 404 for the toolkit -- compiled
# cleanly, because nothing built this target.
desktop:
	cargo clippy -p lotus-explore-rs --features desktop --locked -- -D warnings
	cargo test -p lotus-explore-rs --features desktop --all-targets --locked --quiet
	cargo run --locked -p lotus-web-assets --bin fetch-assets
	cd apps/lotus-explore-rs && dx build --package lotus-explore-rs --desktop --locked --features desktop
	just verify-bundle
	just bundle-icon

# The optimisation level is declared in three places, and `dx` passes
# `--rustc-args=-Copt-level=` last, so the justfile silently wins over the
# `[profile.release]` it is supposed to match. That drift shipped once: the
# one build said `s` while the profile said `z`, and every release bundle was
# 185300 raw / 38433 brotli bytes larger than intended, with nothing in
# CI noticing. Assert the three agree.
opt-levels:
	#!/usr/bin/env bash
	set -euo pipefail
	profile=$(sed -n 's/^opt-level = "\(.\)"/\1/p' Cargo.toml | head -1)
	wasmopt=$(sed -n '/^\[web.wasm_opt\]/,/^\[/ s/^level = "\(.\)"/\1/p' apps/lotus-explore-rs/Dioxus.toml | head -1)
	# Only real `dx` invocations, so this recipe cannot match its own source.
	levels=$(grep -E 'dx (build|serve)' justfile | grep -o -- '-Copt-level=.' | sed 's/.*=//' | sort -u | tr '\n' ' ')
	printf 'Cargo.toml [profile.release]  opt-level = %s\n' "$profile"
	printf 'Dioxus.toml [web.wasm_opt]   level     = %s\n' "$wasmopt"
	printf 'justfile --rustc-args                  = %s\n' "${levels% }"
	fail=0
	[ -n "$profile" ] || { echo "MISMATCH: no opt-level found in Cargo.toml" >&2; fail=1; }
	[ "$profile" = "$wasmopt" ] || { echo "MISMATCH: wasm_opt level '$wasmopt' != profile opt-level '$profile'" >&2; fail=1; }
	[ "$(printf '%s' "$levels" | wc -w | tr -d ' ')" -eq 1 ] || { echo "MISMATCH: justfile passes more than one -Copt-level: $levels" >&2; fail=1; }
	[ "${levels% }" = "$profile" ] || { echo "MISMATCH: justfile -Copt-level='${levels% }' != profile '$profile'" >&2; fail=1; }
	exit $fail

# One `cargo check -p <app>` per app keeps the wasm build green.
# The four pure crates are checked for wasm *in their own right*, not only
# because the app pulls them in: that is what keeps them free of IO. The app
# would fail to build if any of them reached for a clock or a socket, but the
# reverse is not true -- a crate can compile as part of the app while being
# untestable in isolation, and the boundary is the point.
wasm:
	cargo check --target wasm32-unknown-unknown --locked \
		-p lotus-model -p lotus-query -p lotus-jsonld -p lotus-curation
	cargo check -p lotus-search --no-default-features --target wasm32-unknown-unknown --locked
	cargo check -p lotus-explore-rs --target wasm32-unknown-unknown --locked

# Per-package WASM clippy (NOT `--workspace --target wasm32`: `lotus-web-assets`
# is a host-only bin — `reqwest::blocking` cannot exist on wasm — so a
# workspace-wide wasm lint can never pass; only wasm-relevant crates are
# linted here). Mirrors the Clippy-WASM step in .github/workflows/ci.yml.
# The two native programs are different lint surfaces: each compiles module code
# written for the other, so they are linted on their own for the same reason as
# in `test`.
clippy-native-builds:
	cargo clippy -p lotus-explore-rs --features server --locked -- -D warnings
	cargo clippy -p lotus-explore-rs --features desktop --locked -- -D warnings

clippy-wasm:
	cargo clippy --target wasm32-unknown-unknown --locked \
		-p lotus-model -p lotus-query -p lotus-jsonld -p lotus-curation -- -D warnings
	# `lotus-search`'s default features are its `reqwest` client, and a
	# `reqwest::Response` is not `Send` on wasm. The pure half is what has to be
	# wasm-clean, so that is what gets linted here; the default build is linted
	# for wasm in the app, which does use it.
	cargo clippy -p lotus-search --no-default-features --target wasm32-unknown-unknown \
		--locked -- -D warnings
	cargo clippy --target wasm32-unknown-unknown -p lotus-explore-rs --locked -- -D warnings

# ── Per-app dev servers / production builds ───────────────────────────────────
# fetch-assets must run from the app crate dir so its relative asset
# directories land inside apps/<app>/public, not at the repo-root public/.

# The dev server's payload is dominated by debug info, not by code. Measured on
# this workspace, dev wasm, same source:
#
#   (default)                    63.6 MiB   build 42s
#   -Copt-level=1                60.1 MiB   build 14s
#   -Cdebuginfo=0                37.5 MiB   build 12s
#   -Cdebuginfo=0 -Cstrip=debuginfo  6.4 MiB   build  6s
#
# Optimising barely moves it; emitting the debug sections is the whole cost, and
# writing them is also most of the build time. Stripping them is a 10x smaller
# payload and a 7x faster dev build, for the loss of line numbers in panic
# backtraces. Neither flag changes codegen, so hot reload is unaffected.
#
# Never measure Lighthouse against this: it is a dev server serving an
# unhashed bundle and the Dioxus JS interpreter, not the shipped artefact. See
# docs/PERFORMANCE.md.
serve app="lotus-explore-rs":
	cd apps/{{app}} && cargo run --locked -p lotus-web-assets --bin fetch-assets
	dx serve --package {{app}} --platform web --locked --open=false --rustc-args="-Cdebuginfo=0 -Cstrip=debuginfo"

# Dioxus supplies a different entry point per renderer and two of them cannot be
# enabled at once, so the renderer is chosen by a feature. Without `--features
# desktop` a native build has no user interface and exits immediately.
# A native window, for working on the app without a browser in the loop.
serve-desktop app="lotus-explore-rs":
	cd apps/{{app}} && cargo run --locked -p lotus-web-assets --bin fetch-assets
	cd apps/{{app}} && dx serve --package {{app}} --desktop --locked --features desktop --open=false

preview app="lotus-explore-rs":
	cd apps/{{app}} && cargo run --locked -p lotus-web-assets --bin fetch-assets
	cd apps/{{app}} && BROWSERSLIST='chrome >= 100, firefox >= 100, safari >= 15' dx serve --package {{app}} --platform web --release --debug-symbols=false --locked --rustc-args=-Copt-level=z --open=false

build app="lotus-explore-rs":
	cd apps/{{app}} && cargo run --locked -p lotus-web-assets --bin fetch-assets
	cd apps/{{app}} && BROWSERSLIST='chrome >= 100, firefox >= 100, safari >= 15' dx build --release --package {{app}} --locked --debug-symbols=false --rustc-args=-Copt-level=z
	just preload-wasm

# The macOS application icon, generated from the same artwork the web app uses.
#
# `dx` writes `CFBundleIconFile = icon.icns` into every macOS bundle whether or
# not one is configured, so a bundle built without this file has a plist naming
# a file that is not there, and Finder shows a generic icon. `iconutil` needs a
# full iconset -- ten sizes, half of them at 2x -- so it is generated rather than
# committed, and the mistake it prevents is a `.icns` that looks right and is
# missing a size.
app-icon:
	#!/usr/bin/env bash
	set -euo pipefail
	src=apps/lotus-explore-rs/public/apple-touch-icon.png
	iconset=$(mktemp -d)/lotus.iconset
	mkdir -p "$iconset"
	for size in 16 32 128 256 512; do
	  sips -z $size $size "$src" --out "$iconset/icon_${size}x${size}.png" >/dev/null
	  sips -z $((size * 2)) $((size * 2)) "$src" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
	done
	iconutil -c icns "$iconset" -o apps/lotus-explore-rs/public/icon.icns
	echo "wrote apps/lotus-explore-rs/public/icon.icns"

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
verify-bundle app="LotusExploreRs" profile="debug":
	#!/usr/bin/env bash
	set -euo pipefail
	root="target/dx/lotus-explore-rs/{{profile}}"
	if [ ! -d "$root" ]; then
	  echo "no build at $root -- run 'dx build --desktop --features desktop' first" >&2
	  exit 1
	fi

	# The bundle layout is the platform's, and it is not worth guessing: a macOS
	# build is `macos/App.app/Contents/Resources/assets`, a Linux build is
	# `linux/<name>/assets`, and the name is whatever the package is called. Guessing
	# is what made this check useless -- it reported "the assets are missing" on
	# every Linux run while the assets were present, in a directory it had decided
	# not to look in.
	#
	# So: find the directory that actually holds the editor, which is the only thing
	# this recipe is really about -- that the URLs the app can request resolve. And
	# say which one it used, so a failure is diagnosable from the log alone.
	resources=""
	for candidate in "$root/macos/{{app}}.app/Contents/Resources" $(find "$root" -maxdepth 3 -type d -name assets 2>/dev/null | sed 's|/assets$||' | sort); do
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
	# not serve. Checking the filesystem could not see that; checking the shape of the
	# URL can.
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
	done < <(rg -o '(src|href)="\./[^"]+"' "$resources/assets/ketcher/index.html" \
	         | sed -E 's/^[a-z]+="//; s/"$//' | sort -u)

	exit "$fail"

# Copy the icon into a built macOS bundle.
#
# Done here rather than through `Dioxus.toml` because `dx` accepts a `resources`
# entry and does not put it where the plist looks, so a configured icon and a
# Finder icon were two different files.
bundle-icon app="LotusExploreRs":
	#!/usr/bin/env bash
	set -euo pipefail
	# `dx` accepts a `resources` entry and does not put it where the plist looks,
	# so a configured icon and a Finder icon were two different files. Only macOS
	# has a plist, and only macOS has an `.icns`; the other platforms take their icon
	# from the desktop file, which `dx` writes itself.
	case "$(uname -s)" in
	Darwin)
	  for profile in debug release; do
	    bundle="target/dx/lotus-explore-rs/$profile/macos/{{app}}.app"
	    if [ -d "$bundle" ]; then
	      cp apps/lotus-explore-rs/public/icon.icns "$bundle/Contents/Resources/icon.icns"
	      echo "installed icon into $bundle"
	    fi
	  done
	  ;;
	*)
	  echo "skip  icon: $(uname -s) takes its icon from the desktop file dx writes"
	  ;;
	esac

# The module is content-hashed, so only a post-build step can name it. Without
# this the 1.4 MiB module is fetched *after* the 45 KiB JS glue has downloaded,
# parsed and executed; measured, that serialisation cost ~120 ms of LCP.
# Recompile the application stylesheet.
#
# The compiled sheet is committed, because `asset!` reads it at compile time and
# a checkout without it cannot build the app. That makes it possible for it to go
# stale, so `tests` in `document_head.rs` fails if a theme token is missing from
# it. Run this after editing `tailwind/styles.css`, and commit the result.
#
# Uses the Tailwind binary dx installed, not npm: dx owns the version, and a
# second copy of the compiler is a second answer to which utilities exist.
css:
	#!/usr/bin/env bash
	set -euo pipefail
	tw=$(ls -d "$HOME"/.dx/tools/tailwindcss-*/tailwindcss 2>/dev/null | sort -V | tail -1 || true)
	if [ -z "$tw" ] || [ ! -x "$tw" ]; then
	  echo "no tailwind binary under ~/.dx/tools; run 'dx build' once to install it" >&2
	  exit 1
	fi
	cd apps/lotus-explore-rs
	"$tw" --input tailwind/styles.css --output public/assets/lotus-explore.css

preload-wasm:
	cargo run --locked --release -p lotus-web-assets --bin inject-wasm-preload

# One number per transfer encoding, so a profile experiment is comparable with
# the one before it instead of eyeballed. `raw` is what the linker emitted,
# `br` is the sibling dx pre-compresses, `gzip -9` is the fallback for hosts
# without brotli. Only the bundle we actually ship is measured: no source maps.
web-bytes:
	#!/usr/bin/env bash
	set -euo pipefail
	out="target/dx/lotus-explore-rs/release/web/public"
	if [ ! -f "$out/index.html" ]; then
	  echo "no build at $out — run 'dx build --release --platform web' in apps/lotus-explore-rs first" >&2
	  exit 1
	fi
	# `dx build` leaves a superseded bundle beside the current one, which
	# inflates the table. Prune before measuring, not after: a number that
	# needs a second run to become correct eventually gets misread.
	for asset in "$out"/assets/*.js "$out"/assets/*.wasm "$out"/assets/*.css; do
	  [ -f "$asset" ] || continue
	  case "$asset" in *.br) continue ;; esac
	  base=$(basename "$asset")
	  grep -qF "$base" "$out/index.html" || rm -f "$asset" "$asset.br"
	done
	kb() { echo $(( $1 / 1024 )); }
	printf '%-34s %10s %10s %10s\n' asset raw_KiB br_KiB gzip_KiB
	tot_raw=0; tot_br=0; tot_gz=0
	for file in "$out"/assets/*.wasm "$out"/assets/*.js "$out"/assets/*.css "$out"/index.html; do
	  [ -f "$file" ] || continue
	  raw=$(stat -f%z "$file")
	  if [ -f "$file.br" ]; then br=$(stat -f%z "$file.br"); else br=$raw; fi
	  gz=$(gzip -c9 "$file" | wc -c | tr -d ' ')
	  printf '%-34s %10s %10s %10s\n' "$(basename "$file")" "$(kb $raw)" "$(kb $br)" "$(kb $gz)"
	  tot_raw=$(( tot_raw + raw )); tot_br=$(( tot_br + br )); tot_gz=$(( tot_gz + gz ))
	done
	printf '%-34s %10s %10s %10s\n' TOTAL "$(kb $tot_raw)" "$(kb $tot_br)" "$(kb $tot_gz)"
	printf '\nwasm raw bytes: %s\n' "$(stat -f%z "$out"/assets/*.wasm)"

# ── Supply-chain hygiene (skip gracefully if a tool is not installed) ─────────

machete:
	@command -v cargo-machete >/dev/null 2>&1 && cargo machete || echo "cargo-machete not installed; skipping"

audit:
	@command -v cargo-audit >/dev/null 2>&1 && cargo audit || echo "cargo-audit not installed; skipping"

deny:
	@command -v cargo-deny >/dev/null 2>&1 && cargo deny check advisories bans licenses sources || echo "cargo-deny not installed; skipping"

outdated:
	@command -v cargo-outdated >/dev/null 2>&1 && cargo outdated --workspace --exit-code 1 || echo "cargo-outdated not installed; skipping"

# Crate READMEs are the crate docs.
#
# Each library crate's README.md is included with `#![doc = include_str!]`, so its
# Rust example runs as a doctest and `cargo test` is what keeps it honest. There is
# no generator: the README is the source, and regenerating it from `//!` comments
# would put the two in a loop.
