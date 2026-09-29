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

# ── Full CI gate (every check the pipeline runs, in order) ────────────────────
# `just ci`. Each step reuses a recipe above (single source of truth). Supply-chain
# tools that may be absent locally are skipped by their own recipes.

ci:
	just fmt
	just check
	just clippy
	just test
	just doc
	just wasm
	just clippy-wasm
	just machete
	just audit
	just deny
	just opt-levels

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
wasm:
	cargo check -p lotus-explore-rs --target wasm32-unknown-unknown --locked

# Per-package WASM clippy (NOT `--workspace --target wasm32`: `lotus-deploy`
# is a host-only bin — `reqwest::blocking` cannot exist on wasm — so a
# workspace-wide wasm lint can never pass; only wasm-relevant crates are
# linted here). Mirrors the Clippy-WASM step in .github/workflows/ci.yml.
clippy-wasm:
	cargo clippy --target wasm32-unknown-unknown -p lotus -p lotus-explore-rs --locked -- -D warnings

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
	cd apps/{{app}} && cargo run --locked -p lotus-deploy --bin fetch-assets
	dx serve --package {{app}} --platform web --locked --open=false --rustc-args="-Cdebuginfo=0 -Cstrip=debuginfo"

preview app="lotus-explore-rs":
	cd apps/{{app}} && cargo run --locked -p lotus-deploy --bin fetch-assets
	cd apps/{{app}} && BROWSERSLIST='chrome >= 100, firefox >= 100, safari >= 15' dx serve --package {{app}} --platform web --release --debug-symbols=false --locked --rustc-args=-Copt-level=z --open=false

build app="lotus-explore-rs":
	cd apps/{{app}} && cargo run --locked -p lotus-deploy --bin fetch-assets
	cd apps/{{app}} && BROWSERSLIST='chrome >= 100, firefox >= 100, safari >= 15' dx build --release --package {{app}} --locked --debug-symbols=false --rustc-args=-Copt-level=z
	just preload-wasm

# The module is content-hashed, so only a post-build step can name it. Without
# this the 1.4 MiB module is fetched *after* the 45 KiB JS glue has downloaded,
# parsed and executed; measured, that serialisation cost ~120 ms of LCP.
preload-wasm:
	cargo run --locked --release -p lotus-deploy --bin inject-wasm-preload

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
	# `dx build` does not prune a hashed asset whose name changed, so a rebuilt
	# tree can hold a superseded bundle next to the current one. Summing both
	# inflates the table, so say so rather than let the number be misread.
	glue=$(ls "$out"/assets/lotus-explore-rs-dxh*.js 2>/dev/null | wc -l | tr -d ' ')
	if [ "$glue" -gt 1 ]; then
	  printf 'NOTE: %s app bundles present; only the one index.html names is loaded.\n' "$glue"
	  printf '      Delete target/dx/lotus-explore-rs to measure a clean tree.\n'
	fi

# ── Supply-chain hygiene (skip gracefully if a tool is not installed) ─────────

machete:
	@command -v cargo-machete >/dev/null 2>&1 && cargo machete || echo "cargo-machete not installed; skipping"

audit:
	@command -v cargo-audit >/dev/null 2>&1 && cargo audit || echo "cargo-audit not installed; skipping"

deny:
	@command -v cargo-deny >/dev/null 2>&1 && cargo deny check advisories bans licenses sources || echo "cargo-deny not installed; skipping"

outdated:
	@command -v cargo-outdated >/dev/null 2>&1 && cargo outdated --workspace --exit-code 1 || echo "cargo-outdated not installed; skipping"

# README sync: regenerate each crate README from README.tpl + source `//!`
# doc comments, lint, and diff against the checked-in README.md.
readme:
	@command -v cargo-readme >/dev/null 2>&1 || { echo "cargo-readme not installed; skipping"; exit 0; }
	@command -v panache >/dev/null 2>&1 || { echo "panache not installed; skipping"; exit 0; }
	@for d in crates/lotus/; do \
	(cd $$d && cargo readme -t README.tpl -o /tmp/readme_panache.md 2>/dev/null && panache lint /tmp/readme_panache.md && diff -q /tmp/readme_panache.md README.md > /dev/null 2>&1 || { echo "README.md out of date for $$d — run: (cd $$d && cargo readme -t README.tpl -o README.md)"; exit 1; }) || exit 1; \
	done
