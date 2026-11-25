# shellcheck shell=bash
# Layer 1: Pinned tool resolution, MK_TOOLS_DIR PATH isolation, $CARGO_HOME/bin
# cache seeding, unified tarball/binstall installers, and setup/tools commands.

# A pinned tool is `crate@version`, or `crate@version:binary` when the binary has
# another name (`dioxus-cli@0.7.10:dx`). Tools are named `crate` or `crate:binary`.
spec_crate() { echo "${1%%@*}"; }
spec_ver() {
    local rest="${1#*@}"
    echo "${rest%%:*}"
}
spec_tool() {
    local rest="${1#*@}"
    case "$rest" in *:*) echo "$(spec_crate "$1"):${rest#*:}" ;; *) spec_crate "$1" ;; esac
}

tool_bin() {
    case "$1" in
        *:*) echo "${1#*:}" ;;
        typos-cli) echo typos ;;
        echoes | refquiem) echo "cargo-$1" ;;
        *) echo "$1" ;;
    esac
}

tool_version() {
    local bin
    bin="$(tool_bin "$1")"
    case "$bin" in
        cargo-*) cargo "${bin#cargo-}" --version ;;
        *) "$bin" --version ;;
    esac
}

# Exact pinned version installed? (probe only, never exits)
has_version() {
    local out
    out="$(tool_version "$1" 2>/dev/null)" || return 1
    printf '%s\n' "$out" | grep -Eq "(^|[^0-9.])${2//./\\.}([^0-9.-]|$)"
}

require() {
    has_version "$1" "$2" ||
        die "$1 $2 (mk.versions) is required, found '$(tool_version "$1" 2>&1 | head -1)'; run ./mk setup"
}

require_spec() { require "$(spec_tool "$1")" "$(spec_ver "$1")"; }

# shellcheck disable=SC2154  # reads mk.versions/mk.local tool arrays
spec_of() {
    local spec
    for spec in "${CARGO_TOOLS_CI[@]}" "${CARGO_TOOLS_NIGHTLY[@]}" ${LOCAL_CARGO_TOOLS[@]+"${LOCAL_CARGO_TOOLS[@]}"}; do
        if [ "$(spec_crate "$spec")" = "$1" ]; then
            echo "$spec"
            return
        fi
    done
    die "$1 is not pinned in mk.versions"
}

need() { require_spec "$(spec_of "$1")"; }

# shellcheck disable=SC2154  # MK_TOOLS_DIR initialized by mk
tool_root() { echo "$MK_TOOLS_DIR/$1/$2"; }

# shellcheck disable=SC2154  # reads mk.versions/mk.local tool pins and _MK_BASE_PATH
refresh_tool_path() {
    local prefix="" dir spec
    for dir in \
        "$(tool_root tombi "$TOMBI_VERSION")/bin" \
        "$(tool_root shellcheck "$SHELLCHECK_VERSION")/bin"; do
        if [ -d "$dir" ]; then prefix="${prefix:+$prefix:}$dir"; fi
    done
    for spec in \
        ${CARGO_TOOLS_CI[@]+"${CARGO_TOOLS_CI[@]}"} \
        ${CARGO_TOOLS_NIGHTLY[@]+"${CARGO_TOOLS_NIGHTLY[@]}"} \
        ${LOCAL_CARGO_TOOLS[@]+"${LOCAL_CARGO_TOOLS[@]}"}; do
        dir="$(tool_root "$(spec_crate "$spec")" "$(spec_ver "$spec")")/bin"
        if [ -d "$dir" ]; then prefix="${prefix:+$prefix:}$dir"; fi
    done
    export PATH="${prefix:+$prefix:}${CARGO_HOME:-$HOME/.cargo}/bin:$_MK_BASE_PATH"
    hash -r 2>/dev/null || true
}

is_cached() {
    local dir bin
    dir="$(tool_root "$2" "$3")/bin"
    bin="$(tool_bin "$1")"
    [ -x "$dir/$bin" ] && PATH="$dir:$PATH" has_version "$1" "$3"
}

# Reuse an already-installed regular executable from $CARGO_HOME/bin when it matches
# the exact pinned version, copying it into MK_TOOLS_DIR so later installs cannot clobber it.
# shellcheck disable=SC2154  # _MK_BASE_PATH exported by mk
seed_from_cargo_bin() {
    local tool="$1" name="$2" ver="$3" bin cargo_bin src dir
    bin="$(tool_bin "$tool")"
    cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
    src="$cargo_bin/$bin"
    dir="$(tool_root "$name" "$ver")/bin"
    [ -f "$src" ] && [ ! -L "$src" ] && [ -x "$src" ] || return 1
    PATH="$cargo_bin:$_MK_BASE_PATH" has_version "$tool" "$ver" || return 1
    mkdir -p "$dir"
    install -m 0755 "$src" "$dir/$bin"
    is_cached "$tool" "$name" "$ver"
}

install_github_tarball() {
    local tool="$1" ver="$2" url="$3" inner_rel="$4" tmp bin_dir
    if is_cached "$tool" "$tool" "$ver"; then return; fi
    if seed_from_cargo_bin "$tool" "$tool" "$ver"; then
        refresh_tool_path
        return
    fi
    tmp="$(mktemp -d)"
    bin_dir="$(tool_root "$tool" "$ver")/bin"
    curl -fsSL "$url" | tar xz -C "$tmp"
    mkdir -p "$bin_dir"
    install -m 0755 "$tmp/$inner_rel" "$bin_dir/$(tool_bin "$tool")"
    rm -rf "$tmp"
    refresh_tool_path
    require "$tool" "$ver"
}

# shellcheck disable=SC2154  # TOMBI_VERSION from mk.versions
install_tombi() {
    local host target url
    host="$(rustc -vV | sed -n 's/^host: //p')"
    target="${host/-linux-gnu/-linux-musl}"
    url="https://github.com/tombi-toml/tombi/releases/download/v${TOMBI_VERSION}/tombi-cli-${TOMBI_VERSION}-${target}.tar.gz"
    install_github_tarball tombi "$TOMBI_VERSION" "$url" "tombi-cli-${TOMBI_VERSION}-${target}/tombi"
}

# shellcheck disable=SC2154  # SHELLCHECK_VERSION from mk.versions
install_shellcheck() {
    local os arch url
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"
    arch="${arch/arm64/aarch64}"
    url="https://github.com/koalaman/shellcheck/releases/download/v${SHELLCHECK_VERSION}/shellcheck-v${SHELLCHECK_VERSION}.${os}.${arch}.tar.gz"
    install_github_tarball shellcheck "$SHELLCHECK_VERSION" "$url" "shellcheck-v${SHELLCHECK_VERSION}/shellcheck"
}

install_cargo_tools() {
    [ $# -gt 0 ] || return 0
    local spec crate ver tool root
    for spec in "$@"; do
        crate="$(spec_crate "$spec")"
        ver="$(spec_ver "$spec")"
        tool="$(spec_tool "$spec")"
        if ! is_cached "$tool" "$crate" "$ver" && ! seed_from_cargo_bin "$tool" "$crate" "$ver"; then
            command -v cargo-binstall >/dev/null || die "cargo-binstall is required: https://github.com/cargo-bins/cargo-binstall"
            root="$(tool_root "$crate" "$ver")"
            mkdir -p "$root"
            # Prebuilt when the crate publishes binaries; otherwise build it.
            if ! cargo binstall --no-confirm --locked --force --root "$root" --no-track "$crate@$ver" &&
                ! cargo install --locked --force --root "$root" --no-track "$crate" --version "$ver"; then
                rm -rf "$root"
                die "failed to install $crate@$ver"
            fi
        fi
        refresh_tool_path
        require_spec "$spec"
    done
}

# shellcheck disable=SC2154  # LOCAL_SYSTEM_PACKAGES from mk.local
install_system_packages() {
    [ "${#LOCAL_SYSTEM_PACKAGES[@]}" -gt 0 ] || return 0
    if { [ "${CI:-}" = true ] || [ "${CI:-}" = 1 ]; } && command -v apt-get >/dev/null; then
        sudo apt-get update -qq
        sudo apt-get install -y --no-install-recommends "${LOCAL_SYSTEM_PACKAGES[@]}"
    else
        echo "note: this repository needs the system packages: ${LOCAL_SYSTEM_PACKAGES[*]}"
    fi
}

# shellcheck disable=SC2154  # reads mk.versions/mk.local setup pins
cmd_setup() {
    local tier="${1:-all}" t v
    if [ "$tier" = nightly ] && idle_scheduled_nightly; then
        echo "note: no commits in the last 24h on scheduled run; installing only audit and outdated"
        install_cargo_tools "$(spec_of cargo-audit)" "$(spec_of cargo-outdated)"
        return 0
    fi
    install_system_packages
    install_cargo_tools ${LOCAL_CARGO_TOOLS[@]+"${LOCAL_CARGO_TOOLS[@]}"}
    for t in ${LOCAL_SETUP[@]+"${LOCAL_SETUP[@]}"}; do cargo_make "$t"; done
    rustup toolchain install # from rust-toolchain.toml
    v="$(msrv)"
    if [ -n "$v" ]; then
        rustup toolchain install "$v" --profile minimal
    fi
    install_tombi
    install_shellcheck
    install_cargo_tools "${CARGO_TOOLS_CI[@]}"
    if [ "$tier" = all ] || [ "$tier" = nightly ]; then
        rustup toolchain install "$RUST_NIGHTLY" --profile minimal --component miri,rust-src
        rustup component add llvm-tools
        install_cargo_tools "${CARGO_TOOLS_NIGHTLY[@]}"
    fi
}

# shellcheck disable=SC2154  # reads mk.versions/mk.local tool pins
cmd_tools() {
    local spec
    require tombi "$TOMBI_VERSION"
    require shellcheck "$SHELLCHECK_VERSION"
    for spec in "${CARGO_TOOLS_CI[@]}" ${LOCAL_CARGO_TOOLS[@]+"${LOCAL_CARGO_TOOLS[@]}"}; do
        require_spec "$spec"
    done
}
