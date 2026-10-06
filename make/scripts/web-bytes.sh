#!/usr/bin/env bash
# One number per transfer encoding, so a profile experiment is comparable with
# the one before it instead of eyeballed. `raw` is what the linker emitted,
# `br` is the sibling dx pre-compresses, `gzip -9` is the fallback for hosts
# without brotli. Only the bundle we actually ship is measured: no source maps.
#
# This script does not modify the build. An earlier version pruned "superseded"
# assets with `grep -qF "$base" index.html || rm -f`, on the theory that anything
# the document does not name is dead weight. That was wrong twice over:
#
#   - `dx` names the module in the *glue JS*, not in `index.html`, so a bare
#     `dx build` produced an `index.html` that mentioned no asset at all and the
#     prune deleted the module. `./mk web-bytes` was destroying the largest
#     artefact on the page on every run.
#   - it then measured what was left and printed a total anyway: 111 KiB, for an
#     application whose module alone is ~1.6 MiB. Off by an order of magnitude,
#     and formatted exactly like a real measurement.
#
# So nothing is deleted here. A measurement tool that mutates its input can only
# ever report on itself. Where the old script deleted, this one refuses, and says
# what it would have deleted.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
out="target/dx/lotus-explore-rs/release/web/public"
if [ ! -f "$out/index.html" ]; then
  echo "no build at $out -- run './mk web-build' first" >&2
  exit 1
fi

# `stat` is spelled differently on macOS and Linux. `-f` first because GNU `stat`
# accepts `-f` too, with unrelated meaning, so the order matters. stderr is
# suppressed on both: on the platform where a form is the wrong flag it prints a
# usage error, and it was that fallback usage error -- not anything about the
# measurement -- that surfaced as `stat: illegal option -- c`.
size()  { stat -f '%z' "$1" 2>/dev/null || stat -c '%s' "$1" 2>/dev/null; }
mtime() { stat -f '%m' "$1" 2>/dev/null || stat -c '%Y' "$1" 2>/dev/null; }
kb()    { echo $(( $1 / 1024 )); }

shopt -s nullglob

# The module, in either place `dx` leaves it: the hashed release copy under
# `assets/`, and the unhashed one under `wasm/` that the dev server serves.
modules=( "$out"/assets/lotus-explore-rs_bg-*.wasm )
if [ "${#modules[@]}" -eq 0 ]; then
  modules=( "$out"/wasm/lotus-explore-rs_bg*.wasm )
fi

if [ "${#modules[@]}" -eq 0 ]; then
  cat >&2 <<'EOF'
no app module in the build

Refusing to print a total without it. The module is ~1.6 MiB raw and every other
asset together is ~111 KiB, so a total over the remainder reports the application
as a fifteenth of its size -- and looks authoritative while doing it.

The usual cause is build.rs's clean_dx_output(), which deletes
target/dx/<app>/<profile>/web/public on *any* wasm compile of the app, so a
`./mk ci` between the build and the measurement removes it. See the "measurement
trap" section of docs/PERFORMANCE.md.

  ./mk web-build && ./mk web-bytes
EOF
  exit 1
fi

if [ "${#modules[@]}" -gt 1 ]; then
  printf 'two modules in the build, so the transfer size is ambiguous:\n' >&2
  printf '  %s\n' "${modules[@]}" >&2
  cat >&2 <<'EOF'

`dx` leaves a superseded module behind when the hash changes. The old script
deleted the loser; this one will not guess which one is current, because that is
the judgement the measurement is supposed to be making. Rebuild to get one.

  ./mk web-build
EOF
  exit 1
fi

# A measurement of a bundle older than HEAD is a measurement of a different
# programme, and nothing in the output used to say so. Warn rather than fail:
# re-measuring a deliberately old bundle is legitimate, it just has to be a
# decision rather than an accident.
bundle_epoch=$(mtime "$out/index.html")
head_epoch=$(git log -1 --format=%ct)
if [ -n "${bundle_epoch:-}" ] && [ -n "${head_epoch:-}" ] && [ "$bundle_epoch" -lt "$head_epoch" ]; then
  printf 'warning: bundle built %s, HEAD committed %s\n' \
    "$(date -r "$bundle_epoch" '+%Y-%m-%d %H:%M:%S')" \
    "$(date -r "$head_epoch" '+%Y-%m-%d %H:%M:%S')" >&2
  printf 'warning: these are not the bytes HEAD would ship\n\n' >&2
fi

printf '%-38s %10s %10s %10s\n' asset raw_KiB br_KiB gzip_KiB
tot_raw=0; tot_br=0; tot_gz=0
files=( "${modules[@]}" "$out"/assets/*.js "$out"/assets/*.css "$out/index.html" )
for file in "${files[@]}"; do
  [ -f "$file" ] || continue
  raw=$(size "$file")
  if [ -f "$file.br" ]; then br=$(size "$file.br"); else br=$raw; fi
  gz=$(gzip -c9 "$file" | wc -c | tr -d ' ')
  printf '%-38s %10s %10s %10s\n' "$(basename "$file")" "$(kb $raw)" "$(kb $br)" "$(kb $gz)"
  tot_raw=$(( tot_raw + raw )); tot_br=$(( tot_br + br )); tot_gz=$(( tot_gz + gz ))
done
printf '%-38s %10s %10s %10s\n' TOTAL "$(kb $tot_raw)" "$(kb $tot_br)" "$(kb $tot_gz)"

# Named rather than deleted, so a stale file can be seen instead of silently
# removed -- and so this table can be trusted to have measured one of each.
#
# A total that quietly includes both a hashed and an unhashed copy of the same
# logical asset is the same class of error as one that quietly omits the module,
# so it is called out here rather than left to be noticed: the TOTAL above sums
# every row printed, and where `dx` has left a superseded copy alongside the
# current one the list below is the reason two rows share a name.
skipped=()
for asset in "$out"/assets/*.js "$out"/assets/*.css; do
  [ -f "$asset" ] || continue
  base=$(basename "$asset")
  grep -qF "$base" "$out/index.html" || skipped+=("$base")
done
if [ "${#skipped[@]}" -gt 0 ]; then
  printf '\nnot named by index.html -- measured above, left in place, and counted\n' >&2
  printf 'in the TOTAL. If one of these is a superseded copy, its row is double-counted:\n' >&2
  printf '  %s\n' "${skipped[@]}" >&2
fi

printf '\nwasm raw bytes: %s\n' "$(size "${modules[0]}")"
