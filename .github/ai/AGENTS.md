## graphify

This project has a knowledge graph at `graphify-out/` (8.7 MB `graph.json`, plus
`GRAPH_REPORT.md` and dated snapshots). It is generated output and git-ignored, so
it is a local convenience and not a source of truth about the code.

**Check that graphify is on `PATH` before relying on it.** It is installed per
machine, not per repository, and has been absent on machines where every commit
reported `could not locate a Python with graphify installed`. When it is missing,
use `grep` and the compiler instead; do not spend the task trying to install it.
Everything below applies only when the command resolves.

When the user types `/graphify`, use the graphify skill or instructions before
doing anything else.

Rules:

- For codebase questions, first run `graphify query "<question>"`. Use
  `graphify path "<A>" "<B>"` for relationships and
  `graphify explain "<concept>"` for focused concepts. These return a scoped
  subgraph, usually much smaller than `GRAPH_REPORT.md` or raw grep output.
- Dirty `graphify-out/` files are expected after hooks or incremental updates,
  and are not a reason to skip graphify. Only skip it when the task is about
  stale or incorrect graph output, or the user says not to use it.
- If `graphify-out/wiki/index.md` exists, use it for broad navigation instead of
  raw source browsing.
- Read `graphify-out/GRAPH_REPORT.md` only for broad architecture review, or
  when `query`/`path`/`explain` do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current
  (AST-only, no API cost).
