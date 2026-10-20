#!/usr/bin/env python3
# Written by Kone Tshivhinda & Claude
"""Check that relative links in Markdown files point to files or directories that exist.

External (http/https/mailto) links and pure #anchors are skipped: they change and would make CI
flaky. Links inside fenced code blocks are ignored. Exit code 1 if anything is broken.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent
SKIP_DIRS = {".git", "node_modules", "target", ".venv", "dist", ".github"}
LINK = re.compile(r"(?<!\!)\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)|!\[[^\]]*\]\(([^)\s]+)\)")


def markdown_files() -> list[Path]:
    out = []
    for path in ROOT.rglob("*.md"):
        if any(part in SKIP_DIRS for part in path.relative_to(ROOT).parts):
            continue
        out.append(path)
    return sorted(out)


def links(text: str) -> list[str]:
    found: list[str] = []
    in_fence = False
    for line in text.splitlines():
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        line = re.sub(r"`[^`]*`", "", line)  # ignore inline code
        for m in LINK.finditer(line):
            found.append(m.group(1) or m.group(2))
    return found


def main() -> int:
    broken: list[str] = []
    checked = 0
    for md in markdown_files():
        for target in links(md.read_text(encoding="utf-8")):
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            checked += 1
            path_part = unquote(target.split("#", 1)[0])
            if not path_part:
                continue
            resolved = (md.parent / path_part).resolve()
            if not resolved.exists():
                broken.append(f"{md.relative_to(ROOT)}: {target}")
    if broken:
        print(f"{len(broken)} broken relative link(s):")
        print("\n".join(f"  {b}" for b in broken))
        return 1
    print(f"ok: {checked} relative links checked in {len(markdown_files())} Markdown files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
