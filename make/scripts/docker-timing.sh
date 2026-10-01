#!/usr/bin/env bash
# Cold and warm build times, and the resulting image size.
#
# The cold run drops both the layer cache and the BuildKit cache mounts.
# `--no-cache` alone invalidates the instructions but leaves the mounts, so the
# number it produces is neither a cold build nor a warm one -- it is a third
# thing that is easy to mistake for either.
#
# `docker buildx prune` needs the daemon and takes a while, so this is a task
# you run on purpose rather than something the gate does.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

tag=lotus-explore-rs:local
ref=$(git rev-parse --short HEAD)

# Bytes to MiB. `docker image inspect --format {{.Size}}` reports bytes and
# nothing else, so this is the only place the conversion happens.
human() { printf '%.1f MiB' "$(echo "$1" | awk '{print $1/1048576}')"; }

echo "==> cold: no layer cache, no cache mounts"
docker buildx prune --force >/dev/null 2>&1 || true
start=$(date +%s)
docker buildx build --target runtime --no-cache --load \
  --build-arg "VCS_REF=$ref" -t "$tag" . >/dev/null
cold=$(( $(date +%s) - start ))
size_cold=$(docker image inspect "$tag" --format '{{.Size}}' 2>/dev/null || echo 0)
printf 'cold  %5ds  %s\n' "$cold" "$(human "$size_cold")"

echo
echo "==> warm: same source, caches intact"
start=$(date +%s)
docker buildx build --target runtime --load \
  --build-arg "VCS_REF=$ref" -t "$tag" . >/dev/null
warm=$(( $(date +%s) - start ))
size_warm=$(docker image inspect "$tag" --format '{{.Size}}' 2>/dev/null || echo 0)
printf 'warm  %5ds  %s\n' "$warm" "$(human "$size_warm")"

echo
echo "==> other targets (warm)"
for t in static cli; do
  docker buildx build --target "$t" --load --build-arg "VCS_REF=$ref" \
    -t "lotus-explore-rs-timing:$t" . >/dev/null
  s=$(docker image inspect "lotus-explore-rs-timing:$t" --format '{{.Size}}' 2>/dev/null || echo 0)
  printf '%-8s %s\n' "$t" "$(human "$s")"
  docker rmi "lotus-explore-rs-timing:$t" >/dev/null 2>&1 || true
done

echo
printf 'speedup: %sx\n' "$(( cold / (warm > 0 ? warm : 1) ))"
