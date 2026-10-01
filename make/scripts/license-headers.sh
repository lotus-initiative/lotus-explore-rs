#!/usr/bin/env bash
# Every `.rs` file must carry both AGPL-3.0-only headers on lines 1 and 2.
#
# `target/`, `.opencode/` and `graphify-out/` are build output, vendored config
# and generated graph data respectively -- none of them is source.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
missing=0
while IFS= read -r f; do
  if ! head -1 "$f" | grep -qxF '// SPDX-License-Identifier: AGPL-3.0-only' \
     || ! head -2 "$f" | tail -1 | grep -qxF '// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project'; then
    echo "missing or misplaced SPDX header: $f" >&2
    missing=1
  fi
done < <(find . \( -name target -o -name .opencode -o -name graphify-out -o -name .git \) -prune -o -name '*.rs' -print)
exit $missing
