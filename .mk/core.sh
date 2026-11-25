# shellcheck shell=bash
# Layer 0: Foundational exit helpers, git diff queries, config upserts,
# docs-only diff classifier, and .mk/meta.py runner.

die() {
    echo "ERROR: $*" >&2
    exit 1
}

# Not applicable here: exit 0 when run directly, SKIP_RC when run by a tier.
# shellcheck disable=SC2154  # SKIP_RC from .mk/catalog.sh
skip() {
    echo "[skip] ${cmd:-mk}: $*"
    if [ "${MK_NESTED:-0}" = 1 ]; then exit "$SKIP_RC"; fi
    exit 0
}

require_python3() {
    command -v python3 >/dev/null || die "python3 is required"
}

meta_py() {
    require_python3
    python3 -B "${ROOT_DIR:-.}/.mk/meta.py" "$@"
}

has_pkg_flag() {
    local arg
    for arg in "$@"; do
        case "$arg" in
            -p | --package | --package=* | --workspace) return 0 ;;
        esac
    done
    return 1
}

append_line_with_newline() {
    local file="$1" line="$2"
    if [ -s "$file" ] && [ -n "$(tail -c1 "$file")" ]; then
        printf '\n' >>"$file"
    fi
    printf '%s\n' "$line" >>"$file"
}

upsert_kv_line() {
    local file="$1" key="$2" raw_rhs="$3" header="${4:-}"
    if grep -q "^${key}=" "$file" 2>/dev/null; then
        sed -i.bak "s|^${key}=.*|${key}=${raw_rhs}|" "$file" && rm -f "${file}.bak"
        return
    fi
    if [ -n "$header" ]; then
        append_line_with_newline "$file" ""
        append_line_with_newline "$file" "$header"
    fi
    append_line_with_newline "$file" "${key}=${raw_rhs}"
}

# Declared outside comments (the template's toolchain file mentions it in one).
has_wasm() {
    grep -Eq '^[^#]*wasm32' rust-toolchain.toml Cargo.toml 2>/dev/null
}

msrv() {
    local v
    v="$(sed -n 's/^rust-version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -1)"
    if [ -z "$v" ] && command -v python3 >/dev/null; then
        v="$(meta_py msrv 2>/dev/null || true)"
    fi
    printf '%s\n' "$v"
}

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

git_changed_files() {
    local base="$1" mb
    shift
    mb="$(git merge-base "$base" HEAD 2>/dev/null)" || return 1
    git diff --no-ext-diff --no-color --name-only "$mb" "$@"
    git ls-files --others --exclude-standard "$@"
}

write_rust_diff() {
    local base="$1" dest="$2" mb u
    mb="$(git merge-base "$base" HEAD)"
    git diff --no-ext-diff --no-color "$mb" -- '*.rs' >"$dest"
    while IFS= read -r u; do
        [ -n "$u" ] || continue
        git diff --no-index --no-ext-diff --no-color -- /dev/null "$u" >>"$dest" || true
    done < <(git ls-files --others --exclude-standard -- '*.rs')
}

changed_rust_packages() {
    local base="$1" git_root
    git_root="$(git rev-parse --show-toplevel)"
    git_changed_files "$base" -- '*.rs' | meta_py changed "$git_root"
}

affected_packages() {
    local base="$1" git_root
    git_root="$(git rev-parse --show-toplevel)"
    git_changed_files "$base" | meta_py affected "$git_root" "${ROOT_DIR:-.}"
}

# True when a non-empty diff against base_ref touches only documentation, workflow,
# editor, or non-Rust hygiene config files (never *.rs, Cargo.*, crate READMEs, or scripts).
docs_or_meta_only_diff() {
    local base changed f
    base="$(base_ref)" || return 1
    changed="$(git_changed_files "$base" | sort -u)" || return 1
    [ -n "$changed" ] || return 1
    while IFS= read -r f; do
        case "$f" in
            docs/*)
                if git grep -qF "$f" -- '*.rs' 2>/dev/null; then return 1; fi
                ;;
            .github/* | skills/* | .agents/* | .vscode/* | \
            .typos.toml | .tombi.toml | .refquiem.toml | prek.toml | bacon.toml | renovate.json | .editorconfig | .gitignore)
                ;;
            */*)
                return 1
                ;;
            *.md)
                if [ "$f" = README.md ] && {
                    grep -q '^\[package\]' Cargo.toml 2>/dev/null ||
                        git grep -qE 'include_(str|bytes)!\([^)]*\.\./\.\./README\.md' -- '*.rs' 2>/dev/null
                }; then
                    return 1
                fi
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
        fmt | lint | lints-inherit | test | test-fast | doc | deny | machete | msrv | echoes | semver | wasm | mutants-diff | mutants-affected)
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
