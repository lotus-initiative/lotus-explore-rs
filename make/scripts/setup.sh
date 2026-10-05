#!/usr/bin/env bash
# Install every tool the other tasks call, at a pinned version.
#
# `cargo binstall` is used where available because compiling these from source is
# minutes each and they are prebuilt binaries. It is not a requirement:
# `cargo install --locked --version` produces the same binary, just slower, so a
# contributor without binstall gets the same set, later.
#
# Versions are pinned rather than floating. A tool that changes its output between
# releases changes what a task prints, and a task whose output is asserted on --
# `opt-levels` and the JUnit report both are -- is a task whose failure is a
# version bump rather than a defect.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

# The toolchain itself, from rust-toolchain.toml. rustup reads that file, so this
# is one line and it cannot disagree with the pin.
echo "==> toolchain"
rustup show active-toolchain || rustup toolchain install

# The Dioxus version the `dioxus` dependency is pinned to. Read rather than
# repeated, because the two drifting is the failure this whole task exists to
# stop: `dx` building an app against a different Dioxus is a build that succeeds
# and an app that does not run.
# The `.*` is deliberately non-greedy in the version itself and the closing quote
# is explicit: a greedy match runs past the version to the *last* quote on the
# line, which is the one closing `features = [...]`, so the result was
# `0.7.10", default-features = false, features = ["asset", ...`. It failed as an
# argument to `cargo install --version` and named dioxus-cli as the cause, which
# is the worst kind of wrong.
DX_VERSION=$(sed -n 's/^dioxus = { version = "=\([^"]*\)".*/\1/p' Cargo.toml | head -1)
: "${DX_VERSION:?could not read the dioxus version from Cargo.toml}"

have_binstall=false
if command -v cargo-binstall >/dev/null 2>&1; then
  have_binstall=true
fi

install_tool() {
  local crate="$1" version="$2"
  local spec="$crate@$version"
  echo "==> $spec"
  if $have_binstall; then
    # `--no-confirm` because this is a script, and a prompt nobody is there to answer
    # is a hang rather than a question.
    if cargo binstall --no-confirm "$spec"; then
      return
    fi
    echo "    binstall could not provide $spec; falling back to cargo install" >&2
  fi
  cargo install --locked "$crate" --version "$version"
}

# Test runner. Without this every other task that mentions tests is wrong.
install_tool cargo-nextest 0.9.146
# This task runner. Installing it is a chicken-and-egg for `./mk setup`, which is
# why the first thing to try is a plain `cargo install`.
install_tool cargo-make 0.37.24

# Lints and manifest hygiene.
install_tool cargo-hack 0.6.45       # feature combinations
install_tool cargo-edit 0.13.13     # `cargo upgrade`, `cargo add`, `cargo rm`
install_tool cargo-msrv 0.19.3       # the real MSRV, rather than the asserted one
install_tool typos-cli 1.50.3       # spelling
install_tool dejadoc 0.4.0           # duplicate doctests and functions

# Supply chain.
install_tool cargo-deny 0.20.2
install_tool cargo-audit 0.22.2
install_tool cargo-machete 0.9.2
install_tool cargo-outdated 0.19.0
# `cargo geiger` for the transitive unsafe audit, `cargo udeps` for unused
# dependencies the manifest scan cannot see. Both are on the weekly schedule
# rather than the gate, so a slow or advisory-heavy one cannot turn the gate red.
install_tool cargo-geiger 0.13.0
install_tool cargo-udeps 0.1.61

# The nextest integration is what makes the coverage report describe the same run
# the suite performs.
install_tool cargo-llvm-cov 0.9.1
install_tool cargo-mutants 27.1.0

# Build time and the wasm bundle. `twiggy` is superseded by `wasm-opt` for size
# work and `cargo bloat` for the native binaries, and neither is in the gate.
install_tool cargo-bloat 0.12.1

# tombi is the TOML formatter and linter, and the tool the git hooks already run
# via `tombi-pre-commit`. It is NOT on crates.io: `tombi-cli` there is a 0.0.1
# placeholder with no binary, so `cargo install tombi-cli` produces nothing. The
# real binary is a GitHub release, and the version is read from `prek.toml` so
# the hook and this cannot be pinned to different releases -- the same reasoning
# as `dx` below.
TOMBI_VERSION=$(sed -n '/tombi-pre-commit/{n;s/rev = "v\([^"]*\)"/\1/p;}' prek.toml | head -1)
: "${TOMBI_VERSION:?could not read the tombi version from prek.toml}"
echo "==> tombi ${TOMBI_VERSION}"
if ! command -v tombi >/dev/null 2>&1 || [ "$(tombi --version 2>/dev/null)" != "tombi ${TOMBI_VERSION} "* ]; then
  case "$(uname -s)/$(uname -m)" in
    Darwin/arm64) tombi_asset="tombi-cli-${TOMBI_VERSION}-aarch64-apple-darwin.tar.gz" ;;
    Darwin/x86_64) tombi_asset="tombi-cli-${TOMBI_VERSION}-x86_64-apple-darwin.tar.gz" ;;
    Linux/aarch64|Linux/arm64) tombi_asset="tombi-cli-${TOMBI_VERSION}-aarch64-unknown-linux-musl.tar.gz" ;;
    Linux/x86_64) tombi_asset="tombi-cli-${TOMBI_VERSION}-x86_64-unknown-linux-musl.tar.gz" ;;
    *) echo "no tombi release asset for $(uname -s)/$(uname -m)" >&2; exit 1 ;;
  esac
  tombi_dir=$(mktemp -d)
  curl -fsSL --retry 3 --retry-all-errors \
    "https://github.com/tombi-toml/tombi/releases/download/v${TOMBI_VERSION}/${tombi_asset}" \
    | tar xz -C "$tombi_dir"
  command install -m 755 "$tombi_dir"/*/tombi "${CARGO_HOME:-$HOME/.cargo}/bin/tombi"
  rm -rf "$tombi_dir"
else
  echo "    tombi is already ${TOMBI_VERSION}"
fi

# `dioxus-cli` installs the `dx` binary. `dx` itself is not on crates.io, so
# `dx@$DX_VERSION` does not resolve; this is the same version, same binary.
echo "==> dioxus-cli@$DX_VERSION"
if ! command -v dx >/dev/null 2>&1 || [ "$(dx --version 2>/dev/null || true)" != "$DX_VERSION" ]; then
  cargo install dioxus-cli --version "$DX_VERSION" --locked
else
  echo "    dx is already $DX_VERSION"
fi

echo
echo "Installed. The tasks are:"
echo
./mk --list-all-steps
