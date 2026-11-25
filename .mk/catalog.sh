# shellcheck shell=bash
# Layer 0: Declarative gate registry, tier definitions, alias normalization,
# descriptions, failure hints, and CLI help/listing.

# shellcheck disable=SC2034  # consumed by .mk/core.sh, .mk/tier.sh
SKIP_RC=78

# shellcheck disable=SC2034  # consumed by .mk/gates.sh
CONVENTIONAL_COMMIT_RE='^(build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)(\([a-z0-9._/-]+\))?!?: [^ ]'

tier_gates() {
    case "$1" in
        ci)
            printf '%s\n' tools actions shell commits ratchet fmt tombi typos lint lints-inherit \
                test doc deny audit machete msrv echoes refquiem semver wasm mutants-diff
            ;;
        ci-core)
            printf '%s\n' tools actions shell commits ratchet fmt tombi typos lint lints-inherit \
                test doc deny audit machete msrv echoes refquiem semver wasm
            ;;
        ci-fast)
            printf '%s\n' tools actions shell commits ratchet fmt tombi typos lint lints-inherit \
                test-fast doc deny machete msrv echoes refquiem semver wasm
            ;;
        ci-nightly) printf '%s\n' mutants coverage powerset miri outdated ;;
        ci-nightly-checks) printf '%s\n' coverage powerset miri outdated ;;
        *) die "unknown tier '$1'" ;;
    esac
}

normalize_cmd_alias() {
    case "$1" in
        toml) echo tombi ;;
        unused-deps) echo machete ;;
        duplication | dup) echo echoes ;;
        stale-refs) echo refquiem ;;
        feature-powerset | hack) echo powerset ;;
        cov) echo coverage ;;
        *) echo "$1" ;;
    esac
}

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
        coverage) echo "line coverage floor check" ;;
        powerset) echo "feature powerset compilation check" ;;
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
        coverage) echo "add tests to restore line coverage, or run './mk coverage-baseline' when raising the floor" ;;
        msrv) echo "avoid APIs newer than rust-version in Cargo.toml, or bump rust-version intentionally" ;;
    esac
}

print_help() {
    cat <<'EOF'
Usage: ./mk <task> [args...]

Tiers:
  check             Fast inner loop (<10s): fmt + tombi + typos + lint (supports -p <crate>)
  check-changed     Inner loop scoped to changed packages + reverse-dependents
  fix               Auto-fix formatting, TOML, typos, and clippy
  test-fast         Unit & integration tests (nextest / cargo test, no doctests; supports -p <crate>)
  test-changed      Unit & integration tests scoped to changed packages + reverse-dependents
  watch             Continuous feedback via bacon / cargo-watch
  ci-fast           Local pre-push gate (skips mutants-diff & network audit; change-aware)
  ci-core           All blocking PR gates except mutants-diff (change-aware)
  ci                Full blocking PR gate (all 21 blocking gates, including mutants-diff; change-aware)
  ci-nightly-checks Nightly non-mutation sweeps (coverage, powerset, miri, outdated)
  ci-nightly        Scheduled deep sweep (mutants, coverage, powerset, miri, outdated)
  ci-release        Release gate (./mk ci + cargo publish --dry-run)

Mutation helpers:
  mutants-diff      Mutation testing on branch Rust diff (supports --iterate, -f, -p)
  mutants-affected  Mutation testing on changed packages + reverse-dependents
  mutants-plan      Emit JSON shard plan [--mode diff|affected|full] [--base <ref>]

Gate aliases (alongside canonical tool names):
  toml              -> tombi     (TOML formatting & schema lint)
  unused-deps       -> machete   (unused dependency detection)
  duplication, dup  -> echoes    (code duplication & dependency API reuse)
  stale-refs        -> refquiem  (stale mentions of deleted or changed symbols)
  feature-powerset, hack -> powerset (cargo-hack --feature-powerset check)
  cov               -> coverage  (line coverage floor check)

Setup & baselines:
  setup [ci|nightly|all]  Install pinned toolchains and tools from mk.versions
  coverage-baseline       Record today's line coverage floor in mk.local
  echoes-baseline         Record today's echoes ratchet counts in mk.local
  --list                  List all available tasks
EOF
}

print_task_list() {
    {
        printf '%s\n' help setup check check-changed fix test-fast test-changed watch bench udeps cov hack \
            toml unused-deps duplication dup stale-refs feature-powerset \
            mutants-affected mutants-plan \
            ci ci-core ci-fast ci-nightly ci-nightly-checks ci-release coverage-baseline echoes-baseline selftest
        tier_gates ci
        tier_gates ci-nightly
    } | sort -u
    local t
    for t in "$@"; do
        echo "local:$t"
    done
    if [ -f Makefile.toml ]; then echo "(any other name runs that cargo-make task)"; fi
}
