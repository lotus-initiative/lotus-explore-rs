# Root task runner for the lotus-explore-rs repository.
# Run `just --list` to see available recipes.

# ── Workspace gate (mirrors .github/workflows/ci.yml) ─────────────────────────

fmt:
	cargo fmt --all -- --check

check:
	cargo check --workspace --all-targets --locked

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

test:
	cargo test --workspace --all-targets --locked --quiet

doc:
	cargo doc --workspace --no-deps --locked

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

ci:
	just fmt
	just check
	just clippy
	just test
	just doc
	just license-headers
	just metadata
	just wasm
	just clippy-wasm
	just machete
	just audit
	just deny
	just opt-levels
	just readme

# The optimisation level is declared in three places, and `dx` passes
# `--rustc-args=-Copt-level=` last, so the justfile silently wins over the
# `[profile.release]` it is supposed to match. That drift shipped once: the
# recipes said `s` while the profile said `z`, and every `just build` produced a
# module 185300 raw / 38433 brotli bytes larger than intended, with nothing in
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

preview app="lotus-explore-rs":
	cd apps/{{app}} && cargo run --locked -p lotus-web-assets --bin fetch-assets
	cd apps/{{app}} && BROWSERSLIST='chrome >= 100, firefox >= 100, safari >= 15' dx serve --package {{app}} --platform web --release --debug-symbols=false --locked --rustc-args=-Copt-level=z --open=false

build app="lotus-explore-rs":
	cd apps/{{app}} && cargo run --locked -p lotus-web-assets --bin fetch-assets
	cd apps/{{app}} && BROWSERSLIST='chrome >= 100, firefox >= 100, safari >= 15' dx build --release --package {{app}} --locked --debug-symbols=false --rustc-args=-Copt-level=z
	just preload-wasm

# The module is content-hashed, so only a post-build step can name it. Without
# this the 1.4 MiB module is fetched *after* the 45 KiB JS glue has downloaded,
# parsed and executed; measured, that serialisation cost ~120 ms of LCP.
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
	  echo "no build at $out — run 'just build' first" >&2
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

# README sync: regenerate each crate README from README.tpl + source `//!` doc
# comments, lint, and diff against the checked-in README.md. A pre-push hook.
#
# `$d` is single-dollar because just interpolates `$name` before the shell sees
# it, so `$$d` became a literal `$d`: `cd: 68631d: No such file or directory`.
# The diff compares formatted output because a pre-commit hook reformats the
# file after `cargo readme` writes it, and raw output reported drift that does
# not exist.
readme:
	#!/usr/bin/env bash
	set -euo pipefail
	command -v cargo-readme >/dev/null 2>&1 || { echo "cargo-readme not installed; skipping"; exit 0; }
	command -v panache >/dev/null 2>&1 || { echo "panache not installed; skipping"; exit 0; }
	# Any crate with a README.tpl has its README generated from its lib.rs docs
	# and committed. A crate with no template has no README and is skipped, which
	# is why this globs for templates rather than naming crates -- and why the
	# no-templates case is a pass rather than a failure.
	found=0
	for template in crates/*/README.tpl; do
	  [ -e "$template" ] || continue
	  dir=$(dirname "$template")
	  found=1
	  if ! ( cd "$dir" \
	         && cargo readme -t README.tpl -o /tmp/readme_panache.md 2>/dev/null \
	         && panache format /tmp/readme_panache.md >/dev/null \
	         && panache lint /tmp/readme_panache.md \
	         && diff -q /tmp/readme_panache.md README.md >/dev/null 2>&1 ); then
	    echo "README.md out of date for $dir" >&2
	    echo "  run: (cd $dir && cargo readme -t README.tpl -o README.md && panache format README.md)" >&2
	    exit 1
	  fi
	done
	[ "$found" = 1 ] || echo "no crate READMEs to check"
