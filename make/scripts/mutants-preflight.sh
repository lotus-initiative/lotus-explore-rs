#!/usr/bin/env bash
# Report the machine's capacity before a mutation run, and refuse one known hazard.
#
# WHAT IS KNOWN
# Two runs of `./mk mutants` wedged a 16 GB / 8-core laptop with a 1 GB swap ceiling.
# The first was `--jobs 8`. The second was `--jobs 2` and also died, which is the part
# that matters: if two concurrent builds can do it, "too many parallel jobs" is not the
# explanation, and lowering `--jobs` from 8 to 4 is a guess rather than a fix.
#
# WHAT IS NOT KNOWN
# The cause. A 94%-full volume is a real problem and worth clearing, but both crashes
# happened with 27-29 GB still free, so disk alone does not account for them, and
# `target/` is large mostly because of ordinary development rather than from mutants
# (a full 1,545-mutant sweep added roughly 4 GB, about 2.6 MB a mutant). The honest
# position is that the constraint has not been identified.
#
# So this script does two modest, true things rather than promising safety it cannot
# deliver:
#
#   1. Prints free disk, `target/` size, RAM and the swap ceiling, because "it crashed"
#      is much easier to diagnose against those numbers than against nothing.
#   2. Refuses to start when the volume is genuinely short of room, which is a
#      predictable failure even if it is not the one that happened.
#
# It cannot prevent a memory-pressure crash. Until the cause is known, the reliable
# way to run this is one package at a time with MUTANTS_JOBS=1, and to watch the
# numbers printed here rather than the run itself.
#
# Override with MUTANTS_MIN_FREE_GB=0 to skip the check.

set -euo pipefail

# 20 GB: a full sweep writes a few GB, so this is headroom rather than a prediction.
# It is a floor against running out of room, not a threshold derived from a crash.
readonly MIN_FREE_GB="${MUTANTS_MIN_FREE_GB:-20}"
readonly ROOT="${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

# Bytes free on the volume holding the repo. `df -k` is POSIX and gives a whole number
# of 1K blocks; the awk converts to GB so the arithmetic below stays in integers.
free_kb="$(df -k "$ROOT" | awk 'NR == 2 { print $4 }')"
free_gb=$((free_kb / 1024 / 1024))

target_gb=0
if [[ -d "$ROOT/target" ]]; then
  target_kb="$(du -sk "$ROOT/target" 2>/dev/null | awk '{ print $1 }')"
  target_gb=$((target_kb / 1024 / 1024))
fi

ram_gb="$(($(sysctl -n hw.memsize) / 1024 / 1024 / 1024))"
swap_line="$(sysctl -n vm.swapusage 2>/dev/null || echo 'unavailable')"
cores="$(sysctl -n hw.ncpu 2>/dev/null || echo '?')"

echo "mutants preflight"
echo "  disk free : ${free_gb} GB of $(df -h "$ROOT" | awk 'NR == 2 { print $2 }') on $(df -h "$ROOT" | awk 'NR == 2 { print $1 }')"
echo "  target/   : ${target_gb} GB (regenerable)"
echo "  memory    : ${ram_gb} GB RAM, ${cores} cores"
echo "  swap      : ${swap_line}"
echo "  jobs      : ${MUTANTS_JOBS:-4}"
echo
echo "  Note: two runs crashed this machine, the second at --jobs 2. The cause is not"
echo "  identified, so this check cannot promise the run will finish. One package at a"
echo "  time with MUTANTS_JOBS=1 is the reliable form."

if ((free_gb < MIN_FREE_GB)); then
  cat >&2 <<EOF

refusing to start: a mutation run needs room to rebuild the workspace per mutant, and
only ${free_gb} GB is free (the floor is ${MIN_FREE_GB} GB).

Reclaim it with either of:

  cargo clean -p lotus-explore-rs -p lotus-model -p lotus-search   # targeted, ~9 GB
  cargo clean                                                   # everything, ~${target_gb} GB

Both are safe -- everything under target/ is rebuilt on demand -- but a full clean
means the next build starts cold, which on this workspace is not quick.

To proceed anyway: MUTANTS_MIN_FREE_GB=0 ./mk mutants
EOF
  exit 1
fi
