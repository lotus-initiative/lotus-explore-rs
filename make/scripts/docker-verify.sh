#!/usr/bin/env bash
# Confirm the image is what it claims to be.
#
# Three things, each of which has been true of an image that looked fine:
#
#   * it runs as root, because the base image's default user was not overridden
#   * its HEALTHCHECK is present but cannot run, because it invokes a shell that
#     distroless does not have -- so it reports a container that is serving as
#     one that is broken
#   * it starts, and the port maps, but the server is not answering
#
# The healthcheck is exercised by waiting for Docker's own verdict rather than
# by running the probe once, so a HEALTHCHECK that is syntactically present and
# fails at runtime shows up here.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

tag=lotus-explore-rs:local
name=lotus-verify-$$
fail=0

# ── non-root ─────────────────────────────────────────────────────────────────
user=$(docker image inspect "$tag" --format '{{.Config.User}}' 2>/dev/null || echo "")
if [ -n "$user" ] && [ "$user" != "0" ] && [ "$user" != "root" ] && [ "$user" != "0:0" ]; then
  echo "ok    image user is '$user'"
else
  echo "FAIL  image user is '${user:-<unset>}'; it must be non-root" >&2
  fail=1
fi

# ── a healthcheck that is present ────────────────────────────────────────────
hc=$(docker image inspect "$tag" --format '{{json .Config.Healthcheck}}' 2>/dev/null || echo "null")
if [ "$hc" = "null" ] || [ -z "$hc" ]; then
  echo "FAIL  the image has no HEALTHCHECK" >&2
  fail=1
else
  echo "ok    HEALTHCHECK $hc"
fi

# ── it starts, and stays started ─────────────────────────────────────────────
docker rm -f "$name" >/dev/null 2>&1 || true
docker run -d --name "$name" --read-only --tmpfs /tmp \
  --cap-drop ALL --security-opt no-new-privileges \
  -p 127.0.0.1:0:8787 "$tag" >/dev/null
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT

# Ask Docker for the verdict, not for a single probe. The image's own start
# period is 10s, so this polls for as long as that plus its retries.
echo "waiting for Docker to report the container healthy..."
healthy=no
for _ in $(seq 1 40); do
  state=$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$name" 2>/dev/null || echo unknown)
  case "$state" in
    healthy) healthy=yes; break ;;
    unhealthy) echo "FAIL  Docker reports the container unhealthy" >&2
              docker inspect --format '{{range .State.Health.Log}}{{.Output}}{{end}}' "$name" 2>/dev/null | tail -5 >&2
              fail=1; break ;;
  esac
  sleep 2
done
if [ "$healthy" = yes ]; then
  echo "ok    the container passed its healthcheck"
else
  [ "$fail" = 0 ] && { echo "FAIL  the container never became healthy" >&2; fail=1; }
fi

# ── and the server really answers ────────────────────────────────────────────
port=$(docker port "$name" 8787/tcp 2>/dev/null | head -1 | sed 's/.*://')
if [ -n "$port" ]; then
  body=$(curl -s --max-time 10 "http://127.0.0.1:$port/health" || true)
  case "$body" in
    *'"status"'*) echo "ok    GET /health -> $body" ;;
    *) echo "FAIL  GET /health returned '$body'" >&2; fail=1 ;;
  esac
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "http://127.0.0.1:$port/" || true)
  echo "ok    GET / -> $code (the web bundle, or 404 with no PUBLIC_DIR)"
else
  echo "FAIL  could not determine the mapped port" >&2
  fail=1
fi

exit $fail
