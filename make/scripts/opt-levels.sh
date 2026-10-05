#!/usr/bin/env bash
# The optimisation level is declared in three places, and `dx` passes
# `--rustc-args=-Copt-level=` last, so the task runner silently wins over the
# `[profile.release]` it is supposed to match. That drift shipped once: the one
# build said `s` while the profile said `z`, and every release bundle was 185300
# raw / 38433 brotli bytes larger than intended, with nothing in CI noticing.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

profile=$(sed -n 's/^opt-level = "\(.\)"/\1/p' Cargo.toml | head -1)
wasmopt=$(sed -n '/^\[web.wasm_opt\]/,/^\[/ s/^level = "\(.\)"/\1/p' apps/lotus-explore-rs/Dioxus.toml | head -1)
# Only the values actually passed to a `dx` invocation, so this script cannot
# match its own source and the prose in `make/web.toml` cannot either. The anchor
# is a non-comment line; the doc comment above the release build names
# `-Copt-level=` too, and reading that as a second declaration is how a two-value
# check would come to report a mismatch that is only a sentence.
levels=$(grep -hv '^\s*#' make/*.toml | grep -ho -- '-Copt-level=.' | sed 's/.*=//' | sort -u | tr '\n' ' ')

printf 'Cargo.toml [profile.release]   opt-level = %s\n' "$profile"
printf 'Dioxus.toml [web.wasm_opt]     level     = %s\n' "$wasmopt"
printf 'make/*.toml --rustc-args                 = %s\n' "${levels% }"
fail=0
[ -n "$profile" ] || { echo "MISMATCH: no opt-level found in Cargo.toml" >&2; fail=1; }
[ -n "$wasmopt" ] || { echo "MISMATCH: no level found in Dioxus.toml" >&2; fail=1; }
[ "$profile" = "$wasmopt" ] || { echo "MISMATCH: wasm_opt level '$wasmopt' != profile opt-level '$profile'" >&2; fail=1; }
[ "$(printf '%s' "$levels" | wc -w | tr -d ' ')" -eq 1 ] || { echo "MISMATCH: more than one -Copt-level is passed: $levels" >&2; fail=1; }
[ "${levels% }" = "$profile" ] || { echo "MISMATCH: -Copt-level='${levels% }' != profile '$profile'" >&2; fail=1; }
exit $fail
