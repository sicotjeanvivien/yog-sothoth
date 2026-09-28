#!/usr/bin/env python3
"""A module whose content is re-exported is private, unless it says why not.

The rule (crates/README.md, *Conventions*): when a file declares `mod x;`
and re-exports from it (`pub use x::Item;`, `pub use self::{x::A, y::B};`,
any `pub(...)`), the module is
private. The re-export is then the only path to each item, and the file
is an organisational detail. Two exceptions keep it visible, each stated
by a comment on the line above the declaration (attributes and doc
comments may sit in between):

    // Visible: callers need items that are not re-exported here.
    // Visible: target of a doc link (<file>).

This script fails on:
  - a visible re-exported module without that comment;
  - a `// Visible:` comment above a module that is private, or that is no
    longer re-exported: a stale justification reads as an intention.

It fails loudly, too, when the inventory is suspiciously small: a broken
glob or a moved tree would otherwise find nothing to check and pass.

Limits (the same heuristic as the measurement the rule was chosen from):
  - a re-export written in another file than the declaration escapes it;
  - so does an inline `mod x { ... }`;
  - test code is skipped: `tests/` directories, `*_tests.rs`, and any
    `mod` under a `cfg(test)` attribute. `yog-wasm` (deferred scaffold) is
    skipped too.

Usage: python3 crates/check-module-visibility.py [<crates dir>]
"""

import os
import re
import sys
from pathlib import Path

MARKER = "// Visible:"
# The workspace declares ~340 re-exported modules (September 2026). Far
# below that, the scan is broken, not the code clean.
MIN_INVENTORY = 100
SKIPPED_CRATES = {"wasm"}

MOD_DECL = re.compile(r"^[ \t]*(?P<vis>pub(?:\([^)]*\))?\s+)?mod\s+(?P<name>\w+)\s*;")
ATTRIBUTE = re.compile(r"^[ \t]*#\[")
DOC = re.compile(r"^[ \t]*///")


PUB_USE = re.compile(r"^[ \t]*pub(?:\([^)]*\))?\s+use\s+(?P<tree>[^;]*);", re.M | re.S)


def top_level_modules(tree: str):
    """First path segment of each branch of a `use` tree.

    `self::x::A` and `x::A` give `x`; `{x::A, y::{B, C}}` gives `x` and `y`.
    """
    tree = re.sub(r"\s+", "", tree)
    if tree.startswith("self::"):
        tree = tree[len("self::"):]
    if not tree.startswith("{"):
        return {tree.split("::", 1)[0]} if "::" in tree else set()
    names, depth, branch = set(), 0, ""
    for char in tree[1:-1] + ",":
        if char == "," and depth == 0:
            names |= top_level_modules(branch)
            branch = ""
            continue
        depth += char == "{"
        depth -= char == "}"
        branch += char
    return names


def reexported_modules(source: str):
    names = set()
    for statement in PUB_USE.finditer(source):
        names |= top_level_modules(statement.group("tree"))
    return names


def is_test_file(path: Path) -> bool:
    return "tests" in path.parts or path.name.endswith("_tests.rs")


def scan(crates_dir: Path):
    """Yields (path, line number, name, visible, justified) per re-exported module."""
    for path in sorted(crates_dir.glob("*/src/**/*.rs")):
        relative = path.relative_to(crates_dir)
        if relative.parts[0] in SKIPPED_CRATES or is_test_file(relative):
            continue
        source = path.read_text(encoding="utf-8")
        lines = source.split("\n")
        reexported = reexported_modules(source)
        for index, line in enumerate(lines):
            declaration = MOD_DECL.match(line)
            if not declaration:
                continue
            # Walk up over attributes and doc comments to the line that may
            # hold the marker.
            above = index - 1
            attributes = []
            while above >= 0 and (ATTRIBUTE.match(lines[above]) or DOC.match(lines[above])):
                if ATTRIBUTE.match(lines[above]):
                    attributes.append(lines[above])
                above -= 1
            if any("cfg(test)" in a for a in attributes):
                continue
            justified = above >= 0 and lines[above].lstrip().startswith(MARKER)
            name = declaration.group("name")
            visible = declaration.group("vis") is not None
            if name in reexported:
                yield path, index + 1, name, visible, justified
            elif justified:
                # A marker above a module nothing re-exports: stale.
                yield path, index + 1, name, None, justified


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    crates_dir = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else repo_root / "crates"

    inventory = 0
    exceptions = 0
    faults = []
    for path, line, name, visible, justified in scan(crates_dir):
        if visible is not None:
            inventory += 1
        if visible and justified:
            exceptions += 1
        elif visible:
            faults.append((path, line, f"`mod {name}` is re-exported here but visible: "
                           f"make it private, or state why above it with `{MARKER} …`"))
        elif justified:
            what = "private" if visible is False else "not re-exported here"
            faults.append((path, line, f"`{MARKER}` above `mod {name}`, which is {what}: "
                           f"the justification is stale, remove it"))

    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    report = []
    if inventory < MIN_INVENTORY:
        message = (f"only {inventory} re-exported module(s) found under {crates_dir} "
                   f"(expected at least {MIN_INVENTORY}): the scan is broken, nothing was checked")
        print(f"::error::{message}")
        report.append(f"- {message}")
        faults.append(None)

    for fault in faults:
        if fault is None:
            continue
        path, line, message = fault
        shown = os.path.relpath(path, repo_root)
        print(f"::error file={shown},line={line}::{message}")
        report.append(f"- `{shown}:{line}` — {message}")

    print(f"{inventory} re-exported modules, {exceptions} visible with a stated reason, "
          f"{len(report)} fault(s)")

    if summary and report:
        with open(summary, "a", encoding="utf-8") as out:
            out.write("### Module visibility\n\n" + "\n".join(report) + "\n")

    return 1 if faults else 0


if __name__ == "__main__":
    sys.exit(main())
