#!/usr/bin/env bash
# rusti-graben standard runner. Identical in every repository: scripts/adopt.sh
# keeps it in sync, so edit it in the template, never here. Repository-specific
# gates, overrides and tools go in mk.local.
#
# Single entrypoint for every gate. CI calls exactly these tasks.
# A gate either runs and can fail, or is skipped because it cannot apply
# (printed with a reason). A missing tool, or one at the wrong version, is a failure.
set -euo pipefail

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
COVERAGE_FAIL_UNDER_LINES=100
# shellcheck source=mk.local
if [ -f mk.local ]; then . ./mk.local; fi

SKIP_RC=78
CI_GATES=(tools actions shell commits ratchet fmt tombi typos lint lints-inherit test doc deny audit machete msrv echoes semver wasm mutants-diff)
NIGHTLY_GATES=(mutants coverage powerset miri outdated)
# Run on demand only (./mk ci-experimental): not mature enough to block, kept ready to promote.
EXPERIMENTAL_GATES=(refquiem)

cmd="${1:-ci}"
shift || true

die() { echo "ERROR: $*" >&2; exit 1; }

cargo_make() {
    [ -f Makefile.toml ] || die "no Makefile.toml for cargo-make task '$1'"
    need cargo-make
    exec cargo make --no-workspace "$@"
}


# Not applicable here: exit 0 when run directly, SKIP_RC when run by a tier.
skip() {
    echo "[skip] $cmd: $*"
    if [ "${MK_NESTED:-0}" = 1 ]; then exit "$SKIP_RC"; fi
    exit 0
}

has_lib() { cargo metadata --no-deps --format-version 1 --locked | grep -q '"kind":\["lib"\]'; }
has_wasm() { grep -q 'wasm32' rust-toolchain.toml Cargo.toml 2>/dev/null; }
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
    for spec in "${CARGO_TOOLS_CI[@]}" "${CARGO_TOOLS_NIGHTLY[@]}" "${CARGO_TOOLS_EXPERIMENTAL[@]}" "${LOCAL_CARGO_TOOLS[@]}"; do
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

# --- tiers ------------------------------------------------------------------

# Runs every gate as its own process (so `set -e` holds inside each), keeps
# going after a failure, then prints a ran/skipped/failed table.
run_tier() {
    local tier="$1" gate rc start
    shift
    local -a report=()
    local failed=0
    for gate in "$@"; do
        echo "--- $gate ---"
        start=$SECONDS
        rc=0
        MK_NESTED=1 "$0" "$gate" || rc=$?
        case "$rc" in
            0) report+=("PASS  $gate ($((SECONDS - start))s)") ;;
            "$SKIP_RC") report+=("SKIP  $gate") ;;
            *) report+=("FAIL  $gate (exit $rc)"); failed=1 ;;
        esac
    done
    echo "=== $tier summary ==="
    printf '%s\n' "${report[@]}"
    if [ "$tier" = ci ]; then echo "(not enforced: ${EXPERIMENTAL_GATES[*]}; run ./mk ci-experimental)"; fi
    [ "$failed" = 0 ] || die "$tier failed"
    echo "==> $tier passed"
}

# A standard gate this repository implements itself: `gate` or `gate=task`.
for override in "${LOCAL_OVERRIDES[@]}"; do
    if [ "${override%%=*}" = "$cmd" ]; then
        echo "($cmd: this repository's own task '${override#*=}', see LOCAL_OVERRIDES in mk.local)"
        cargo_make "${override#*=}" "$@"
    fi
done

case "$cmd" in
    --list | --list-all-steps)
        printf '%s\n' setup tools "${CI_GATES[@]}" "${NIGHTLY_GATES[@]}" "${EXPERIMENTAL_GATES[@]}" \
            ci ci-nightly ci-release ci-experimental coverage-baseline selftest | sort -u
        for t in "${LOCAL_CI_GATES[@]}" "${LOCAL_NIGHTLY_GATES[@]}"; do echo "local:$t"; done
        if [ -f Makefile.toml ]; then echo "(any other name runs that cargo-make task)"; fi
        ;;
    setup)
        tier="${1:-all}"
        if [ "${#LOCAL_SYSTEM_PACKAGES[@]}" -gt 0 ]; then
            if [ "${CI:-}" = true ] && command -v apt-get >/dev/null; then
                sudo apt-get update -qq
                sudo apt-get install -y --no-install-recommends "${LOCAL_SYSTEM_PACKAGES[@]}"
            else
                echo "note: this repository needs the system packages: ${LOCAL_SYSTEM_PACKAGES[*]}"
            fi
        fi
        install_cargo_tools "${LOCAL_CARGO_TOOLS[@]}"
        for t in "${LOCAL_SETUP[@]}"; do cargo make --no-workspace "$t"; done
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
        if [ "$tier" = all ] || [ "$tier" = experimental ]; then
            install_cargo_tools "${CARGO_TOOLS_EXPERIMENTAL[@]}"
        fi
        ;;
    tools)
        require tombi "$TOMBI_VERSION"
        require shellcheck "$SHELLCHECK_VERSION"
        require_tools "${CARGO_TOOLS_CI[@]}" "${LOCAL_CARGO_TOOLS[@]}"
        ;;
    actions) need zizmor; zizmor --offline .github ;;
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
        # The coverage floor in mk.local may only go up relative to the base branch.
        base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
        old="$( (git show "$(git merge-base "$base" HEAD):mk.local" 2>/dev/null || true) | sed -n 's/^COVERAGE_FAIL_UNDER_LINES="*\([0-9]*\)"*/\1/p')"
        [ -n "$old" ] || skip "no ratchet on $base yet"
        [ "$COVERAGE_FAIL_UNDER_LINES" -ge "$old" ] ||
            die "COVERAGE_FAIL_UNDER_LINES went down from $old to $COVERAGE_FAIL_UNDER_LINES"
        ;;
    commits)
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
    fmt) cargo fmt --all -- --check ;;
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
            grep -q '^\[lints\.' Cargo.toml || die "Cargo.toml has no [lints.*] or [workspace.lints.*] tables"
            exit 0
        fi
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
        cargo echoes --all-targets --config .echoes.toml "$@"
        ;;
    refquiem)
        # Experimental: not in any tier until refquiem matures (see EXPERIMENTAL_GATES).
        need refquiem
        [ "$(git rev-parse --is-shallow-repository)" = false ] || die "refquiem needs full git history (fetch-depth: 0)"
        if [ "${GITHUB_ACTIONS:-}" = true ]; then cargo refquiem --github; else cargo refquiem; fi
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
        need cargo-mutants
        # Exit 2 = missed mutants, 3 = timeouts: both fail. Exclusions live in .cargo/mutants.toml.
        cargo mutants --workspace --all-features
        ;;
    mutants-diff)
        # Mutants in code changed against the base branch (and uncommitted changes), on every PR.
        need cargo-mutants
        base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
        diff="$(mktemp)"
        trap 'rm -f "$diff"' EXIT
        git diff --no-ext-diff --no-color "$(git merge-base "$base" HEAD)" -- '*.rs' >"$diff"
        [ -s "$diff" ] || skip "no Rust changes against $base"
        cargo mutants --workspace --all-features --in-diff "$diff"
        ;;
    coverage)
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
    powerset) need cargo-hack; cargo hack --workspace --feature-powerset --no-dev-deps check ;;
    miri) cargo "+$RUST_NIGHTLY" miri test --workspace --all-features --locked ;;
    outdated) need cargo-outdated; cargo outdated --workspace --root-deps-only --exit-code 1 ;;
    ci) run_tier ci "${CI_GATES[@]}" "${LOCAL_CI_GATES[@]/#/local:}" ;;
    ci-nightly) run_tier ci-nightly "${NIGHTLY_GATES[@]}" "${LOCAL_NIGHTLY_GATES[@]/#/local:}" ;;
    local:*) cargo_make "${cmd#local:}" "$@" ;;
    ci-experimental) run_tier ci-experimental "${EXPERIMENTAL_GATES[@]}" ;;
    ci-release)
        "$0" ci
        cargo publish --workspace --dry-run --locked
        ;;
    selftest)
        [ -x tests/gate_selftest.sh ] || skip "the gate selftest lives in the rusti-graben template only"
        tests/gate_selftest.sh
        ;;
    *)
        # Anything else is a repository task (cargo-make), when there is a Makefile.toml.
        if [ -f Makefile.toml ]; then cargo_make "$cmd" "$@"; fi
        echo "Usage: ./mk {$(./mk --list | grep -v '^(' | paste -sd'|' -)}"
        exit 1
        ;;
esac
