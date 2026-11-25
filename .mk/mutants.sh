# shellcheck shell=bash
# Layer 2: Bounded mutation runner (resource probes, flag/config parsing,
# CPU budgeting, atomic lockdir, and process-group watchdog).

MUTANTS_MIN_FREE_MEM_PCT="${MUTANTS_MIN_FREE_MEM_PCT:-15}"
MUTANTS_MIN_FREE_DISK_GB="${MUTANTS_MIN_FREE_DISK_GB:-10}"
_MK_MUTANTS_PID=""
_MK_MUTANTS_LOCK=""
_MK_MUTANTS_ABORTED=0
_MK_M_IN_PLACE=0
_MK_M_HAS_IN_PLACE_ARG=0
_MK_M_HAS_GITIGNORE=0
_MK_M_HAS_JOBSERVER_CAP=0
_MK_M_HAS_CUSTOM_CONFIG=0
_MK_M_CUSTOM_CONFIG=""
_MK_M_JOBS=1
_MK_M_THREADS=1

cleanup_mutants_trap() {
    _MK_MUTANTS_ABORTED=1
    if [ -n "$_MK_MUTANTS_PID" ]; then kill -INT -- "-$_MK_MUTANTS_PID" 2>/dev/null || true; fi
}

cores() { getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2; }

free_mem_pct() {
    if [ -r /proc/meminfo ]; then
        awk '/^MemTotal:/ { t = $2 } /^MemAvailable:/ { a = $2 } END { if (t) print int(a * 100 / t) }' /proc/meminfo
    elif [ "$(sysctl -n kern.memorystatus_vm_pressure_level 2>/dev/null)" = 4 ]; then
        echo 0
    else
        sysctl -n kern.memorystatus_level 2>/dev/null || true
    fi
}

free_disk_gb() { df -Pk "$1" 2>/dev/null | awk 'NR == 2 { print int($4 / 1048576) }'; }

mutants_starved() {
    local mem disk dir
    mem="$(free_mem_pct)"
    if [ -n "$mem" ] && [ "$mem" -lt "$MUTANTS_MIN_FREE_MEM_PCT" ]; then
        echo "available memory ${mem}% < ${MUTANTS_MIN_FREE_MEM_PCT}% (MUTANTS_MIN_FREE_MEM_PCT)"
        return
    fi
    for dir in "${ROOT_DIR:-.}" "${TMPDIR:-/tmp}"; do
        disk="$(free_disk_gb "$dir")"
        if [ -n "$disk" ] && [ "$disk" -lt "$MUTANTS_MIN_FREE_DISK_GB" ]; then
            echo "free disk ${disk} GB on $dir < ${MUTANTS_MIN_FREE_DISK_GB} GB (MUTANTS_MIN_FREE_DISK_GB)"
            return
        fi
    done
}

mutants_config_has_gitignore() {
    local has_custom="$1" custom="$2"
    if [ "$has_custom" = 0 ] && [ -f .cargo/mutants.toml ] &&
        grep -Eq '^[[:space:]]*gitignore[[:space:]]*=' .cargo/mutants.toml; then
        return 0
    fi
    [ -n "$custom" ] && [ -f "$custom" ] && grep -Eq '^[[:space:]]*gitignore[[:space:]]*=' "$custom"
}

mutants_parse_flags() {
    local arg expect_config=0
    _MK_M_IN_PLACE=0; _MK_M_HAS_IN_PLACE_ARG=0; _MK_M_HAS_GITIGNORE=0
    _MK_M_HAS_JOBSERVER_CAP=0; _MK_M_HAS_CUSTOM_CONFIG=0; _MK_M_CUSTOM_CONFIG=""
    if [ "${MUTANTS_IN_PLACE:-0}" = 1 ]; then _MK_M_IN_PLACE=1; fi
    for arg in "$@"; do
        if [ "$expect_config" = 1 ]; then _MK_M_CUSTOM_CONFIG="$arg"; expect_config=0; continue; fi
        case "$arg" in
            --in-place) _MK_M_IN_PLACE=1; _MK_M_HAS_IN_PLACE_ARG=1 ;;
            --gitignore | --gitignore=*) _MK_M_HAS_GITIGNORE=1 ;;
            --jobserver-tasks | --jobserver-tasks=* | --jobserver=false) _MK_M_HAS_JOBSERVER_CAP=1 ;;
            --no-config) _MK_M_HAS_CUSTOM_CONFIG=1 ;;
            --config) _MK_M_HAS_CUSTOM_CONFIG=1; expect_config=1 ;;
            --config=*) _MK_M_HAS_CUSTOM_CONFIG=1; _MK_M_CUSTOM_CONFIG="${arg#--config=}" ;;
        esac
    done
    if [ "$_MK_M_HAS_GITIGNORE" = 0 ] &&
        mutants_config_has_gitignore "$_MK_M_HAS_CUSTOM_CONFIG" "$_MK_M_CUSTOM_CONFIG"; then
        _MK_M_HAS_GITIGNORE=1
    fi
}

mutants_compute_budget() {
    local in_place="$1" n="$2"
    if [ "$in_place" = 1 ]; then
        _MK_M_JOBS=1
        if [ -n "${CI:-}" ]; then _MK_M_THREADS="$n"; else _MK_M_THREADS=$((n / 2)); fi
    elif [ -n "${CI:-}" ]; then
        _MK_M_JOBS="${MUTANTS_JOBS:-$((n > 3 ? n / 2 : 1))}"
        _MK_M_THREADS=$((n / _MK_M_JOBS))
    else
        _MK_M_JOBS="${MUTANTS_JOBS:-1}"
        _MK_M_THREADS=$((n / _MK_M_JOBS / 2))
    fi
    [ "$_MK_M_THREADS" -ge 1 ] || _MK_M_THREADS=1
}

mutants_ensure_tmpdir() {
    [ -z "${TMPDIR:-}" ] || return 0
    local tmp_disk root="${ROOT_DIR:-.}"
    if [ -n "${RUNNER_TEMP:-}" ] && [ -d "$RUNNER_TEMP" ]; then
        export TMPDIR="$RUNNER_TEMP"
    elif tmp_disk="$(free_disk_gb /tmp)" && [ "${tmp_disk:-0}" -lt "$MUTANTS_MIN_FREE_DISK_GB" ]; then
        mkdir -p "$root/target/tmp"
        export TMPDIR="$root/target/tmp"
    fi
}

mutants_target_dir() {
    local root="${ROOT_DIR:-.}" base_target="${CARGO_TARGET_DIR:-${ROOT_DIR:-.}/target}"
    base_target="${base_target%/}"
    case "$base_target" in /*) ;; *) base_target="$root/$base_target" ;; esac
    case "$base_target" in */mutants) printf '%s\n' "$base_target" ;; *) printf '%s\n' "$base_target/mutants" ;; esac
}

mutants_acquire_lock() {
    local lock_dir="$1" waited=0 owner_pid
    mkdir -p "$(dirname "$lock_dir")"
    while ! mkdir "$lock_dir" 2>/dev/null; do
        owner_pid="$(cat "$lock_dir/pid" 2>/dev/null || true)"
        if [ -n "$owner_pid" ] && ! kill -0 "$owner_pid" 2>/dev/null; then rm -rf "$lock_dir"; continue; fi
        if [ "$waited" = 0 ]; then echo "mutants: waiting for lock $lock_dir..."; waited=1; fi
        sleep 1 || true
    done
    printf '%s\n' "$$" >"$lock_dir/pid"
    _MK_MUTANTS_LOCK="$lock_dir"
}

mutants_release_lock() {
    [ -z "$_MK_MUTANTS_LOCK" ] || rm -rf "$_MK_MUTANTS_LOCK"
    _MK_MUTANTS_PID=""
    _MK_MUTANTS_LOCK=""
}

mutants_exec_watchdog() {
    local pid="$1" reason rc=0
    _MK_MUTANTS_PID="$pid"
    _MK_MUTANTS_ABORTED=0
    trap 'cleanup_mutants_trap' INT TERM
    while kill -0 "$pid" 2>/dev/null; do
        if [ "$_MK_MUTANTS_ABORTED" = 1 ]; then break; fi
        reason="$(mutants_starved)"
        if [ -n "$reason" ]; then
            echo "ERROR: stopping cargo-mutants: $reason" >&2
            kill -INT -- "-$pid" 2>/dev/null || true
            for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
                kill -0 "$pid" 2>/dev/null || break; sleep 2 || true
            done
            kill -KILL -- "-$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
            mutants_release_lock
            exit 1
        fi
        sleep 2 || true
    done
    wait "$pid" || rc=$?
    trap - INT TERM
    mutants_release_lock
    return "$rc"
}

mutants_print_banner() {
    local n="$1"
    if [ "$_MK_M_IN_PLACE" = 1 ]; then
        echo "mutants: in-place, $_MK_M_THREADS build/test threads, $n cores;" \
            "stops below ${MUTANTS_MIN_FREE_MEM_PCT}% memory or ${MUTANTS_MIN_FREE_DISK_GB} GB disk"
    else
        echo "mutants: $_MK_M_JOBS at a time, $_MK_M_THREADS build/test threads each, $n cores;" \
            "stops below ${MUTANTS_MIN_FREE_MEM_PCT}% memory or ${MUTANTS_MIN_FREE_DISK_GB} GB disk"
    fi
}

# run_mutants <cargo-mutants args...>: the only way mk starts cargo-mutants.
# shellcheck disable=SC2154  # MUTANTS_PACKAGES from mk/mk.local
run_mutants() {
    need cargo-mutants
    local n reason inc="${MUTANTS_INCREMENTAL:-1}"
    n="$(cores)"
    mutants_parse_flags "$@"
    mutants_compute_budget "$_MK_M_IN_PLACE" "$n"
    mutants_ensure_tmpdir
    reason="$(mutants_starved)"
    [ -z "$reason" ] || die "not starting cargo-mutants: $reason"

    local -a scope=(--workspace) extra_args=() env_args=()
    if [ "${#MUTANTS_PACKAGES[@]}" -gt 0 ]; then scope=("${MUTANTS_PACKAGES[@]/#/--package=}"); fi
    if has_pkg_flag "$@"; then scope=(); fi
    if [ "$_MK_M_HAS_GITIGNORE" = 0 ]; then extra_args+=(--gitignore=true); fi
    if [ "$_MK_M_IN_PLACE" = 1 ]; then
        [ "$_MK_M_HAS_IN_PLACE_ARG" = 1 ] || extra_args+=(--in-place)
        [ "$_MK_M_HAS_JOBSERVER_CAP" = 1 ] || extra_args+=(--jobserver-tasks "$_MK_M_THREADS")
    else
        extra_args+=(--jobs "$_MK_M_JOBS")
        [ "$_MK_M_HAS_JOBSERVER_CAP" = 1 ] || extra_args+=(--jobserver-tasks "$((_MK_M_JOBS * _MK_M_THREADS))")
    fi
    case "${RUSTC_WRAPPER:-}${RUSTC_WORKSPACE_WRAPPER:-}" in *sccache*) inc=0 ;; esac
    env_args=(CARGO_BUILD_JOBS="$_MK_M_THREADS" NEXTEST_TEST_THREADS="$_MK_M_THREADS"
        CARGO_INCREMENTAL="$inc" CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0)
    if [ "$_MK_M_IN_PLACE" = 0 ] && [ "$_MK_M_JOBS" -eq 1 ]; then
        local mutants_target; mutants_target="$(mutants_target_dir)"
        mutants_acquire_lock "${mutants_target}.lock"; env_args+=(CARGO_TARGET_DIR="$mutants_target")
    elif [ "$_MK_M_IN_PLACE" = 0 ]; then
        env_args=(-u CARGO_TARGET_DIR "${env_args[@]}")
    fi
    mutants_print_banner "$n"
    set -m
    env "${env_args[@]}" nice -n 19 cargo mutants ${extra_args[@]+"${extra_args[@]}"} ${scope[@]+"${scope[@]}"} --all-features "$@" &
    local pid=$!
    set +m
    mutants_exec_watchdog "$pid"
}
