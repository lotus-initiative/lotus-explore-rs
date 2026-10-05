#!/usr/bin/env bash
# Report the machine's capacity before a mutation run, and refuse on the two
# resources that have actually run out.
#
# WHAT IS KNOWN
# Two runs of `./mk mutants` wedged this 16 GB / 8-core machine, the second at
# `--jobs 2`. So neither disk nor job count explains it: both crashes had
# 27-29 GB free, and a per-crate build peaks at 114-216 MB measured, so even
# eight concurrent builds are about 1.7 GB. Something else is responsible.
#
# The thing that is genuinely anomalous is the machine's own state. Measured on
# 2026-10-05: 16 GB RAM with 5.7 of 7 GB swap already in use and 1.45 GB free,
# before any mutation run starts. Whatever holds that is the thing to find, and
# a guard that checks only disk cannot see it.
#
# So this checks the two resources that can actually be exhausted -- free disk
# and free swap -- and prints the rest. It refuses rather than predicts.
#
# Until the cause is known the reliable form is one package at a time with
# MUTANTS_JOBS=1. MUTANTS_MIN_FREE_GB=0 and MUTANTS_MIN_SWAP_FREE_GB=0 skip the
# checks.

set -euo pipefail

# Floors, not predictions: a floor against running out, not a threshold derived
# from a crash. 20 GB because a full sweep writes a few GB; 2 GB of swap because
# the observed state had 1.45 GB free.
readonly MIN_FREE_GB="${MUTANTS_MIN_FREE_GB:-20}"
readonly MIN_SWAP_FREE_GB="${MUTANTS_MIN_SWAP_FREE_GB:-2}"
readonly ROOT="${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

# Bytes free on the volume holding the repo. `df -k` is POSIX and gives a whole
# number of 1K blocks; the awk converts to GB so the arithmetic below stays in
# integers.
free_kb="$(df -k "$ROOT" | awk 'NR == 2 { print $4 }')"
free_gb=$((free_kb / 1024 / 1024))

target_gb=0
if [[ -d "$ROOT/target" ]]; then
  target_kb="$(du -sk "$ROOT/target" 2>/dev/null | awk '{ print $1 }')"
  target_gb=$((target_kb / 1024 / 1024))
fi

ram_gb="$(($(sysctl -n hw.memsize) / 1024 / 1024 / 1024))"
cores="$(sysctl -n hw.ncpu 2>/dev/null || echo '?')"

# Free swap as an integer number of MB, or -1 where it cannot be read. macOS
# reports it in `vm.swapusage`; Linux needs /proc/meminfo, where "SwapTotal" and
# "SwapFree" are in kB. Anything else reports -1 and the check is skipped
# rather than guessed at.
swap_free_mb=-1
if [[ "$(uname -s)" == "Darwin" ]]; then
  swap_free_mb="$(sysctl -n vm.swapusage 2>/dev/null |
    sed -n 's/.*free = \([0-9.]*\)\([MG]\).*/\1/p' |
    awk '{ if ($2 == "G") printf "%d", $1 * 1024; else printf "%d", $1 }')"
  [[ -n "$swap_free_mb" ]] || swap_free_mb=-1
elif [[ -r /proc/meminfo ]]; then
  swap_free_mb="$(awk '/^SwapFree:/ { print int($2 / 1024) }' /proc/meminfo)"
fi
[[ "$swap_free_mb" =~ ^-?[0-9]+$ ]] || swap_free_mb=-1
swap_free_gb=$((swap_free_mb >= 0 ? swap_free_mb / 1024 : 0))
swap_line="$(sysctl -n vm.swapusage 2>/dev/null || echo 'unavailable')"

echo "mutants preflight"
echo "  disk free : ${free_gb} GB of $(df -h "$ROOT" | awk 'NR == 2 { print $2 }') on $(df -h "$ROOT" | awk 'NR == 2 { print $1 }')"
echo "  target/   : ${target_gb} GB (regenerable)"
echo "  memory    : ${ram_gb} GB RAM, ${cores} cores"
echo "  swap      : ${swap_line}"
echo "  jobs      : ${MUTANTS_JOBS:-4}"
if ((swap_free_mb < 0)); then
  echo "  swap free : unreadable here; the swap check is skipped"
fi
echo
echo "  Note: two runs crashed this machine, the second at --jobs 2, both with"
echo "  27-29 GB of disk free. The cause is not identified, so this cannot promise"
echo "  the run finishes. One package at a time with MUTANTS_JOBS=1 is reliable."

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

if ((swap_free_mb >= 0 && swap_free_gb < MIN_SWAP_FREE_GB)); then
  cat >&2 <<EOF

refusing to start: only ${swap_free_mb} MB of swap is free (the floor is ${MIN_SWAP_FREE_GB} GB).

The two recorded crashes were not disk and not --jobs, and this machine was found
sitting on 1.45 GB of free swap with nothing of ours running. A build burst on
top of that is the likeliest way to wedge it.

Check what is holding the memory before retrying:

  memory_pressure                 # macOS: swap use and page-ins by process
  sysctl vm.swapusage

Free it, or lower the floor for a run you are watching: MUTANTS_MIN_SWAP_FREE_GB=0 ./mk mutants
EOF
  exit 1
fi
