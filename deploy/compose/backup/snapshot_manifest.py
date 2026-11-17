#!/usr/bin/env python3
"""P12.02 — backup-artifact integrity manifest tool (stdlib only).

WRITTEN-UNRUN (P12.02): never executed in the authoring loop; the first
human run is the gate and may surface defects. Companion suite:
``test_snapshot_manifest.py`` (stdlib unittest, also written-unrun).

Builds a deterministic JSON manifest of every regular file under a root
directory (sha256, byte size, path sorted ascending) — the integrity record
uploaded alongside PostgreSQL-repo copies and JetStream snapshots to the
P12.01 §4 object-storage target. ``--verify`` recomputes and compares; any
difference (missing file, changed digest, extra file) is a FAILURE — a
corrupted or incomplete artifact is a counted loss, never silently accepted
(the P9.24 §5 R-5 discipline: CRC/verification failures are counted, never
ignored).

No manifest is a security boundary: sha256 is integrity evidence, not a
signature; the release-signing discipline lives elsewhere and is untouched.

Usage:
    python snapshot_manifest.py --root <dir>            # build manifest.json
    python snapshot_manifest.py --root <dir> --verify   # verify against it
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

MANIFEST_NAME = "manifest.json"
MANIFEST_FORMAT = 1
CHUNK = 1 << 20


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            block = handle.read(CHUNK)
            if not block:
                break
            digest.update(block)
    return digest.hexdigest()


def build_entries(root: Path) -> list[dict[str, object]]:
    entries: list[dict[str, object]] = []
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        if path.name == MANIFEST_NAME:
            continue
        entries.append(
            {
                "path": path.relative_to(root).as_posix(),
                "size": path.stat().st_size,
                "sha256": sha256_file(path),
            }
        )
    return entries


def build_manifest(root: Path) -> dict[str, object]:
    return {
        "format": MANIFEST_FORMAT,
        "root_name": root.name,
        "entries": build_entries(root),
    }


def write_manifest(root: Path) -> Path:
    target = root / MANIFEST_NAME
    payload = json.dumps(build_manifest(root), indent=2, sort_keys=True) + "\n"
    target.write_text(payload, encoding="utf-8", newline="\n")
    return target


def verify_manifest(root: Path) -> list[str]:
    target = root / MANIFEST_NAME
    if not target.is_file():
        return [f"manifest missing: {target}"]
    recorded = json.loads(target.read_text(encoding="utf-8"))
    if recorded.get("format") != MANIFEST_FORMAT:
        return [f"unsupported manifest format: {recorded.get('format')!r}"]

    problems: list[str] = []
    expected = {str(e["path"]): e for e in recorded.get("entries", [])}
    actual = {str(e["path"]): e for e in build_entries(root)}

    for path in sorted(set(expected) - set(actual)):
        problems.append(f"missing file: {path}")
    for path in sorted(set(actual) - set(expected)):
        problems.append(f"unexpected file: {path}")
    for path in sorted(set(expected) & set(actual)):
        if expected[path]["size"] != actual[path]["size"]:
            problems.append(f"size mismatch: {path}")
        elif expected[path]["sha256"] != actual[path]["sha256"]:
            problems.append(f"digest mismatch: {path}")
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", required=True, type=Path, help="artifact root directory")
    parser.add_argument("--verify", action="store_true", help="verify an existing manifest")
    args = parser.parse_args(argv)

    root = args.root
    if not root.is_dir():
        print(f"snapshot_manifest: not a directory: {root}", file=sys.stderr)
        return 66

    if args.verify:
        problems = verify_manifest(root)
        if problems:
            for problem in problems:
                print(f"VERIFY FAIL: {problem}", file=sys.stderr)
            print(f"snapshot_manifest: {len(problems)} problem(s) under {root}", file=sys.stderr)
            return 1
        print(f"VERIFY OK: manifest matches all artifacts under {root}")
        return 0

    target = write_manifest(root)
    print(f"MANIFEST OK: {target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
