#!/usr/bin/env bash
# The task runner, with the one flag this repository needs.
#
# Run `./mk <task>` — or `make <task>`, which is what `mk` is aliased to in most
# shells' muscle memory.
#
# Why this exists rather than being a `cargo make` invocation in the docs:
# cargo-make 0.37 enables "workspace support", which re-runs the requested task
# once per workspace member with the working directory set to that member. In a
# workspace of eight crates that is eight executions of every task, so
#
#   cargo make tombi-check      7.3s   (8 executions of a 0.6s task)
#   cargo make --no-workspace   0.6s
#
# Cargo tasks hid most of it, because the second `cargo check` of an unchanged
# workspace is a cache hit. Everything that is not cargo -- tombi, typos, docker,
# the asset fetch, `setup` -- paid the full 8x, and `cargo make ci` ran its whole
# fan-out eight times.
#
# There is no configuration key and no environment variable for this in 0.37;
# `CARGO_MAKE_CRATE_IS_WORKSPACE` is read from `cargo metadata` before the
# makefile's own `[env]` is applied, so it cannot be set from there. The flag is
# the supported mechanism, and this is the one place it is written.
#
# It forwards its arguments, so `./mk ci -- --profile ci` and `./mk ci` both work
# and nothing is hidden behind it.
set -euo pipefail
exec cargo make --no-workspace "$@"
