#!/usr/bin/env bash
# rusti-graben standard runner. Identical in every repository: scripts/adopt.sh
# keeps it in sync, so edit it in the template, never here. Repository-specific
# gates, overrides and tools go in mk.local.
#
# Single entrypoint for every gate. CI calls exactly these tasks.
# A gate either runs and can fail, or is skipped because it cannot apply
# (printed with a reason). A missing tool, or one at the wrong version, is a failure.
set -euo pipefail

if [ -n "${NO_COLOR:-}" ] || [ "${CI:-}" = 1 ]; then
    export CARGO_TERM_COLOR=never
    export NO_COLOR=1
fi

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT_DIR"
export _MK_BASE_PATH="${_MK_BASE_PATH:-$PATH}"
# shellcheck source=mk.versions
. ./mk.versions

# Repository-specific settings (see the template's mk.local for every key).
LOCAL_CI_GATES=()       # cargo-make tasks added to ./mk ci
LOCAL_NIGHTLY_GATES=()  # cargo-make tasks added to ./mk ci-nightly
LOCAL_OVERRIDES=()      # standard gates this repository runs as its own cargo-make task
LOCAL_SETUP=()          # cargo-make tasks ./mk setup runs in every tier (installs mk cannot pin)
LOCAL_CARGO_TOOLS=()    # extra crate@version tools, installed and version-checked
LOCAL_SYSTEM_PACKAGES=() # apt packages ./mk setup installs on CI (Linux runners)
MIRI_PACKAGES=()        # crates miri tests (default: the whole workspace)
MUTANTS_PACKAGES=()     # crates cargo-mutants mutates (default: the whole workspace)
COVERAGE_FAIL_UNDER_LINES=100
ECHOES_MAX_APPROXIMATE=0 # ratcheted approximate duplicate blocks (only down)
ECHOES_MAX_REUSE=0       # ratcheted dependency-reuse opportunities (only down)
MK_TOOLS_DIR="${MK_TOOLS_DIR:-${CARGO_HOME:-$HOME/.cargo}/mk-tools}"
# shellcheck source=mk.local
if [ -f mk.local ]; then . ./mk.local; fi
# shellcheck source=.mk/catalog.sh
. "$ROOT_DIR/.mk/catalog.sh"
# shellcheck source=.mk/core.sh
. "$ROOT_DIR/.mk/core.sh"
# shellcheck source=.mk/tools.sh
. "$ROOT_DIR/.mk/tools.sh"
# shellcheck source=.mk/mutants.sh
. "$ROOT_DIR/.mk/mutants.sh"
# shellcheck source=.mk/mutants_plan.sh
. "$ROOT_DIR/.mk/mutants_plan.sh"
# shellcheck source=.mk/gates.sh
. "$ROOT_DIR/.mk/gates.sh"
# shellcheck source=.mk/nightly.sh
. "$ROOT_DIR/.mk/nightly.sh"
# shellcheck source=.mk/tier.sh
. "$ROOT_DIR/.mk/tier.sh"

refresh_tool_path

cmd="$(normalize_cmd_alias "${1:-ci}")"
shift || true

if dispatch_local_override "$cmd" "$@"; then exit 0; fi

case "$cmd" in
    -h | --help | help) print_help ;;
    --list | --list-all-steps)
        print_task_list ${LOCAL_CI_GATES[@]+"${LOCAL_CI_GATES[@]}"} ${LOCAL_NIGHTLY_GATES[@]+"${LOCAL_NIGHTLY_GATES[@]}"}
        ;;
    check) cmd_check "$@" ;;
    check-changed) cmd_check_changed "$@" ;;
    fix) cmd_fix ;;
    test-fast) cmd_test_fast "$@" ;;
    test-changed) cmd_test_changed "$@" ;;
    watch) cmd_watch "$@" ;;
    bench) cmd_bench "$@" ;;
    udeps) cmd_udeps ;;
    setup) cmd_setup "$@" ;;
    tools) cmd_tools ;;
    actions) cmd_actions ;;
    shell) cmd_shell ;;
    ratchet) cmd_ratchet ;;
    commits) cmd_commits "$@" ;;
    fmt) cmd_fmt ;;
    tombi) cmd_tombi ;;
    typos) cmd_typos ;;
    lint) cmd_lint "$@" ;;
    lints-inherit) cmd_lints_inherit ;;
    test) cmd_test "$@" ;;
    doc) cmd_doc ;;
    deny) cmd_deny ;;
    audit) cmd_audit ;;
    machete) cmd_machete ;;
    msrv) cmd_msrv ;;
    echoes) cmd_echoes "$@" ;;
    echoes-baseline) cmd_echoes_baseline ;;
    refquiem) cmd_refquiem "$@" ;;
    semver) cmd_semver ;;
    wasm) cmd_wasm ;;
    mutants) run_mutants "$@" ;;
    mutants-diff) cmd_mutants_diff "$@" ;;
    mutants-affected) cmd_mutants_affected "$@" ;;
    mutants-plan) cmd_mutants_plan "$@" ;;
    coverage) cmd_coverage ;;
    coverage-baseline) cmd_coverage_baseline ;;
    powerset) cmd_powerset ;;
    miri) cmd_miri ;;
    outdated) cmd_outdated ;;
    ci | ci-core | ci-fast) run_standard_tier "$cmd" ${LOCAL_CI_GATES[@]+"${LOCAL_CI_GATES[@]}"} ;;
    ci-nightly-checks | ci-nightly) run_nightly_tier "$cmd" ${LOCAL_NIGHTLY_GATES[@]+"${LOCAL_NIGHTLY_GATES[@]}"} ;;
    local:*) cargo_make "${cmd#local:}" "$@" ;;
    ci-release) cmd_ci_release ;;
    selftest) cmd_selftest ;;
    *) cmd_fallback_task "$cmd" "$@" ;;
esac
