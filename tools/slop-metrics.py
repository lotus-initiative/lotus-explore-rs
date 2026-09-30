#!/usr/bin/env python3
"""Report the file-size and hygiene metrics the definition-of-done asks for.

Crates the numbers by brace balance rather than by regex, so a function that
contains nested blocks is measured to its real end.  Test code -- anything
inside a `#[cfg(test)]` module, a `tests/` directory, or a `#[test]` function --
is tracked separately, because the rules for it differ from the rules for
shipped code.

    python3 tools/slop-metrics.py            # full report
    python3 tools/slop-metrics.py --long 5  # longest N functions
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = ["crates", "apps"]

FN_START = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?"
    r"(?:default\s+|const\s+|async\s+|unsafe\s+|extern\s+\"[^\"]*\"\s+)*"
    r"fn\s+(\w+)"
)
ITEM_START = re.compile(r"^\s*#\[(?!test\b|cfg_attr)[A-Za-z_]+")
TEST_MARKER = re.compile(r"^\s*(#\[test\]|#\[tokio::test|#\[rstest|#\[case\b|#\[ignore\b)")
CFG_TEST = re.compile(r"^\s*#\[cfg\(test\)\]\s*$")


def source_files() -> list[Path]:
    files: list[Path] = []
    for group in TARGET:
        root = ROOT / group
        if not root.is_dir():
            continue
        files.extend(
            path
            for path in root.rglob("*.rs")
            if "target" not in path.relative_to(ROOT).parts
            and not path.name.endswith(".orig")
        )
    return sorted(files)


def strip_noncode(line: str) -> str:
    """Remove string literals and line comments so brace counting is honest."""
    out: list[str] = []
    index = 0
    while index < len(line):
        char = line[index]
        if char == "/" and index + 1 < len(line) and line[index + 1] == "/":
            break
        if char in "\"'":
            quote = char
            index += 1
            while index < len(line):
                if line[index] == "\\":
                    index += 2
                    continue
                if line[index] == quote:
                    break
                index += 1
        elif char == "r" and index + 1 < len(line) and line[index + 1] in "#":
            close = line[index :].find('"')
            if close == -1:
                break
            term = line[index : index + close + 1] + '"'
            end = line.find(term, index + close + 1)
            index = len(line) if end == -1 else end + len(term)
        else:
            out.append(char)
        index += 1
    return "".join(out)


def classify(text: str) -> list[bool]:
    """Return, per line, whether it is test-only code.

    A line is test code when it sits inside a `#[cfg(test)]` module or inside a
    function carrying `#[test]`.  A stack records what each open brace belongs
    to, so a test module nested inside an `impl` block is still recognised.
    Attribute lines are attributed to whatever block encloses them.
    """
    lines = text.splitlines()
    is_test = [False] * len(lines)
    stack: list[bool] = []
    pending_test = False

    for number, raw in enumerate(lines):
        code = strip_noncode(raw)
        trimmed = raw.strip()
        if not stack or not stack[-1]:
            if TEST_MARKER.match(raw) or CFG_TEST.match(raw):
                pending_test = True
        if trimmed and not trimmed.startswith("//"):
            enclosing = bool(stack) and stack[-1]
            is_test[number] = enclosing or pending_test

        for char in code:
            if char == "{":
                stack.append(pending_test)
                pending_test = False
            elif char == "}" and stack:
                stack.pop()
    return is_test


def function_bodies(text: str) -> list[dict]:
    """Measure functions by scanning for `fn` and matching braces forward."""
    lines = text.splitlines()
    is_test = classify(text)
    found: list[dict] = []
    index = 0
    while index < len(lines):
        match = FN_START.match(lines[index])
        if not match:
            index += 1
            continue
        # Walk forward to the opening brace of the body.
        cursor = index
        body_start = None
        while cursor < len(lines):
            code = strip_noncode(lines[cursor])
            brace = code.find("{")
            if brace != -1:
                body_start = cursor
                break
            if ";" in code and cursor > index:
                break
            cursor += 1
            if cursor - index > 60:
                break
        if body_start is None:
            index += 1
            continue
        depth = 0
        end = body_start
        started = False
        for cursor in range(body_start, len(lines)):
            code = strip_noncode(lines[cursor])
            for char in code:
                if char == "{":
                    depth += 1
                    started = True
                elif char == "}":
                    depth -= 1
            if started and depth <= 0:
                end = cursor
                break
        test_here = any(is_test[index : end + 1])
        found.append(
            {
                "name": match.group(1),
                "start": index + 1,
                "end": end + 1,
                "lines": end - index + 1,
                "test": test_here,
            }
        )
        index = end + 1
    return found


def hygiene(text: str) -> dict[str, list[tuple[int, str]]]:
    is_test = classify(text)
    hits: dict[str, list[tuple[int, str]]] = defaultdict(list)
    for number, raw in enumerate(text.splitlines()):
        code = strip_noncode(raw)
        kind = "test" if is_test[number] else "code"
        if ".unwrap()" in code:
            hits[f"unwrap.{kind}"].append((number + 1, raw.strip()))
        if ".expect(" in code:
            hits[f"expect.{kind}"].append((number + 1, raw.strip()))
        if re.search(r"#\[allow\(", code):
            hits[f"allow.{kind}"].append((number + 1, raw.strip()))
        if re.search(r"#\[(clippy::)?(expect_used|unwrap_used)\b", code):
            hits[f"expect_used.{kind}"].append((number + 1, raw.strip()))
        if re.match(r"^\s*//\s*(let |fn |if |for |match |return |use |impl |struct )", raw):
            hits["commented_code.code"].append((number + 1, raw.strip()))
    return hits


def crate_of(path: Path) -> str:
    parts = path.relative_to(ROOT).parts
    return "/".join(parts[:2]) if len(parts) > 2 else str(path.parent)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--long", type=int, default=12)
    parser.add_argument("--json", type=Path)
    parser.add_argument("--rsx", action="store_true", help="report long rsx blocks")
    args = parser.parse_args()

    files = source_files()
    per_crate: dict[str, dict[str, int]] = defaultdict(
        lambda: {"files": 0, "loc": 0, "files_over_250": 0, "fn_over_40": 0}
    )
    all_functions: list[dict] = []
    sizes: list[tuple[int, str]] = []
    totals: dict[str, int] = defaultdict(int)

    for path in files:
        text = path.read_text(encoding="utf-8")
        rel = str(path.relative_to(ROOT))
        lines = text.splitlines()
        crate = crate_of(path)
        per_crate[crate]["files"] += 1
        per_crate[crate]["loc"] += len(lines)
        sizes.append((len(lines), rel))
        if len(lines) > 250:
            per_crate[crate]["files_over_250"] += 1
        found = function_bodies(text)
        for entry in found:
            entry["file"] = rel
            all_functions.append(entry)
            if not entry["test"] and entry["lines"] > 40:
                per_crate[crate]["fn_over_40"] += 1
        for key, values in hygiene(text).items():
            totals[key] += len(values)

    print("=" * 72)
    print("PER-CRATE")
    print("=" * 72)
    print(f"{'crate':<34}{'files':>7}{'LOC':>8}{'>250':>7}{'fn>40':>8}")
    total_files = total_loc = 0
    for crate in sorted(per_crate):
        row = per_crate[crate]
        total_files += row["files"]
        total_loc += row["loc"]
        print(
            f"{crate:<34}{row['files']:>7}{row['loc']:>8}"
            f"{row['files_over_250']:>7}{row['fn_over_40']:>8}"
        )
    print(f"{'TOTAL':<34}{total_files:>7}{total_loc:>8}")

    print()
    print("=" * 72)
    print("FILES OVER 250 LINES")
    print("=" * 72)
    over = sorted((n, f) for n, f in sizes if n > 250 and "tests" not in f)
    for count, rel in reversed(over):
        print(f"{count:>6}  {rel}")
    print(f"  ({len(over)} production files over 250 lines)")

    print()
    print("=" * 72)
    print(f"LONGEST FUNCTIONS (non-test, top {args.long})")
    print("=" * 72)
    code_fns = sorted(
        (f for f in all_functions if not f["test"]), key=lambda f: -f["lines"]
    )
    for entry in code_fns[: args.long]:
        print(
            f"{entry['lines']:>6}  {entry['file']}:{entry['start']}  fn {entry['name']}"
        )
    test_fns = sorted((f for f in all_functions if f["test"]), key=lambda f: -f["lines"])
    print()
    print(f"LONGEST TEST FUNCTIONS (top {min(5, len(test_fns))})")
    for entry in test_fns[:5]:
        print(f"{entry['lines']:>6}  {entry['file']}:{entry['start']}  fn {entry['name']}")

    over_fns = [f for f in code_fns if f["lines"] > 40]
    print()
    print(f"{len(over_fns)} non-test functions over 40 lines")

    print()
    print("=" * 72)
    print("HYGIENE COUNTS")
    print("=" * 72)
    for key in sorted(totals):
        print(f"{key:<28}{totals[key]:>6}")

    if args.json:
        args.json.write_text(
            json.dumps(
                {
                    "per_crate": dict(per_crate),
                    "files": total_files,
                    "loc": total_loc,
                    "functions": len(all_functions),
                    "hygiene": dict(totals),
                    "longest": code_fns[:40],
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        print(f"\nwrote {args.json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
