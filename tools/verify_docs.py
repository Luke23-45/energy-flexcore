#!/usr/bin/env python3
"""Structural and citation checks for the FlexCore report.

Runs with the Python standard library only. Exits non-zero if any check fails.

    python tools/verify_docs.py [path/to/report.md]
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DIR = ROOT / "docs" / "plan"

HEADING_NUMBERED = re.compile(r"^(#{1,3})\s+(\d+(?:\.\d+)?)\.?\s+(.*\S)\s*$")
HEADING_ANY = re.compile(r"^(#{1,6})\s+(.*\S)\s*$")
SECTION_REF = re.compile(r"§(\d+(?:\.\d+)?)")
REF_DEF = re.compile(r"^- \[R(\d+)\]")
REF_CITE = re.compile(r"\[R(\d+)\]")
TOC_ITEM = re.compile(r"^\d+\.\s+(.*\S)\s*$")

failures: list[str] = []
notes: list[str] = []


def fail(message: str) -> None:
    failures.append(message)


def find_report(argv: list[str]) -> Path:
    if len(argv) > 1:
        path = Path(argv[1])
        return path if path.is_absolute() else ROOT / path
    candidates = sorted(DEFAULT_DIR.glob("*.md"))
    if not candidates:
        raise SystemExit(f"No report found in {DEFAULT_DIR}")
    if len(candidates) > 1:
        notes.append(
            "multiple reports found, checking "
            + ", ".join(p.name for p in candidates)
        )
    return candidates[-1]


def check_headings(lines: list[str]) -> tuple[dict[str, int], list[tuple[str, str]]]:
    """Return numbered headings and ordered top-level sections."""
    numbered: dict[str, int] = {}
    top: list[tuple[str, str]] = []
    for lineno, line in enumerate(lines, 1):
        match = HEADING_NUMBERED.match(line)
        if not match:
            continue
        level, number, title = match.groups()
        if number in numbered:
            fail(f"duplicate heading number {number} at line {lineno}")
        numbered[number] = lineno
        if len(level) == 1:
            top.append((number, title))

    expected = [str(i) for i in range(1, len(top) + 1)]
    actual = [number for number, _ in top]
    if actual != expected:
        fail(f"top-level sections are not sequential 1..N: {actual}")

    return numbered, top


def check_section_refs(lines: list[str], numbered: dict[str, int]) -> None:
    missing = sorted(
        {
            ref
            for line in lines
            for ref in SECTION_REF.findall(line)
            if ref not in numbered
        }
    )
    if missing:
        fail(f"section references without a heading: {', '.join('§' + m for m in missing)}")


def check_fences(lines: list[str]) -> None:
    markers = [i for i, line in enumerate(lines, 1) if line.strip().startswith("```")]
    if len(markers) % 2:
        fail(f"unbalanced code fences ({len(markers)} markers): {markers}")


def check_tables(lines: list[str]) -> int:
    in_fence = False
    blocks: list[list[tuple[int, int]]] = []
    current: list[tuple[int, int]] = []

    for lineno, line in enumerate(lines, 1):
        if line.strip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if line.lstrip().startswith("|"):
            current.append((lineno, line.count("|")))
        elif current:
            blocks.append(current)
            current = []
    if current:
        blocks.append(current)

    for block in blocks:
        counts = {count for _, count in block}
        if len(counts) > 1:
            fail(
                "inconsistent pipe counts in table ending at line "
                f"{block[-1][0]}: {sorted(counts)}"
            )
    return len(blocks)


def check_references(lines: list[str]) -> None:
    defined: set[int] = set()
    cited: set[int] = set()
    for line in lines:
        definition = REF_DEF.match(line)
        if definition:
            defined.add(int(definition.group(1)))
            continue
        cited.update(int(n) for n in REF_CITE.findall(line))

    if not defined:
        fail("no reference definitions found (expected '- [R1] ...')")
        return

    missing_numbers = [n for n in range(1, max(defined) + 1) if n not in defined]
    if missing_numbers:
        fail(f"reference definitions are not contiguous: missing {missing_numbers}")

    undefined = sorted(cited - defined)
    uncited = sorted(defined - cited)
    if undefined:
        fail(f"citations without a definition: {', '.join(f'R{n}' for n in undefined)}")
    if uncited:
        fail(f"references defined but never cited: {', '.join(f'R{n}' for n in uncited)}")


def check_toc(lines: list[str], top: list[tuple[str, str]]) -> None:
    start = None
    for index, line in enumerate(lines):
        if HEADING_ANY.match(line) and line.startswith("### ") and "table of contents" in line.lower():
            start = index + 1
            break
    if start is None:
        notes.append("no table of contents heading found; skipping TOC check")
        return

    items: list[str] = []
    for line in lines[start:]:
        if HEADING_ANY.match(line):
            break
        stripped = line.strip()
        if not stripped:
            if items:
                continue
            continue
        match = TOC_ITEM.match(stripped)
        if match:
            items.append(match.group(1).strip())

    expected = [title for _, title in top]
    if items != expected:
        fail("table of contents does not match top-level headings")
        for index, (want, got) in enumerate(zip(expected, items), 1):
            if want != got:
                fail(f"  §{index}: expected {want!r}, found {got!r}")
                break
        if len(items) != len(expected):
            fail(f"  TOC has {len(items)} entries, report has {len(expected)} sections")


def main(argv: list[str]) -> int:
    report = find_report(argv)
    if not report.exists():
        raise SystemExit(f"Report not found: {report}")

    try:
        text = report.read_text(encoding="utf-8-sig")
    except UnicodeDecodeError as exc:
        raise SystemExit(f"{report} is not valid UTF-8: {exc}") from None
    lines = text.splitlines()

    numbered, top = check_headings(lines)
    check_section_refs(lines, numbered)
    check_fences(lines)
    tables = check_tables(lines)
    check_references(lines)
    check_toc(lines, top)

    duplicates = [
        title for title, count in Counter(t for _, t in top).items() if count > 1
    ]
    if duplicates:
        fail(f"duplicate top-level titles: {duplicates}")

    try:
        display = report.relative_to(ROOT)
    except ValueError:
        display = report
    print(f"report: {display}")
    print(f"lines: {len(lines)}  sections: {len(top)}  headings: {len(numbered)}  tables: {tables}")
    for note in notes:
        print(f"note: {note}")

    if failures:
        print(f"\nFAIL ({len(failures)}):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print("OK: headings, section references, fences, tables, references, TOC")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
