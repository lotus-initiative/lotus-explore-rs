# shellcheck shell=bash
# Layer 3: Nightly sweeps, release dry-run, template selftest, and fallback dispatch.

run_llvm_cov() {
    need cargo-llvm-cov
    if command -v cargo-nextest >/dev/null 2>&1; then
        cargo llvm-cov nextest --workspace --all-features --locked --no-tests=pass "$@"
    else
        cargo llvm-cov --workspace --all-features --locked "$@"
    fi
}

# shellcheck disable=SC2154  # COVERAGE_FAIL_UNDER_LINES from mk/mk.local
cmd_coverage() {
    run_llvm_cov --lcov --output-path lcov.info --fail-under-lines "$COVERAGE_FAIL_UNDER_LINES"
}

# shellcheck disable=SC2154  # COVERAGE_FAIL_UNDER_LINES from mk/mk.local
cmd_coverage_baseline() {
    local pct
    pct="$(run_llvm_cov --summary-only | awk '/^TOTAL/{sub(/%/, "", $10); print int($10)}')"
    [ -n "$pct" ] || die "could not read line coverage"
    upsert_kv_line mk.local COVERAGE_FAIL_UNDER_LINES "\"$pct\"" \
        "# Ratchet: may only go up (./mk ratchet). Set with ./mk coverage-baseline."
    echo "COVERAGE_FAIL_UNDER_LINES: $COVERAGE_FAIL_UNDER_LINES -> $pct (the ratchet gate rejects a decrease against the base branch)"
}

cmd_powerset() { need cargo-hack; cargo hack --workspace --feature-powerset --no-dev-deps check; }

# shellcheck disable=SC2154  # RUST_NIGHTLY and MIRI_PACKAGES from mk.versions/mk.local
cmd_miri() {
    cargo "+$RUST_NIGHTLY" miri --version >/dev/null 2>&1 || die "miri on $RUST_NIGHTLY is not installed; run ./mk setup nightly"
    export MIRIFLAGS="${MIRIFLAGS:--Zmiri-disable-isolation -Zmiri-tree-borrows}"
    if [ "${#MIRI_PACKAGES[@]}" -gt 0 ]; then
        cargo "+$RUST_NIGHTLY" miri test --all-features --locked "${MIRI_PACKAGES[@]/#/--package=}"
    else
        cargo "+$RUST_NIGHTLY" miri test --workspace --all-features --locked
    fi
}

cmd_outdated() { need cargo-outdated; cargo outdated --workspace --root-deps-only --exit-code 1; }

cmd_ci_release() {
    "$0" ci
    if meta_py has-publishable --locked; then
        cargo publish --workspace --dry-run --locked
    else
        echo "[skip] publish: every crate sets publish = false"
    fi
}

cmd_selftest() {
    [ -x tests/gate_selftest.sh ] || skip "the gate selftest lives in the rusti-graben template only"
    tests/gate_selftest.sh
}

cmd_fallback_task() {
    local target_cmd="$1"
    shift
    if declare -F "task_$target_cmd" >/dev/null || [ -f Makefile.toml ]; then
        cargo_make "$target_cmd" "$@"
        return 0
    fi
    echo "Usage: ./mk {$(./mk --list | grep -v '^(' | paste -sd'|' -)}"
    return 1
}
