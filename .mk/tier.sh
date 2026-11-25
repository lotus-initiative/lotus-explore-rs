# shellcheck shell=bash
# Layer 3: Tier orchestrator, local cargo-make bridge, and LOCAL_OVERRIDES guard/dispatch.

cargo_make() {
    if declare -F "task_$1" >/dev/null; then
        local task="task_$1"
        shift
        "$task" "$@"
        return
    fi
    [ -f Makefile.toml ] || die "no Makefile.toml or task_$1 function for local task '$1'"
    need cargo-make
    cargo make --no-workspace "$@"
}

# A standard gate this repository implements itself: `gate` or `gate=task`.
# shellcheck disable=SC2154  # LOCAL_OVERRIDES from mk/mk.local
dispatch_local_override() {
    local target_cmd="$1" override
    shift
    for override in ${LOCAL_OVERRIDES[@]+"${LOCAL_OVERRIDES[@]}"}; do
        case "${override%%=*}" in
            # Overriding them would bypass run_mutants' limits and watchdog.
            mutants | mutants-diff | mutants-affected)
                die "LOCAL_OVERRIDES cannot replace ${override%%=*}: scope it with MUTANTS_PACKAGES, configure it in .cargo/mutants.toml"
                ;;
        esac
        if [ "${override%%=*}" = "$target_cmd" ]; then
            echo "($target_cmd: this repository's own task '${override#*=}', see LOCAL_OVERRIDES in mk.local)"
            cargo_make "${override#*=}" "$@"
            return 0
        fi
    done
    return 1
}

# Runs every gate as its own process (so `set -e` holds inside each), keeps
# going after a failure, then prints a ran/skipped/failed table.
# shellcheck disable=SC2154  # SKIP_RC from .mk/catalog.sh
run_tier() {
    local tier="$1" gate rc start desc hint skip_rust=0 lint_passed=0
    shift
    if [ "$tier" = ci ] || [ "$tier" = ci-fast ] || [ "$tier" = ci-core ]; then
        if docs_or_meta_only_diff; then skip_rust=1; fi
    fi
    local -a report=()
    local failed=0
    for gate in "$@"; do
        desc="$(gate_desc "$gate")"
        if [ -n "$desc" ]; then echo "--- $gate ($desc) ---"; else echo "--- $gate ---"; fi
        if [ "$skip_rust" = 1 ] && is_rust_compile_gate "$gate"; then
            echo "[skip] $gate: docs/config-only diff against $(base_ref)"
            report+=("SKIP  $gate")
            continue
        fi
        start=$SECONDS
        rc=0
        MK_NESTED=1 MK_LINT_PASSED="$lint_passed" "$0" "$gate" || rc=$?
        case "$rc" in
            0)
                if [ "$gate" = lint ]; then lint_passed=1; fi
                report+=("PASS  $gate ($((SECONDS - start))s)")
                ;;
            "$SKIP_RC") report+=("SKIP  $gate") ;;
            *)
                hint="$(gate_hint "$gate")"
                if [ -n "$hint" ]; then echo "hint ($gate): $hint" >&2; fi
                report+=("FAIL  $gate (exit $rc)")
                failed=1
                ;;
        esac
    done
    echo "=== $tier summary ==="
    printf '%s\n' "${report[@]}"
    [ "$failed" = 0 ] || die "$tier failed"
    echo "==> $tier passed"
}

run_standard_tier() {
    local tier="$1" g
    shift
    local -a gates=()
    while IFS= read -r g; do [ -n "$g" ] && gates+=("$g"); done < <(tier_gates "$tier")
    run_tier "$tier" "${gates[@]}" ${@+"${@/#/local:}"}
}

run_nightly_tier() {
    local tier="$1"
    shift
    if idle_scheduled_nightly; then
        echo "note: no commits in the last 24h on scheduled run; running only advisory/freshness gates (audit, outdated)"
        run_tier "$tier" audit outdated
    else
        run_standard_tier "$tier" "$@"
    fi
}
