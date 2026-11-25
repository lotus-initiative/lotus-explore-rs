#!/usr/bin/env python3
"""Pure cargo-metadata queries, workspace graph scoping, and mutant shard planning."""

from __future__ import annotations

from collections import deque
from collections.abc import Callable, Iterable
import json
import os
import subprocess
import sys
from typing import Any

LIB_TARGET_KINDS = frozenset({"lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"})

ROOT_TRIGGER_FILES = frozenset({
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/mutants.toml",
    "mk", "mk.versions", "mk.local",
})

PACKAGE_TRIGGER_DIRS = frozenset({"src", "tests", "benches", "examples"})


def workspace_member_packages(meta: dict[str, Any]) -> list[dict[str, Any]]:
    """Return workspace member package objects from parsed cargo metadata."""
    members = set(meta.get("workspace_members", []))
    packages = meta.get("packages", [])
    return [pkg for pkg in packages if not members or pkg["id"] in members]


def package_names(meta: dict[str, Any]) -> list[str]:
    """Return sorted workspace member crate names."""
    return sorted(pkg["name"] for pkg in workspace_member_packages(meta))


def manifest_paths(meta: dict[str, Any]) -> list[str]:
    """Return sorted Cargo.toml manifest paths for workspace members."""
    return sorted(pkg["manifest_path"] for pkg in workspace_member_packages(meta))


def has_lib_target(meta: dict[str, Any]) -> bool:
    """Return True if any workspace member defines a library or proc-macro target."""
    return any(
        LIB_TARGET_KINDS.intersection(target.get("kind", []))
        for pkg in workspace_member_packages(meta)
        for target in pkg.get("targets", [])
    )


def first_msrv(meta: dict[str, Any]) -> str:
    """Return the first declared rust_version among workspace members, or empty string."""
    for pkg in workspace_member_packages(meta):
        if pkg.get("rust_version"):
            return str(pkg["rust_version"])
    return ""


def first_license(meta: dict[str, Any]) -> str:
    """Return the lexicographically first non-empty license across workspace members."""
    licenses = sorted(
        {str(pkg["license"]) for pkg in workspace_member_packages(meta) if pkg.get("license")}
    )
    return licenses[0] if licenses else ""


def has_stable_version(meta: dict[str, Any]) -> bool:
    """Return True if any workspace member version is 1.0.0 or higher."""
    return any(
        not str(pkg.get("version", "")).startswith("0.")
        for pkg in workspace_member_packages(meta)
    )


def has_publishable_package(meta: dict[str, Any]) -> bool:
    """Return True if any workspace member does not set publish = false (publish is None)."""
    return any(pkg.get("publish") is None for pkg in workspace_member_packages(meta))


def _pkg_dir(pkg: dict[str, Any]) -> str:
    return os.path.dirname(os.path.realpath(pkg["manifest_path"]))


def _package_dirs(pkgs: list[dict[str, Any]]) -> list[tuple[int, str, str, str]]:
    """Return (dir_len, real_dir, id, name) tuples sorted deepest-first."""
    entries = [
        (len(pdir), pdir, pkg["id"], pkg["name"])
        for pkg in pkgs
        for pdir in (_pkg_dir(pkg),)
    ]
    entries.sort(reverse=True)
    return entries


def _match_package_dir(abs_file: str, dirs: list[tuple[int, str, str, str]]) -> tuple[str, str, str] | None:
    """Find the deepest package directory containing abs_file."""
    for _, pdir, pid, name in dirs:
        if abs_file == pdir or abs_file.startswith(pdir + os.sep):
            return pdir, pid, name
    return None

def _clean_files(files: Iterable[str]) -> list[str]:
    return sorted({line.strip() for line in files if line.strip()})

def changed_rust_packages(meta: dict[str, Any], git_root: str, files: Iterable[str]) -> list[str]:
    """Return sorted workspace package names owning any changed file."""
    cleaned = _clean_files(files)
    if not cleaned:
        return []
    git_root_real = os.path.realpath(git_root)
    dirs = _package_dirs(workspace_member_packages(meta))
    found: set[str] = set()
    for rel in cleaned:
        abs_file = os.path.realpath(os.path.join(git_root_real, rel))
        matched = _match_package_dir(abs_file, dirs)
        if matched is not None:
            found.add(matched[2])
    return sorted(found)


def _reverse_dep_graph(pkgs: list[dict[str, Any]]) -> dict[str, set[str]]:
    """Build target_pkg_id -> {dependent_pkg_id} within the workspace."""
    id_to_name = {pkg["id"]: pkg["name"] for pkg in pkgs}
    dir_to_ids: dict[str, list[str]] = {}
    name_to_ids: dict[str, list[str]] = {}
    for pkg in pkgs:
        pdir = _pkg_dir(pkg)
        dir_to_ids.setdefault(pdir, []).append(pkg["id"])
        name_to_ids.setdefault(pkg["name"], []).append(pkg["id"])

    rev_deps: dict[str, set[str]] = {pid: set() for pid in id_to_name}
    for pkg in pkgs:
        caller = pkg["id"]
        for dep in pkg.get("dependencies", []):
            dep_path = dep.get("path")
            if dep_path:
                candidates = dir_to_ids.get(os.path.realpath(dep_path), [])
            elif dep.get("source") is None:
                candidates = name_to_ids.get(dep["name"], [])
            else:
                candidates = []
            for tid in candidates:
                if id_to_name[tid] == dep["name"]:
                    rev_deps[tid].add(caller)
    return rev_deps


def _is_root_trigger(rel_root: str) -> bool:
    return rel_root in ROOT_TRIGGER_FILES or rel_root.startswith(".mk/")


def _is_package_trigger(rel: str, inside: str) -> bool:
    top = inside.split(os.sep)[0]
    return (
        rel.endswith(".rs")
        or os.path.basename(rel) == "Cargo.toml"
        or top in PACKAGE_TRIGGER_DIRS
        or inside == "README.md"
    )


def affected_packages(meta: dict[str, Any], git_root: str, root_dir: str, files: Iterable[str]) -> list[str]:
    """Return sorted changed packages plus transitive workspace reverse-dependents."""
    cleaned = _clean_files(files)
    if not cleaned:
        return []
    pkgs = workspace_member_packages(meta)
    id_to_name = {pkg["id"]: pkg["name"] for pkg in pkgs}
    git_root_real = os.path.realpath(git_root)
    root_real = os.path.realpath(root_dir)
    for rel in cleaned:
        abs_file = os.path.realpath(os.path.join(git_root_real, rel))
        rel_root = os.path.relpath(abs_file, root_real).replace(os.sep, "/")
        if _is_root_trigger(rel_root):
            return sorted(id_to_name.values())

    dirs = _package_dirs(pkgs)
    seeds: set[str] = set()
    for rel in cleaned:
        abs_file = os.path.realpath(os.path.join(git_root_real, rel))
        matched = _match_package_dir(abs_file, dirs)
        if matched is not None:
            pdir, pid, _ = matched
            if _is_package_trigger(rel, os.path.relpath(abs_file, pdir)):
                seeds.add(pid)

    rev_deps = _reverse_dep_graph(pkgs)
    seen = set(seeds)
    queue = deque(seeds)
    while queue:
        cur = queue.popleft()
        for nxt in rev_deps.get(cur, ()):
            if nxt not in seen:
                seen.add(nxt)
                queue.append(nxt)
    return sorted({id_to_name[pid] for pid in seen})


def compute_shards(mutants: int, target: int, max_shards: int) -> int:
    """Compute bounded shard count for a mutant count, target per shard, and cap."""
    if mutants < 0:
        raise ValueError(f"mutants must be non-negative, got {mutants}")
    if target <= 0:
        raise ValueError(f"target must be positive, got {target}")
    if max_shards <= 0:
        raise ValueError(f"max_shards must be positive, got {max_shards}")
    if mutants == 0:
        return 0
    return min(max((mutants + target - 1) // target, 1), max_shards)


def format_plan_json(mode: str, mutants: int, shards: int, packages: list[str]) -> str:
    """Render compact single-line JSON for mutants-plan."""
    return json.dumps(
        {"mode": mode, "mutants": mutants, "shards": shards, "packages": packages},
        separators=(",", ":"),
    )


def _load_cargo_metadata(locked: bool) -> dict[str, Any]:
    cmd = ["cargo", "metadata", "--no-deps", "--format-version", "1"]
    if locked:
        cmd.append("--locked")
    return json.loads(subprocess.check_output(cmd, text=True))


def _print_lines(items: list[str]) -> int:
    for item in items:
        print(item)
    return 0


def _print_optional(value: str) -> int:
    if value:
        print(value)
    return 0


_META_HANDLERS: dict[str, Callable[[dict[str, Any]], int]] = {
    "packages": lambda m: _print_lines(package_names(m)),
    "manifests": lambda m: _print_lines(manifest_paths(m)),
    "has-lib": lambda m: 0 if has_lib_target(m) else 1,
    "msrv": lambda m: _print_optional(first_msrv(m)),
    "license": lambda m: _print_optional(first_license(m)),
    "has-stable-version": lambda m: 0 if has_stable_version(m) else 1,
    "has-publishable": lambda m: 0 if has_publishable_package(m) else 1,
}


def main(argv: list[str]) -> int:
    """CLI entrypoint dispatching metadata, graph, and shard-plan queries."""
    if not argv:
        print("usage: meta.py <subcommand> [args...]", file=sys.stderr)
        return 1
    cmd = argv[0]
    locked = "--locked" in argv[1:]
    rest = [arg for arg in argv[1:] if arg != "--locked"]

    try:
        if cmd == "plan-json" and len(rest) >= 3:
            print(format_plan_json(rest[0], int(rest[1]), int(rest[2]), rest[3:]))
            return 0
        if cmd == "shards" and len(rest) == 3:
            print(compute_shards(int(rest[0]), int(rest[1]), int(rest[2])))
            return 0
    except ValueError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    meta = _load_cargo_metadata(locked)
    if cmd in _META_HANDLERS and not rest:
        return _META_HANDLERS[cmd](meta)
    if cmd == "changed" and len(rest) == 1:
        return _print_lines(changed_rust_packages(meta, rest[0], sys.stdin))
    if cmd == "affected" and len(rest) == 2:
        return _print_lines(affected_packages(meta, rest[0], rest[1], sys.stdin))

    print(f"unknown or malformed subcommand: {cmd}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
