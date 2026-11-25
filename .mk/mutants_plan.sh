# shellcheck shell=bash
# Layer 2: Mutation scope filtering, diff/affected commands, and shard planner.

_MK_DIFF_TMP=""
_MK_PLAN_MODE="diff"
_MK_PLAN_BASE=""
_MK_PLAN_HAS_BASE=0

cleanup_diff_tmp() {
    if [ -n "$_MK_DIFF_TMP" ]; then rm -f "$_MK_DIFF_TMP"; fi
}

# shellcheck disable=SC2154  # MUTANTS_PACKAGES from mk/mk.local
intersect_mutants_scope() {
    local pkg m keep
    while IFS= read -r pkg; do
        [ -n "$pkg" ] || continue
        if [ "${#MUTANTS_PACKAGES[@]}" -eq 0 ]; then printf '%s\n' "$pkg"; continue; fi
        keep=0
        for m in "${MUTANTS_PACKAGES[@]}"; do
            if [ "$pkg" = "$m" ]; then keep=1; break; fi
        done
        [ "$keep" = 0 ] || printf '%s\n' "$pkg"
    done
}

scoped_packages_for_mutants() {
    local mode="$1" base="${2:-}"
    case "$mode" in
        changed) changed_rust_packages "$base" | intersect_mutants_scope ;;
        affected) affected_packages "$base" | intersect_mutants_scope ;;
        full) meta_py packages | intersect_mutants_scope ;;
        *) die "unknown scope mode '$mode'" ;;
    esac
}

cmd_mutants_diff() {
    need cargo-mutants
    local base p
    local -a pkg_args=() scoped=()
    base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
    if ! has_pkg_flag "$@"; then
        while IFS= read -r p; do [ -n "$p" ] && scoped+=("$p"); done < <(scoped_packages_for_mutants changed "$base")
        [ "${#scoped[@]}" -gt 0 ] || skip "no Rust changes in mutation scope against $base"
        pkg_args=("${scoped[@]/#/--package=}")
    fi
    _MK_DIFF_TMP="$(mktemp)"
    trap 'cleanup_diff_tmp' EXIT
    write_rust_diff "$base" "$_MK_DIFF_TMP"
    [ -s "$_MK_DIFF_TMP" ] || skip "no Rust changes against $base"
    run_mutants ${pkg_args[@]+"${pkg_args[@]}"} --in-diff "$_MK_DIFF_TMP" "$@"
}

cmd_mutants_affected() {
    need cargo-mutants
    local base p
    local -a pkg_args=() scoped=()
    if [ $# -gt 0 ] && [[ "$1" != -* ]]; then
        base="$1"; shift
        git rev-parse -q --verify "$base^{commit}" >/dev/null || die "unknown base ref '$base'"
    else
        base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
    fi
    if ! has_pkg_flag "$@"; then
        while IFS= read -r p; do [ -n "$p" ] && scoped+=("$p"); done < <(scoped_packages_for_mutants affected "$base")
        [ "${#scoped[@]}" -gt 0 ] || skip "no affected packages in mutation scope against $base"
        pkg_args=("${scoped[@]/#/--package=}")
    fi
    run_mutants ${pkg_args[@]+"${pkg_args[@]}"} "$@"
}

mutants_plan_parse_args() {
    _MK_PLAN_MODE="diff"; _MK_PLAN_BASE=""; _MK_PLAN_HAS_BASE=0
    while [ $# -gt 0 ]; do
        case "$1" in
            --mode) [ $# -ge 2 ] || die "--mode requires a value"; _MK_PLAN_MODE="$2"; shift 2 ;;
            --mode=*) _MK_PLAN_MODE="${1#--mode=}"; shift ;;
            --base) [ $# -ge 2 ] || die "--base requires a value"; _MK_PLAN_BASE="$2"; _MK_PLAN_HAS_BASE=1; shift 2 ;;
            --base=*) _MK_PLAN_BASE="${1#--base=}"; _MK_PLAN_HAS_BASE=1; shift ;;
            *) die "unknown mutants-plan argument '$1'" ;;
        esac
    done
    case "$_MK_PLAN_MODE" in
        diff | affected | full) ;;
        *) die "invalid --mode '$_MK_PLAN_MODE' (expected diff, affected, or full)" ;;
    esac
}

mutants_plan_emit_nonzero() {
    local mode="$1" mutants="$2"
    shift 2
    local target max_shards shards
    if [ "$mode" = diff ]; then
        target="${MUTANTS_TARGET_PER_SHARD:-150}"; max_shards="${MUTANTS_MAX_SHARDS:-4}"
    else
        target="${MUTANTS_TARGET_PER_SHARD:-250}"; max_shards="${MUTANTS_MAX_SHARDS:-8}"
    fi
    [[ "$target" =~ ^[1-9][0-9]*$ ]] || die "MUTANTS_TARGET_PER_SHARD must be a positive integer, got '$target'"
    [[ "$max_shards" =~ ^[1-9][0-9]*$ ]] || die "MUTANTS_MAX_SHARDS must be a positive integer, got '$max_shards'"
    shards="$(meta_py shards "$mutants" "$target" "$max_shards")"
    meta_py plan-json "$mode" "$mutants" "$shards" "$@"
}

cmd_mutants_plan() {
    need cargo-mutants; mutants_plan_parse_args "$@"
    local mode="$_MK_PLAN_MODE" base="$_MK_PLAN_BASE" scope_mode="$_MK_PLAN_MODE" p
    if [ "$mode" = diff ] || [ "$mode" = affected ]; then
        if [ "$_MK_PLAN_HAS_BASE" = 1 ]; then
            git rev-parse -q --verify "$base^{commit}" >/dev/null || die "unknown base ref '$base'"
        elif ! base="$(base_ref)"; then meta_py plan-json "$mode" 0 0; return 0; fi
    fi
    if [ "$mode" = diff ]; then scope_mode="changed"; fi
    local -a scoped=()
    while IFS= read -r p; do [ -n "$p" ] && scoped+=("$p"); done < <(scoped_packages_for_mutants "$scope_mode" "$base")
    if [ "${#scoped[@]}" -eq 0 ]; then meta_py plan-json "$mode" 0 0; return 0; fi
    local -a list_cmd=(cargo mutants --list "${scoped[@]/#/--package=}" --all-features)
    if [ "$mode" = diff ]; then
        _MK_DIFF_TMP="$(mktemp)"; trap 'cleanup_diff_tmp' EXIT
        write_rust_diff "$base" "$_MK_DIFF_TMP"
        if [ ! -s "$_MK_DIFF_TMP" ]; then meta_py plan-json "$mode" 0 0; return 0; fi
        list_cmd+=(--in-diff "$_MK_DIFF_TMP")
    fi
    local mutants
    mutants="$(awk 'NF{c++} END{print c+0}' <<<"$("${list_cmd[@]}")")"
    mutants_plan_emit_nonzero "$mode" "$mutants" "${scoped[@]}"
}
