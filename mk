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
# shellcheck source=mk.versions
. ./mk.versions

# Repository-specific settings (see the template's mk.local for every key).
LOCAL_CI_GATES=()       # cargo-make tasks added to ./mk ci
LOCAL_NIGHTLY_GATES=()  # cargo-make tasks added to ./mk ci-nightly
LOCAL_OVERRIDES=()      # standard gates this repository runs as its own cargo-make task
LOCAL_SETUP=()          # cargo-make tasks ./mk setup runs in every tier (installs mk cannot pin)
LOCAL_CARGO_TOOLS=()    # extra crate@version tools, pinned like mk.versions
LOCAL_SYSTEM_PACKAGES=() # apt packages ./mk setup installs on CI (Linux runners)
MIRI_PACKAGES=()        # crates miri tests (default: the whole workspace)
MUTANTS_PACKAGES=()     # crates cargo-mutants mutates (default: the whole workspace)
COVERAGE_FAIL_UNDER_LINES=100
ECHOES_MAX_APPROXIMATE=0 # legacy approximate duplicate blocks (ratchet: only down)
ECHOES_MAX_REUSE=0       # legacy dependency-reuse opportunities (ratchet: only down)
# shellcheck source=mk.local
if [ -f mk.local ]; then . ./mk.local; fi

SKIP_RC=78
CI_GATES=(tools actions shell commits ratchet fmt tombi typos lint lints-inherit test doc deny audit machete msrv echoes refquiem semver wasm mutants-diff)
CI_FAST_GATES=(tools actions shell commits ratchet fmt tombi typos lint lints-inherit test-fast doc deny machete msrv echoes refquiem semver wasm)
NIGHTLY_GATES=(mutants coverage powerset miri outdated)

cmd="${1:-ci}"
shift || true
case "$cmd" in
    toml) cmd=tombi ;;
    unused-deps) cmd=machete ;;
    duplication | dup) cmd=echoes ;;
    stale-refs) cmd=refquiem ;;
    feature-powerset) cmd=powerset ;;
esac

die() { echo "ERROR: $*" >&2; exit 1; }

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


# Not applicable here: exit 0 when run directly, SKIP_RC when run by a tier.
skip() {
    echo "[skip] $cmd: $*"
    if [ "${MK_NESTED:-0}" = 1 ]; then exit "$SKIP_RC"; fi
    exit 0
}

has_lib() { cargo metadata --no-deps --format-version 1 --locked | grep -q '"kind":\["lib"\]'; }
# Declared outside comments (the template's toolchain file mentions it in one).
has_wasm() { grep -Eq '^[^#]*wasm32' rust-toolchain.toml Cargo.toml 2>/dev/null; }
msrv() { sed -n 's/^rust-version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -1; }

# Branch this work merges into: MK_BASE, the PR base on CI, origin/HEAD, origin/main, origin/master.
base_ref() {
    local ref
    for ref in "${MK_BASE:-}" "${GITHUB_BASE_REF:+origin/$GITHUB_BASE_REF}" \
        "$(git symbolic-ref -q --short refs/remotes/origin/HEAD 2>/dev/null || true)" origin/main origin/master; do
        if [ -n "$ref" ] && git rev-parse -q --verify "$ref^{commit}" >/dev/null; then
            echo "$ref"
            return
        fi
    done
    return 1
}

CONVENTIONAL='^(build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)(\([a-z0-9._/-]+\))?!?: [^ ]'

# --- pinned tools -----------------------------------------------------------

# A pinned tool is `crate@version`, or `crate@version:binary` when the binary has
# another name (`dioxus-cli@0.7.10:dx`). Tools are named `crate` or `crate:binary`.
spec_crate() { echo "${1%%@*}"; }
spec_ver() { local rest="${1#*@}"; echo "${rest%%:*}"; }
spec_tool() {
    local rest="${1#*@}"
    case "$rest" in *:*) echo "$(spec_crate "$1"):${rest#*:}" ;; *) spec_crate "$1" ;; esac
}

tool_version() {
    case "$1" in
        *:*) "${1#*:}" --version ;;
        typos-cli) typos --version ;;
        zizmor) zizmor --version ;;
        echoes | refquiem) cargo "$1" --version ;;
        cargo-*) cargo "${1#cargo-}" --version ;;
        tombi) tombi --version ;;
        shellcheck) shellcheck --version ;;
        *) "$1" --version ;; # a crate whose binary has its name (mk.local tools)
    esac
}

# Exact pinned version installed? (probe only, never exits)
has_version() {
    local out
    out="$(tool_version "$1" 2>/dev/null)" || return 1
    printf '%s\n' "$out" | grep -Eq "(^|[^0-9.])${2//./\\.}([^0-9.]|$)"
}

require() {
    has_version "$1" "$2" ||
        die "$1 $2 (mk.versions) is required, found '$(tool_version "$1" 2>&1 | head -1)'; run ./mk setup"
}

require_spec() { require "$(spec_tool "$1")" "$(spec_ver "$1")"; }

require_tools() {
    local spec
    for spec in "$@"; do require_spec "$spec"; done
}

spec_of() {
    local spec
    for spec in "${CARGO_TOOLS_CI[@]}" "${CARGO_TOOLS_NIGHTLY[@]}" "${LOCAL_CARGO_TOOLS[@]}"; do
        if [ "$(spec_crate "$spec")" = "$1" ]; then echo "$spec"; return; fi
    done
    die "$1 is not pinned in mk.versions"
}

need() { require_spec "$(spec_of "$1")"; }

install_tombi() {
    if has_version tombi "$TOMBI_VERSION"; then return; fi
    local host target tmp bin_dir
    host="$(rustc -vV | sed -n 's/^host: //p')"
    target="${host/-linux-gnu/-linux-musl}"
    tmp="$(mktemp -d)"
    bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    curl -fsSL "https://github.com/tombi-toml/tombi/releases/download/v${TOMBI_VERSION}/tombi-cli-${TOMBI_VERSION}-${target}.tar.gz" |
        tar xz -C "$tmp"
    mkdir -p "$bin_dir"
    install -m 0755 "$tmp/tombi-cli-${TOMBI_VERSION}-${target}/tombi" "$bin_dir/tombi"
    rm -rf "$tmp"
}

install_shellcheck() {
    if has_version shellcheck "$SHELLCHECK_VERSION"; then return; fi
    local os arch tmp bin_dir
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"
    arch="${arch/arm64/aarch64}"
    tmp="$(mktemp -d)"
    bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
    curl -fsSL "https://github.com/koalaman/shellcheck/releases/download/v${SHELLCHECK_VERSION}/shellcheck-v${SHELLCHECK_VERSION}.${os}.${arch}.tar.gz" |
        tar xz -C "$tmp"
    mkdir -p "$bin_dir"
    install -m 0755 "$tmp/shellcheck-v${SHELLCHECK_VERSION}/shellcheck" "$bin_dir/shellcheck"
    rm -rf "$tmp"
}

install_cargo_tools() {
    command -v cargo-binstall >/dev/null || die "cargo-binstall is required: https://github.com/cargo-bins/cargo-binstall"
    local spec
    for spec in "$@"; do
        if ! has_version "$(spec_tool "$spec")" "$(spec_ver "$spec")"; then
            # Prebuilt when the crate publishes binaries; otherwise build it.
            cargo binstall --no-confirm --locked --force "$(spec_crate "$spec")@$(spec_ver "$spec")" ||
                cargo install --locked --force "$(spec_crate "$spec")" --version "$(spec_ver "$spec")"
        fi
        require_spec "$spec"
    done
}

# --- mutation testing -------------------------------------------------------

# cargo-mutants rebuilds a crate once per mutant, in scratch copies, for hours.
# Unbounded, that wedged developer machines (twice, once at --jobs 2) and cost a
# CI runner its connection to GitHub (lotus-explore-rs, 2026-11-01). So every
# run is bounded by construction, whatever the caller passes:
# - concurrency: MUTANTS_JOBS mutants at once (default 1 locally, half the cores
#   on CI), the cores split between them for cargo and nextest, and locally half
#   of them left to the rest of the machine;
# - size: no debuginfo and no incremental cache, most of a test build's memory,
#   link time and disk, and nothing a mutant needs;
# - priority: nice 19, so the desktop stays responsive;
# - a watchdog: it refuses to start, and stops a running mutation run with
#   SIGINT (so cargo-mutants removes its scratch copies), when available memory
#   or free disk falls below a floor, rather than let the machine swap itself to
#   a halt. Floors: MUTANTS_MIN_FREE_MEM_PCT (default 15) and
#   MUTANTS_MIN_FREE_DISK_GB (default 10), on the repository's and TMPDIR's volumes.
MUTANTS_MIN_FREE_MEM_PCT="${MUTANTS_MIN_FREE_MEM_PCT:-15}"
MUTANTS_MIN_FREE_DISK_GB="${MUTANTS_MIN_FREE_DISK_GB:-10}"

cores() { getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2; }

# Available memory as a whole percentage, or nothing where it cannot be read.
# macOS: the kernel's own availability level, the figure jetsam acts on, read
# as 0 once the kernel reports critical pressure.
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

# Why the machine is below a floor, or nothing when it is not.
mutants_starved() {
    local mem disk dir
    mem="$(free_mem_pct)"
    if [ -n "$mem" ] && [ "$mem" -lt "$MUTANTS_MIN_FREE_MEM_PCT" ]; then
        echo "available memory ${mem}% < ${MUTANTS_MIN_FREE_MEM_PCT}% (MUTANTS_MIN_FREE_MEM_PCT)"
        return
    fi
    for dir in "$ROOT_DIR" "${TMPDIR:-/tmp}"; do
        disk="$(free_disk_gb "$dir")"
        if [ -n "$disk" ] && [ "$disk" -lt "$MUTANTS_MIN_FREE_DISK_GB" ]; then
            echo "free disk ${disk} GB on $dir < ${MUTANTS_MIN_FREE_DISK_GB} GB (MUTANTS_MIN_FREE_DISK_GB)"
            return
        fi
    done
}

# run_mutants <cargo-mutants args...>: the only way mk starts cargo-mutants.
run_mutants() {
    need cargo-mutants
    local n jobs threads reason pid rc=0
    n="$(cores)"
    if [ -n "${CI:-}" ]; then
        jobs="${MUTANTS_JOBS:-$((n > 3 ? n / 2 : 1))}"
        threads=$((n / jobs))
    else
        jobs="${MUTANTS_JOBS:-1}"
        threads=$((n / jobs / 2))
    fi
    [ "$threads" -ge 1 ] || threads=1
    if [ -z "${TMPDIR:-}" ]; then
        if [ -n "${RUNNER_TEMP:-}" ] && [ -d "$RUNNER_TEMP" ]; then
            export TMPDIR="$RUNNER_TEMP"
        elif [ "$(free_disk_gb /tmp)" -lt "$MUTANTS_MIN_FREE_DISK_GB" ]; then
            mkdir -p "$ROOT_DIR/target/tmp"
            export TMPDIR="$ROOT_DIR/target/tmp"
        fi
    fi
    reason="$(mutants_starved)"
    [ -z "$reason" ] || die "not starting cargo-mutants: $reason"
    local -a scope=(--workspace)
    if [ "${#MUTANTS_PACKAGES[@]}" -gt 0 ]; then scope=("${MUTANTS_PACKAGES[@]/#/--package=}"); fi
    # A caller naming its own packages (`./mk mutants -p crate`) replaces the default.
    local arg
    for arg in "$@"; do
        case "$arg" in -p | --package | --package=* | --workspace) scope=() ;; esac
    done
    echo "mutants: $jobs at a time, $threads build/test threads each, $n cores;" \
        "stops below ${MUTANTS_MIN_FREE_MEM_PCT}% memory or ${MUTANTS_MIN_FREE_DISK_GB} GB disk"
    # Its own process group, so the watchdog and Ctrl-C reach every rustc and test.
    set -m
    CARGO_BUILD_JOBS="$threads" NEXTEST_TEST_THREADS="$threads" CARGO_INCREMENTAL=0 \
        CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
        nice -n 19 cargo mutants --jobs "$jobs" ${scope[@]+"${scope[@]}"} --all-features "$@" &
    pid=$!
    set +m
    trap 'kill -INT -- "-$pid" 2>/dev/null || true' INT TERM
    while kill -0 "$pid" 2>/dev/null; do
        reason="$(mutants_starved)"
        if [ -n "$reason" ]; then
            echo "ERROR: stopping cargo-mutants: $reason" >&2
            kill -INT -- "-$pid" 2>/dev/null || true
            for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
                kill -0 "$pid" 2>/dev/null || break
                sleep 2 || true
            done
            kill -KILL -- "-$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
            exit 1
        fi
        sleep 2 || true
    done
    wait "$pid" || rc=$?
    trap - INT TERM
    return "$rc"
}

# --- tiers ------------------------------------------------------------------

gate_desc() {
    case "$1" in
        tools) echo "pinned tool versions from mk.versions" ;;
        actions) echo "GitHub Actions security & SHA pinning" ;;
        shell) echo "shell script static analysis" ;;
        commits) echo "Conventional Commits format" ;;
        ratchet) echo "coverage, duplication & lint-allow ratchets vs. base" ;;
        fmt) echo "Rust code formatting" ;;
        tombi) echo "TOML formatting & schema lint" ;;
        typos) echo "source & documentation spell check" ;;
        lint) echo "clippy & workspace lints" ;;
        lints-inherit) echo "workspace lint inheritance across crates" ;;
        test-fast) echo "unit & integration tests" ;;
        test) echo "unit, integration & doc tests" ;;
        doc) echo "rustdoc build & link verification" ;;
        deny) echo "licenses, bans, advisories & sources" ;;
        audit) echo "RustSec vulnerability & unmaintained crate check" ;;
        machete) echo "unused dependency detection" ;;
        msrv) echo "minimum supported Rust version check" ;;
        echoes) echo "code duplication & dependency API reuse" ;;
        refquiem) echo "stale symbol reference detection" ;;
        semver) echo "semver API compatibility check" ;;
        wasm) echo "wasm32 target compilation check" ;;
        mutants-diff) echo "mutation testing on branch Rust diff" ;;
        mutants) echo "bounded full-workspace mutation testing" ;;
        coverage | cov) echo "line coverage floor check" ;;
        powerset | hack) echo "feature powerset compilation check" ;;
        miri) echo "undefined behavior check under Miri" ;;
        outdated) echo "direct dependency freshness check" ;;
    esac
}

gate_hint() {
    case "$1" in
        tools) echo "run './mk setup' to install pinned tool versions from mk.versions" ;;
        fmt | tombi | typos) echo "run './mk fix' to auto-format and fix common issues" ;;
        lint) echo "run './mk fix' or use #[expect(lint, reason = \"...\")] for justified exceptions" ;;
        lints-inherit) echo "add '[lints] workspace = true' to every member Cargo.toml" ;;
        commits) echo "use Conventional Commits format: feat|fix|docs|chore|refactor|test: <description>" ;;
        ratchet) echo "ratchets in mk.local and Cargo.toml may only tighten relative to base branch" ;;
        echoes) echo "deduplicate code, or run './mk echoes-baseline' on initial adoption (see docs/GATES.md)" ;;
        refquiem) echo "update stale doc/comment references, or add '// refquiem: allow(name)' with docs/DEBT.md entry" ;;
        mutants | mutants-diff) echo "add a test that kills the surviving mutant, or document an exclusion in .cargo/mutants.toml" ;;
        coverage | cov) echo "add tests to restore line coverage, or run './mk coverage-baseline' when raising the floor" ;;
        msrv) echo "avoid APIs newer than rust-version in Cargo.toml, or bump rust-version intentionally" ;;
    esac
}

# True when a non-empty diff against base_ref touches only documentation, workflow,
# editor, or non-Rust hygiene config files (never *.rs, Cargo.*, crate READMEs, or scripts).
docs_or_meta_only_diff() {
    local base mb changed f
    base="$(base_ref)" || return 1
    mb="$(git merge-base "$base" HEAD 2>/dev/null)" || return 1
    changed="$(
        {
            git diff --name-only "$mb"
            git ls-files --others --exclude-standard
        } | sort -u
    )"
    [ -n "$changed" ] || return 1
    while IFS= read -r f; do
        case "$f" in
            docs/* | .github/* | skills/* | .vscode/* | LICENSE* | NOTICE* | *.md | \
            .typos.toml | .tombi.toml | .refquiem.toml | prek.toml | bacon.toml | renovate.json | .editorconfig | .gitignore)
                case "$f" in */README.md) return 1 ;; esac
                ;;
            *)
                return 1
                ;;
        esac
    done <<<"$changed"
    return 0
}

is_rust_compile_gate() {
    case "$1" in
        fmt | lint | lints-inherit | test | test-fast | doc | deny | machete | msrv | echoes | semver | wasm | mutants-diff)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

idle_scheduled_nightly() {
    [ "${GITHUB_EVENT_NAME:-}" = schedule ] &&
        [ -z "$(git log --since='24 hours ago' --oneline -n 1 2>/dev/null)" ]
}

# Runs every gate as its own process (so `set -e` holds inside each), keeps
# going after a failure, then prints a ran/skipped/failed table.
run_tier() {
    local tier="$1" gate rc start desc hint skip_rust=0
    shift
    if [ "$tier" = ci ] || [ "$tier" = ci-fast ]; then
        if docs_or_meta_only_diff; then skip_rust=1; fi
    fi
    local -a report=()
    local failed=0
    for gate in "$@"; do
        desc="$(gate_desc "$gate")"
        if [ -n "$desc" ]; then
            echo "--- $gate ($desc) ---"
        else
            echo "--- $gate ---"
        fi
        if [ "$skip_rust" = 1 ] && is_rust_compile_gate "$gate"; then
            echo "[skip] $gate: docs/config-only diff against $(base_ref)"
            report+=("SKIP  $gate")
            continue
        fi
        start=$SECONDS
        rc=0
        MK_NESTED=1 "$0" "$gate" || rc=$?
        case "$rc" in
            0) report+=("PASS  $gate ($((SECONDS - start))s)") ;;
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

# A standard gate this repository implements itself: `gate` or `gate=task`.
for override in "${LOCAL_OVERRIDES[@]}"; do
    case "${override%%=*}" in
        # Overriding them would bypass run_mutants' limits and watchdog.
        mutants | mutants-diff)
            die "LOCAL_OVERRIDES cannot replace ${override%%=*}: scope it with MUTANTS_PACKAGES, configure it in .cargo/mutants.toml" ;;
    esac
    if [ "${override%%=*}" = "$cmd" ]; then
        echo "($cmd: this repository's own task '${override#*=}', see LOCAL_OVERRIDES in mk.local)"
        cargo_make "${override#*=}" "$@"
        exit 0
    fi
done

case "$cmd" in
    -h | --help | help)
        cat <<'EOF'
Usage: ./mk <task> [args...]

Tiers:
  check            Fast inner loop (<10s): fmt + tombi + typos + lint
  fix              Auto-fix formatting, TOML, typos, and clippy
  test-fast        Unit & integration tests (nextest / cargo test, no doctests)
  watch            Continuous feedback via bacon / cargo-watch
  ci-fast          Local pre-push gate (skips mutants-diff & network audit; change-aware)
  ci               Full blocking PR gate (all 21 checks + mutants-diff; change-aware)
  ci-nightly       Scheduled deep sweep (mutants, coverage, powerset, miri, outdated)
  ci-release       Release gate (./mk ci + cargo publish --dry-run)

Gate aliases (alongside canonical tool names):
  toml             -> tombi     (TOML formatting & schema lint)
  unused-deps      -> machete   (unused dependency detection)
  duplication, dup -> echoes    (code duplication & dependency API reuse)
  stale-refs       -> refquiem  (stale mentions of deleted or changed symbols)
  feature-powerset -> powerset  (cargo-hack --feature-powerset check)
  cov              -> coverage  (line coverage floor check)

Setup & baselines:
  setup [ci|nightly|all]  Install pinned toolchains and tools from mk.versions
  coverage-baseline       Record today's line coverage floor in mk.local
  echoes-baseline         Record today's echoes ratchet counts in mk.local
  --list                  List all available tasks
EOF
        ;;
    --list | --list-all-steps)
        printf '%s\n' help setup tools check fix test-fast watch bench udeps cov hack \
            toml unused-deps duplication dup stale-refs feature-powerset \
            "${CI_GATES[@]}" "${NIGHTLY_GATES[@]}" \
            ci ci-fast ci-nightly ci-release coverage-baseline echoes-baseline selftest | sort -u
        for t in "${LOCAL_CI_GATES[@]}" "${LOCAL_NIGHTLY_GATES[@]}"; do echo "local:$t"; done
        if [ -f Makefile.toml ]; then echo "(any other name runs that cargo-make task)"; fi
        ;;
    check)
        "$0" fmt
        "$0" tombi
        "$0" typos
        "$0" lint
        ;;
    fix)
        cargo fmt
        require tombi "$TOMBI_VERSION"
        tombi format
        need typos-cli
        typos --write-changes || true
        cargo clippy --workspace --all-targets --all-features --locked --fix --allow-dirty --allow-staged -- -D warnings
        ;;
    test-fast)
        if command -v cargo-nextest >/dev/null 2>&1; then
            cargo nextest run --workspace --all-features --locked --no-tests=pass "$@"
        else
            cargo test --workspace --lib --bins --tests --all-features --locked "$@"
        fi
        ;;
    watch)
        if command -v bacon >/dev/null 2>&1; then
            bacon "${1:-check}"
        elif command -v cargo-watch >/dev/null 2>&1; then
            cargo watch -s "./mk ${1:-check}"
        else
            die "install bacon (cargo binstall bacon) or cargo-watch to use ./mk watch"
        fi
        ;;
    bench)
        cargo bench --workspace --all-features --locked --no-run "$@"
        ;;
    udeps)
        command -v cargo-udeps >/dev/null 2>&1 || skip "cargo-udeps not installed (./mk machete covers unused deps in ci)"
        cargo "+$RUST_NIGHTLY" udeps --workspace --all-targets --all-features --locked
        ;;
    setup)
        tier="${1:-all}"
        if [ "$tier" = nightly ] && idle_scheduled_nightly; then
            echo "note: no commits in the last 24h on scheduled run; installing only audit and outdated"
            install_cargo_tools "$(spec_of cargo-audit)" "$(spec_of cargo-outdated)"
            exit 0
        fi
        if [ "${#LOCAL_SYSTEM_PACKAGES[@]}" -gt 0 ]; then
            if [ "${CI:-}" = true ] && command -v apt-get >/dev/null; then
                sudo apt-get update -qq
                sudo apt-get install -y --no-install-recommends "${LOCAL_SYSTEM_PACKAGES[@]}"
            else
                echo "note: this repository needs the system packages: ${LOCAL_SYSTEM_PACKAGES[*]}"
            fi
        fi
        install_cargo_tools "${LOCAL_CARGO_TOOLS[@]}"
        for t in "${LOCAL_SETUP[@]}"; do cargo_make "$t"; done
        rustup toolchain install # from rust-toolchain.toml
        rustup toolchain install "$(msrv)" --profile minimal
        install_tombi
        install_shellcheck
        install_cargo_tools "${CARGO_TOOLS_CI[@]}"
        if [ "$tier" = all ] || [ "$tier" = nightly ]; then
            rustup toolchain install "$RUST_NIGHTLY" --profile minimal --component miri,rust-src
            rustup component add llvm-tools
            install_cargo_tools "${CARGO_TOOLS_NIGHTLY[@]}"
        fi
        ;;
    tools)
        require tombi "$TOMBI_VERSION"
        require shellcheck "$SHELLCHECK_VERSION"
        require_tools "${CARGO_TOOLS_CI[@]}" "${LOCAL_CARGO_TOOLS[@]}"
        ;;
    actions)
        [ -d .github ] || skip "no .github directory"
        need zizmor
        zizmor --offline .github
        ;;
    shell)
        require shellcheck "$SHELLCHECK_VERSION"
        # Every tracked shell script, found by extension or shebang.
        scripts=()
        while IFS= read -r f; do
            case "$f" in
                *.sh) scripts+=("$f") ;;
                *) if head -n1 "$f" 2>/dev/null | grep -Eq '^#!.*[/ ](bash|sh)\b'; then scripts+=("$f"); fi ;;
            esac
        done < <(git ls-files)
        [ "${#scripts[@]}" -gt 0 ] || skip "no shell scripts"
        shellcheck --external-sources --severity=style "${scripts[@]}"
        ;;
    ratchet)
        # The ratchets in mk.local and Cargo.toml may only tighten relative to the base branch:
        # the coverage floor goes up, the echoes legacy counts and Cargo.toml allow lines go down.
        base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
        mb="$(git merge-base "$base" HEAD)"
        base_cargo="$(git show "$mb:Cargo.toml" 2>/dev/null || true)"
        if grep -Eq '^\[(workspace\.)?lints\.' <<<"$base_cargo"; then
            old_allows="$(grep -c '= *"allow"' <<<"$base_cargo" || true)"
            new_allows="$(grep -c '= *"allow"' Cargo.toml || true)"
            if [ "$new_allows" -gt "$old_allows" ]; then
                die "Cargo.toml lint allow count went up from $old_allows to $new_allows (use #[expect(.., reason)] at the site)"
            fi
        fi
        base_local="$(git show "$mb:mk.local" 2>/dev/null || true)"
        [ -n "$base_local" ] || exit 0
        old_value() { sed -n "s/^$1=\"*\([0-9]*\)\"*.*/\1/p" <<<"$base_local"; }
        old="$(old_value COVERAGE_FAIL_UNDER_LINES)"
        if [ -n "$old" ] && [ "$COVERAGE_FAIL_UNDER_LINES" -lt "$old" ]; then
            die "COVERAGE_FAIL_UNDER_LINES went down from $old to $COVERAGE_FAIL_UNDER_LINES"
        fi
        for key in ECHOES_MAX_APPROXIMATE ECHOES_MAX_REUSE; do
            old="$(old_value "$key")"
            if [ -n "$old" ] && [ "${!key}" -gt "$old" ]; then
                die "$key went up from $old to ${!key}"
            fi
        done
        ;;
    commits)
        if [ $# -gt 0 ] && [ -f "$1" ]; then
            subject="$(grep -v '^#' "$1" | head -1)"
            grep -Eq "$CONVENTIONAL" <<<"$subject" || die "not a conventional commit: $subject"
            exit 0
        fi
        base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
        bad=0
        while IFS= read -r subject; do
            if ! grep -Eq "$CONVENTIONAL" <<<"$subject"; then
                echo "not a conventional commit: $subject"
                bad=1
            fi
        done < <(git log --no-merges --format=%s "$(git merge-base "$base" HEAD)..HEAD")
        [ "$bad" = 0 ]
        ;;
    # Workspace members only: --all would also check (and `cargo fmt --all` rewrite)
    # path dependencies that live in other repositories.
    fmt) cargo fmt -- --check ;;
    tombi)
        require tombi "$TOMBI_VERSION"
        tombi format --check
        # Also fails on a config tombi cannot parse (format only warns and uses defaults).
        tombi lint --error-on-warnings
        ;;
    typos) need typos-cli; typos ;;
    lint) cargo clippy --workspace --all-targets --all-features --locked -- -D warnings ;;
    lints-inherit)
        if ! grep -q '^\[workspace\.lints' Cargo.toml; then
            # Single crate: the lints live in its own manifest.
            for sec in clippy rust rustdoc; do
                grep -q "^\[lints\.$sec\]" Cargo.toml || die "Cargo.toml is missing [lints.$sec]"
            done
            exit 0
        fi
        for sec in clippy rust rustdoc; do
            grep -q "^\[workspace\.lints\.$sec\]" Cargo.toml || die "Cargo.toml is missing [workspace.lints.$sec]"
        done
        # Workspace lints only apply to crates that opt in.
        missing=0
        for manifest in $(cargo metadata --no-deps --format-version 1 --locked | grep -o '"manifest_path":"[^"]*"' | cut -d'"' -f4); do
            awk '/^\[lints\]/{s=1;next} /^\[/{s=0} s && /^workspace *= *true/{f=1} END{exit !f}' "$manifest" ||
                { echo "missing '[lints] workspace = true' in $manifest"; missing=1; }
        done
        [ "$missing" = 0 ]
        ;;
    test) cargo test --workspace --all-features --locked ;;
    doc) RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --document-private-items --locked ;;
    deny) need cargo-deny; cargo deny --all-features --locked check ;;
    audit) need cargo-audit; cargo audit --deny warnings ;;
    machete) need cargo-machete; cargo machete ;;
    msrv)
        v="$(msrv)"
        [ -n "$v" ] || die "set rust-version in Cargo.toml"
        rustup run "$v" true 2>/dev/null || die "MSRV toolchain $v is not installed; run ./mk setup"
        cargo "+$v" check --workspace --all-targets --all-features --locked
        ;;
    echoes)
        need echoes
        # echoes silently falls back to a shallow scan without its config, so insist on the deep one.
        for key in 'functions = true' 'context-blocks = true' 'fail-on-findings = true'; do
            grep -q "^$key" .echoes.toml 2>/dev/null || die ".echoes.toml must set '$key'"
        done
        grep -Eq '^depth = [1-9]' .echoes.toml || die ".echoes.toml must set [context-dependencies] depth > 0"
        if [ "${GITHUB_ACTIONS:-}" = true ]; then set -- --github; fi
        # --no-fail: the counts are judged here, against the ratchet in mk.local.
        # An incomplete analysis still exits non-zero and fails the gate.
        rc=0
        out="$(cargo echoes --all-targets --config .echoes.toml --no-fail "$@" 2>&1)" || rc=$?
        printf '%s\n' "$out"
        [ "$rc" = 0 ] || die "echoes could not complete its analysis (exit $rc)"
        count() { grep -oE "[0-9]+ $1" <<<"$out" | head -1 | cut -d' ' -f1; }
        exact="$(count 'duplicated groups')"
        approximate="$(count 'approximate groups')"
        reuse="$(count 'dependency reuse')"
        [ -n "$exact" ] && [ -n "$approximate" ] && [ -n "$reuse" ] || die "could not read the echoes summary"
        [ "$exact" = 0 ] || die "$exact exact duplicate group(s): delete all but one copy"
        [ "$approximate" -le "$ECHOES_MAX_APPROXIMATE" ] ||
            die "$approximate approximate duplicate groups, more than ECHOES_MAX_APPROXIMATE=$ECHOES_MAX_APPROXIMATE in mk.local"
        [ "$reuse" -le "$ECHOES_MAX_REUSE" ] ||
            die "$reuse dependency reuse opportunities, more than ECHOES_MAX_REUSE=$ECHOES_MAX_REUSE in mk.local"
        if [ "$approximate" -lt "$ECHOES_MAX_APPROXIMATE" ] || [ "$reuse" -lt "$ECHOES_MAX_REUSE" ]; then
            echo "note: lower the echoes ratchet in mk.local to approximate=$approximate reuse=$reuse (./mk echoes-baseline)"
        fi
        ;;
    echoes-baseline)
        # Adoption: record today's legacy echoes counts as the ratchet (exact duplicates stay 0).
        out="$(MK_NESTED=0 "$0" echoes 2>&1 || true)"
        approximate="$(grep -oE '[0-9]+ approximate groups' <<<"$out" | head -1 | cut -d' ' -f1)"
        reuse="$(grep -oE '[0-9]+ dependency reuse' <<<"$out" | head -1 | cut -d' ' -f1)"
        [ -n "$approximate" ] && [ -n "$reuse" ] || die "could not read the echoes summary"
        for kv in "ECHOES_MAX_APPROXIMATE=$approximate" "ECHOES_MAX_REUSE=$reuse"; do
            key="${kv%%=*}"
            if grep -q "^$key=" mk.local 2>/dev/null; then
                sed -i.bak "s/^$key=.*/$kv/" mk.local && rm mk.local.bak
            else
                printf '%s\n' "$kv" >>mk.local
            fi
        done
        echo "echoes ratchet: approximate=$approximate reuse=$reuse (mk.local)"
        ;;
    refquiem)
        need refquiem
        [ "$(git rev-parse --is-shallow-repository)" = false ] || die "refquiem needs full git history (fetch-depth: 0)"
        for key in 'scan-commented-code = true' 'level = "deny"'; do
            grep -q "^$key" .refquiem.toml 2>/dev/null || die ".refquiem.toml must set '$key'"
        done
        ! grep -Eq '^since *=' .refquiem.toml || die ".refquiem.toml must not truncate history with 'since'"
        if [ "${GITHUB_ACTIONS:-}" = true ]; then set -- --github "$@"; fi
        cargo refquiem --config .refquiem.toml "$@"
        has_range=0
        for arg in "$@"; do
            case "$arg" in --base | --base=* | --since | --since=*) has_range=1 ;; esac
        done
        if [ "$has_range" = 0 ] && base="$(base_ref)"; then
            mb="$(git merge-base "$base" HEAD)"
            if [ "$mb" != "$(git rev-parse HEAD)" ] || ! git diff --quiet HEAD; then
                cargo refquiem --config .refquiem.toml --base "$mb" "$@"
            fi
        fi
        ;;
    semver)
        need cargo-semver-checks
        has_lib || skip "no library target"
        # Pre-1.0 crates are in development: breaking changes need no version bump.
        if ! cargo metadata --no-deps --format-version 1 --locked | grep -o '"version":"[^"]*"' | grep -qv '"version":"0\.'; then
            skip "every crate is pre-1.0 (in development), so breaking changes are allowed"
        fi
        base="$(git tag --merged HEAD --no-contains HEAD --list 'v*' --sort=-v:refname | head -1)"
        [ -n "$base" ] || skip "no earlier v* release tag to compare against"
        cargo semver-checks --workspace --all-features --baseline-rev "$base"
        ;;
    wasm)
        has_wasm || skip "no wasm32 target declared in rust-toolchain.toml or Cargo.toml"
        rustup target add wasm32-unknown-unknown
        cargo check --workspace --target wasm32-unknown-unknown --all-features --locked
        ;;
    mutants)
        # Exit 2 = missed mutants, 3 = timeouts: both fail. Exclusions live in .cargo/mutants.toml.
        # Extra arguments go to cargo-mutants: `./mk mutants -p crate` runs one crate.
        run_mutants "$@"
        ;;
    mutants-diff)
        # Mutants in code changed against the base branch (and uncommitted changes), on every PR.
        need cargo-mutants
        base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
        diff="$(mktemp)"
        trap 'rm -f "$diff"' EXIT
        git diff --no-ext-diff --no-color "$(git merge-base "$base" HEAD)" -- '*.rs' >"$diff"
        [ -s "$diff" ] || skip "no Rust changes against $base"
        run_mutants --in-diff "$diff"
        ;;
    coverage | cov)
        need cargo-llvm-cov
        cargo llvm-cov --workspace --all-features --locked --lcov --output-path lcov.info \
            --fail-under-lines "$COVERAGE_FAIL_UNDER_LINES"
        ;;
    coverage-baseline)
        # Adoption or after adding tests: set the floor to today's coverage (rounded down).
        need cargo-llvm-cov
        pct="$(cargo llvm-cov --workspace --all-features --locked --summary-only | awk '/^TOTAL/{sub(/%/, "", $10); print int($10)}')"
        [ -n "$pct" ] || die "could not read line coverage"
        if grep -q '^COVERAGE_FAIL_UNDER_LINES=' mk.local 2>/dev/null; then
            sed -i.bak "s/^COVERAGE_FAIL_UNDER_LINES=.*/COVERAGE_FAIL_UNDER_LINES=\"$pct\"/" mk.local && rm mk.local.bak
        else
            printf '\n# Ratchet: may only go up (./mk ratchet). Set with ./mk coverage-baseline.\nCOVERAGE_FAIL_UNDER_LINES="%s"\n' "$pct" >>mk.local
        fi
        echo "COVERAGE_FAIL_UNDER_LINES: $COVERAGE_FAIL_UNDER_LINES -> $pct (the ratchet gate rejects a decrease against the base branch)"
        ;;
    powerset | hack) need cargo-hack; cargo hack --workspace --feature-powerset --no-dev-deps check ;;
    miri)
        # Undefined behaviour, not sandboxing: tests may read the clock and the
        # environment. MIRI_PACKAGES in mk.local limits it to the crates where
        # it means something (miri cannot spawn processes or open sockets).
        cargo "+$RUST_NIGHTLY" miri --version >/dev/null 2>&1 || die "miri on $RUST_NIGHTLY is not installed; run ./mk setup nightly"
        export MIRIFLAGS="${MIRIFLAGS:--Zmiri-disable-isolation -Zmiri-tree-borrows}"
        if [ "${#MIRI_PACKAGES[@]}" -gt 0 ]; then
            cargo "+$RUST_NIGHTLY" miri test --all-features --locked "${MIRI_PACKAGES[@]/#/--package=}"
        else
            cargo "+$RUST_NIGHTLY" miri test --workspace --all-features --locked
        fi
        ;;
    outdated) need cargo-outdated; cargo outdated --workspace --root-deps-only --exit-code 1 ;;
    ci) run_tier ci "${CI_GATES[@]}" "${LOCAL_CI_GATES[@]/#/local:}" ;;
    ci-fast) run_tier ci-fast "${CI_FAST_GATES[@]}" "${LOCAL_CI_GATES[@]/#/local:}" ;;
    ci-nightly)
        if idle_scheduled_nightly; then
            echo "note: no commits in the last 24h on scheduled run; running only advisory/freshness gates (audit, outdated)"
            run_tier ci-nightly audit outdated
        else
            run_tier ci-nightly "${NIGHTLY_GATES[@]}" "${LOCAL_NIGHTLY_GATES[@]/#/local:}"
        fi
        ;;
    local:*) cargo_make "${cmd#local:}" "$@" ;;
    ci-release)
        "$0" ci
        if cargo metadata --no-deps --format-version 1 --locked | grep -q '"publish":null'; then
            cargo publish --workspace --dry-run --locked
        else
            echo "[skip] publish: every crate sets publish = false"
        fi
        ;;
    selftest)
        [ -x tests/gate_selftest.sh ] || skip "the gate selftest lives in the rusti-graben template only"
        tests/gate_selftest.sh
        ;;
    *)
        # Anything else is a repository task (task_<name> function in mk.local or cargo-make).
        if declare -F "task_$cmd" >/dev/null || [ -f Makefile.toml ]; then
            cargo_make "$cmd" "$@"
            exit 0
        fi
        echo "Usage: ./mk {$(./mk --list | grep -v '^(' | paste -sd'|' -)}"
        exit 1
        ;;
esac
