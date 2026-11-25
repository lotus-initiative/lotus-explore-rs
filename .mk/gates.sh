# shellcheck shell=bash
# Layer 3: Individual gate command handlers (inner-loop, hygiene, security,
# supply-chain, and ratchet gates).

run_for_affected_packages() {
    local subcmd="$1" empty_mode="$2" empty_msg="$3" base p
    shift 3
    if base="$(base_ref)"; then
        local -a pkgs=()
        while IFS= read -r p; do [ -n "$p" ] && pkgs+=("$p"); done < <(affected_packages "$base")
        if [ "${#pkgs[@]}" -eq 0 ]; then
            if [ "$empty_mode" = skip ]; then skip "$empty_msg against $base"; fi
            echo "[skip] $empty_msg against $base"
            return 0
        fi
        "$0" "$subcmd" "${pkgs[@]/#/--package=}" "$@"
    else
        "$0" "$subcmd" "$@"
    fi
}

cmd_check() {
    "$0" fmt
    "$0" tombi
    "$0" typos
    "$0" lint "$@"
}

cmd_check_changed() {
    "$0" fmt
    "$0" tombi
    "$0" typos
    run_for_affected_packages lint echo "lint: no affected Rust packages" "$@"
}

# shellcheck disable=SC2154  # TOMBI_VERSION from mk.versions
cmd_fix() {
    cargo fmt
    require tombi "$TOMBI_VERSION"
    tombi format
    need typos-cli
    typos --write-changes || true
    cargo clippy --workspace --all-targets --all-features --locked --fix --allow-dirty --allow-staged -- -D warnings
}

cmd_test_fast() {
    local -a scope=(--workspace)
    if has_pkg_flag "$@"; then scope=(); fi
    if command -v cargo-nextest >/dev/null 2>&1; then
        cargo nextest run ${scope[@]+"${scope[@]}"} --all-features --locked --no-tests=pass "$@"
    else
        cargo test ${scope[@]+"${scope[@]}"} --lib --bins --tests --all-features --locked "$@"
    fi
}

cmd_test_changed() {
    run_for_affected_packages test-fast skip "no affected Rust packages" "$@"
}

cmd_watch() {
    if command -v bacon >/dev/null 2>&1; then
        bacon "${1:-check}"
    elif command -v cargo-watch >/dev/null 2>&1; then
        cargo watch -s "./mk ${1:-check}"
    else
        die "install bacon (cargo binstall bacon) or cargo-watch to use ./mk watch"
    fi
}

cmd_bench() { cargo bench --workspace --all-features --locked --no-run "$@"; }

# shellcheck disable=SC2154  # RUST_NIGHTLY from mk.versions
cmd_udeps() {
    command -v cargo-udeps >/dev/null 2>&1 || skip "cargo-udeps not installed (./mk machete covers unused deps in ci)"
    cargo "+$RUST_NIGHTLY" udeps --workspace --all-targets --all-features --locked
}

cmd_actions() {
    [ -d .github ] || skip "no .github directory"
    need zizmor
    zizmor --offline .github
}

# shellcheck disable=SC2154  # SHELLCHECK_VERSION from mk.versions
cmd_shell() {
    require shellcheck "$SHELLCHECK_VERSION"
    local f
    local -a scripts=()
    while IFS= read -r f; do
        case "$f" in
            *.sh) scripts+=("$f") ;;
            *) if head -n1 "$f" 2>/dev/null | grep -Eq '^#!.*[/ ](bash|sh)\b'; then scripts+=("$f"); fi ;;
        esac
    done < <(git ls-files)
    [ "${#scripts[@]}" -gt 0 ] || skip "no shell scripts"
    shellcheck --external-sources --severity=style "${scripts[@]}"
}

count_lint_allows() {
    awk '/^\[(workspace\.)?lints\./{s=1;next} /^\[/{s=0} s && $0 !~ /^[[:space:]]*#/ && /= *"allow"/{c++} END{print c+0}'
}

ratchet_old_value() {
    sed -n "s/^$1=\"*\([0-9][0-9]*\)\"*.*/\1/p" <<<"$2" | head -1
}

# shellcheck disable=SC2154  # ratchet vars from mk/mk.local
validate_ratchet_inputs() {
    [[ "$COVERAGE_FAIL_UNDER_LINES" =~ ^[0-9]+$ ]] && [ "$COVERAGE_FAIL_UNDER_LINES" -le 100 ] ||
        die "COVERAGE_FAIL_UNDER_LINES must be an integer 0..=100, got '$COVERAGE_FAIL_UNDER_LINES'"
    local key
    for key in ECHOES_MAX_APPROXIMATE ECHOES_MAX_REUSE; do
        [[ "${!key}" =~ ^[0-9]+$ ]] || die "$key must be a non-negative integer, got '${!key}'"
    done
}

# shellcheck disable=SC2154  # ratchet vars from mk/mk.local
cmd_ratchet() {
    validate_ratchet_inputs
    local base mb base_cargo old_allows new_allows base_local old key
    base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
    mb="$(git merge-base "$base" HEAD)"
    base_cargo="$(git show "$mb:Cargo.toml" 2>/dev/null || true)"
    if grep -Eq '^\[(workspace\.)?lints\.' <<<"$base_cargo"; then
        old_allows="$(count_lint_allows <<<"$base_cargo")"
        new_allows="$(count_lint_allows <Cargo.toml)"
        [ "$new_allows" -le "$old_allows" ] ||
            die "Cargo.toml lint allow count went up from $old_allows to $new_allows (use #[expect(.., reason)] at the site)"
    fi
    base_local="$(git show "$mb:mk.local" 2>/dev/null || true)"
    old="$(ratchet_old_value COVERAGE_FAIL_UNDER_LINES "$base_local")"
    if [ -z "$old" ] && git cat-file -e "$mb:mk" 2>/dev/null; then old=100; fi
    if [ -n "$old" ] && [ "$COVERAGE_FAIL_UNDER_LINES" -lt "$old" ]; then
        die "COVERAGE_FAIL_UNDER_LINES went down from $old to $COVERAGE_FAIL_UNDER_LINES"
    fi
    for key in ECHOES_MAX_APPROXIMATE ECHOES_MAX_REUSE; do
        old="$(ratchet_old_value "$key" "$base_local")"
        if [ -z "$old" ] && git cat-file -e "$mb:.echoes.toml" 2>/dev/null; then old=0; fi
        if [ -n "$old" ] && [ "${!key}" -gt "$old" ]; then die "$key went up from $old to ${!key}"; fi
    done
}

# shellcheck disable=SC2154  # CONVENTIONAL_COMMIT_RE from .mk/catalog.sh
cmd_commits() {
    local subject base bad=0
    if [ $# -gt 0 ] && [ -f "$1" ]; then
        subject="$(grep -v '^#' "$1" | head -1 || true)"
        grep -Eq "$CONVENTIONAL_COMMIT_RE" <<<"$subject" || die "not a conventional commit: $subject"
        return 0
    fi
    base="$(base_ref)" || skip "no base branch to compare with (set MK_BASE)"
    while IFS= read -r subject; do
        if ! grep -Eq "$CONVENTIONAL_COMMIT_RE" <<<"$subject"; then
            echo "not a conventional commit: $subject"
            bad=1
        fi
    done < <(git log --no-merges --format=%s "$(git merge-base "$base" HEAD)..HEAD")
    [ "$bad" = 0 ]
}

cmd_fmt() { cargo fmt -- --check; }

# shellcheck disable=SC2154  # TOMBI_VERSION from mk.versions
cmd_tombi() {
    require tombi "$TOMBI_VERSION"
    tombi format --check
    # Also fails on a config tombi cannot parse (format only warns and uses defaults).
    tombi lint --error-on-warnings
}

cmd_typos() { need typos-cli; typos; }

cmd_lint() {
    local -a scope=(--workspace)
    if has_pkg_flag "$@"; then scope=(); fi
    cargo clippy ${scope[@]+"${scope[@]}"} --all-targets --all-features --locked "$@" -- -D warnings
}

cmd_lints_inherit() {
    local sec manifest missing=0
    if ! grep -q '^\[workspace\.lints' Cargo.toml; then
        for sec in clippy rust rustdoc; do
            grep -q "^\[lints\.$sec\]" Cargo.toml || die "Cargo.toml is missing [lints.$sec]"
        done
        return 0
    fi
    for sec in clippy rust rustdoc; do
        grep -q "^\[workspace\.lints\.$sec\]" Cargo.toml || die "Cargo.toml is missing [workspace.lints.$sec]"
    done
    while IFS= read -r manifest; do
        awk '/^\[lints\]/{s=1;next} /^\[/{s=0} s && /^workspace *= *true/{f=1} END{exit !f}' "$manifest" ||
            { echo "missing '[lints] workspace = true' in $manifest"; missing=1; }
    done < <(meta_py manifests --locked)
    [ "$missing" = 0 ]
}

cmd_test() {
    need cargo-nextest
    local -a nextest_args=(--workspace --all-features --locked --no-tests=pass)
    if [ -n "${CI:-}" ] && [ -f .config/nextest.toml ] && grep -Eq '^\[profile\.ci\]' .config/nextest.toml; then
        nextest_args+=(--profile ci)
    fi
    cargo nextest run "${nextest_args[@]}" "$@"
    if meta_py has-lib --locked; then cargo test --workspace --doc --all-features --locked; fi
}

cmd_doc() { RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --document-private-items --locked; }
cmd_deny() { need cargo-deny; cargo deny --all-features --locked check; }
cmd_audit() { need cargo-audit; cargo audit --deny warnings; }
cmd_machete() { need cargo-machete; cargo machete; }

cmd_msrv() {
    local v active_rustc
    v="$(msrv)"
    [ -n "$v" ] || die "set rust-version in Cargo.toml"
    if [ "${MK_NESTED:-0}" = 1 ] && [ "${MK_LINT_PASSED:-0}" = 1 ]; then
        active_rustc="$(rustc --version 2>/dev/null | awk '{print $2}')"
        [ "$v" != "$active_rustc" ] || skip "already verified by lint on rustc $v"
    fi
    rustup run "$v" true 2>/dev/null || die "MSRV toolchain $v is not installed; run ./mk setup"
    cargo "+$v" check --workspace --all-targets --all-features --locked
}

echoes_metric() {
    grep -oE "[0-9]+ $2" <<<"$1" | head -1 | cut -d' ' -f1
}

# shellcheck disable=SC2154  # ECHOES_MAX_* from mk/mk.local
cmd_echoes() {
    need echoes
    local key rc=0 out exact approximate reuse
    for key in 'functions = true' 'context-blocks = true' 'fail-on-findings = true'; do
        grep -q "^$key" .echoes.toml 2>/dev/null || die ".echoes.toml must set '$key'"
    done
    grep -Eq '^depth = [1-9]' .echoes.toml || die ".echoes.toml must set [context-dependencies] depth > 0"
    if [ "${GITHUB_ACTIONS:-}" = true ]; then set -- --github "$@"; fi
    out="$(cargo echoes --all-targets --config .echoes.toml --no-fail "$@" 2>&1)" || rc=$?
    printf '%s\n' "$out"
    [ "$rc" = 0 ] || die "echoes could not complete its analysis (exit $rc)"
    exact="$(echoes_metric "$out" "duplicated groups")"
    approximate="$(echoes_metric "$out" "approximate groups")"
    reuse="$(echoes_metric "$out" "dependency reuse")"
    [ -n "$exact" ] && [ -n "$approximate" ] && [ -n "$reuse" ] || die "could not read the echoes summary"
    [ "$exact" = 0 ] || die "$exact exact duplicate group(s): delete all but one copy"
    [ "$approximate" -le "$ECHOES_MAX_APPROXIMATE" ] ||
        die "$approximate approximate duplicate groups, more than ECHOES_MAX_APPROXIMATE=$ECHOES_MAX_APPROXIMATE in mk.local"
    [ "$reuse" -le "$ECHOES_MAX_REUSE" ] ||
        die "$reuse dependency reuse opportunities, more than ECHOES_MAX_REUSE=$ECHOES_MAX_REUSE in mk.local"
    if [ "$approximate" -lt "$ECHOES_MAX_APPROXIMATE" ] || [ "$reuse" -lt "$ECHOES_MAX_REUSE" ]; then
        echo "note: lower the echoes ratchet in mk.local to approximate=$approximate reuse=$reuse (./mk echoes-baseline)"
    fi
}

cmd_echoes_baseline() {
    local out approximate reuse kv
    out="$(MK_NESTED=0 "$0" echoes 2>&1 || true)"
    approximate="$(echoes_metric "$out" "approximate groups")"
    reuse="$(echoes_metric "$out" "dependency reuse")"
    [ -n "$approximate" ] && [ -n "$reuse" ] || die "could not read the echoes summary"
    for kv in "ECHOES_MAX_APPROXIMATE=$approximate" "ECHOES_MAX_REUSE=$reuse"; do
        upsert_kv_line mk.local "${kv%%=*}" "${kv#*=}"
    done
    echo "echoes ratchet: approximate=$approximate reuse=$reuse (mk.local)"
}

cmd_refquiem() {
    need refquiem
    local key arg has_range=0 base mb
    [ "$(git rev-parse --is-shallow-repository)" = false ] || die "refquiem needs full git history (fetch-depth: 0)"
    for key in 'scan-commented-code = true' 'level = "deny"'; do
        grep -q "^$key" .refquiem.toml 2>/dev/null || die ".refquiem.toml must set '$key'"
    done
    ! grep -Eq '^since *=' .refquiem.toml || die ".refquiem.toml must not truncate history with 'since'"
    if [ "${GITHUB_ACTIONS:-}" = true ]; then set -- --github "$@"; fi
    cargo refquiem --config .refquiem.toml "$@"
    for arg in "$@"; do
        case "$arg" in --base | --base=* | --since | --since=*) has_range=1 ;; esac
    done
    if [ "$has_range" = 0 ] && base="$(base_ref)"; then
        mb="$(git merge-base "$base" HEAD)"
        if [ "$mb" != "$(git rev-parse HEAD)" ] || ! git diff --quiet HEAD; then
            cargo refquiem --config .refquiem.toml --base "$mb" "$@"
        fi
    fi
}

cmd_semver() {
    need cargo-semver-checks
    meta_py has-lib --locked || skip "no library target"
    meta_py has-stable-version --locked || skip "every crate is pre-1.0 (in development), so breaking changes are allowed"
    local base
    base="$(git tag --merged HEAD --no-contains HEAD --list 'v*' --sort=-v:refname | head -1)"
    [ -n "$base" ] || skip "no earlier v* release tag to compare against"
    cargo semver-checks --workspace --all-features --baseline-rev "$base"
}

cmd_wasm() {
    has_wasm || skip "no wasm32 target declared in rust-toolchain.toml or Cargo.toml"
    rustup target add wasm32-unknown-unknown
    cargo check --workspace --target wasm32-unknown-unknown --all-features --locked
}
