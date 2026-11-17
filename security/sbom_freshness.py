#!/usr/bin/env python3
"""P19.17 — SBOM/evidence freshness verifier (stdlib only).

WRITTEN-UNRUN (P19.17): never executed in the authoring loop; the first
human run is the gate and may surface defects. Companion suite:
``test_sbom_freshness.py`` (stdlib unittest, also written-unrun).
Pattern mirror: the P12.02 ``snapshot_manifest.py`` verifier (sorted paths,
sha256, counted and named problems, fail-closed exit codes).

Checks the P19.08 §6 freshness thresholds (record
``docs/phases-11-20/phase-19-security-certification-compliance/records/
08_p19_08_sbom_generation_process.md``) against the FILES on disk:

- F-1 (build freshness): an SBOM is CURRENT only for the exact artifact
  digest it was generated from. Each ``sbom`` entry declares its source
  artifact (relative path + sha256); the verifier recomputes the
  artifact's digest and compares, and additionally requires the SBOM
  file's mtime to be at least as recent as the artifact's mtime — an
  artifact that changed after the SBOM was written is STALE, counted,
  never silently accepted.
- F-3 (advisory-window freshness): an ``advisory-scan`` entry declares the
  ``generated_at_unix_ms`` of the scan; older than the registered ceiling
  (default 90 days, the F-3 value) it is SCAN-STALE — never a clear (the
  P10.20 §8 coverage-honesty rule).
- DECLARATION vs FILE: every declared timestamp must agree with the
  file's actual mtime within a tolerance (default 24 h, settable). A
  declaration that disagrees with the file it describes is a problem —
  the declared freshness of a file is only meaningful if it is the
  freshness of THAT file.

HONEST BOUNDARIES (read before trusting a green exit):

- This script CERTIFIES NOTHING. A green exit means "no counted problem
  in the declared tree under the declared thresholds" — it is a test
  outcome about THIS check's coverage, never a guarantee, and never a
  scan result (no scanner, audit tool, or advisory database is invoked
  here).
- The freshness manifest is MACHINE-GENERATED data of the P19.08 §4/§5
  procedure (per-image CycloneDX storage next to artifacts, keyed by
  candidate id). It is never hand-maintained; a hand-edited manifest is
  exactly the failure mode the declaration-vs-file check exists to catch.
- sha256 here is INTEGRITY evidence, not a signature; the release-signing
  discipline lives elsewhere and is untouched.
- F-2 (regeneration ordering: SBOMs regenerate BEFORE the advisory scans)
  is the P19.09 §5 runbook's PROCESS ordering — deliberately not coded
  here; a script cannot vouch for an ordering it did not observe.

Usage (human gate; the clock is INJECTED — never wall time, mirroring the
program's injected-clock discipline):

    python sbom_freshness.py --tree <dir> --now-unix-ms <ms> --verify
    python sbom_freshness.py --tree <dir> --now-iso 2026-11-17T00:00:00Z --verify
    # optional: --advisory-window-days 90 (the F-3 registered ceiling)
    #           --declaration-tolerance-hours 24

Exit codes: 0 no counted problem; 1 counted problems (each printed, named);
64 bad invocation; 66 the tree is not a directory.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import sys
from pathlib import Path

MANIFEST_NAME = "freshness.json"
MANIFEST_FORMAT = 1
CHUNK = 1 << 20

KIND_SBOM = "sbom"
KIND_ADVISORY_SCAN = "advisory-scan"
KNOWN_KINDS = (KIND_SBOM, KIND_ADVISORY_SCAN)
DEFAULT_ADVISORY_WINDOW_DAYS = 90  # the F-3 registered ceiling (P19.08 §6)
DEFAULT_DECLARATION_TOLERANCE_HOURS = 24


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            block = handle.read(CHUNK)
            if not block:
                break
            digest.update(block)
    return digest.hexdigest()


def mtime_unix_ms(path: Path) -> int:
    return int(round(path.stat().st_mtime * 1000))


def parse_now_iso(text: str) -> int:
    """Parses an ISO-8601 UTC instant into unix milliseconds (fail closed on
    anything unparseable — an unverifiable 'now' verifies nothing)."""
    normalized = text.strip()
    if normalized.endswith(("Z", "z")):
        normalized = normalized[:-1] + "+00:00"
    try:
        moment = _dt.datetime.fromisoformat(normalized)
    except ValueError:
        raise SystemExit(f"sbom_freshness: --now-iso is not parseable: {text!r}") from None
    if moment.tzinfo is None:
        moment = moment.replace(tzinfo=_dt.timezone.utc)
    return int(moment.timestamp() * 1000)


def _entry_problems(entry: dict[str, object], tree: Path, now_ms: int,
                    window_days: int, tolerance_hours: int) -> list[str]:
    problems: list[str] = []
    rel = str(entry.get("path", ""))
    kind = str(entry.get("kind", ""))
    if kind not in KNOWN_KINDS:
        problems.append(f"unknown kind: {rel} declares {kind!r}")
        return problems

    target = tree / rel
    if not target.is_file():
        problems.append(f"declared file missing: {rel}")
        return problems

    file_ms = mtime_unix_ms(target)
    tolerance_ms = tolerance_hours * 3_600_000

    if kind == KIND_SBOM:
        # F-1: currency against the exact artifact.
        artifact_rel = entry.get("artifact")
        artifact_sha = entry.get("artifact_sha256")
        if not isinstance(artifact_rel, str) or not artifact_rel:
            problems.append(f"sbom entry declares no artifact: {rel}")
            return problems
        if not isinstance(artifact_sha, str) or len(artifact_sha) != 64:
            problems.append(f"sbom entry declares no 64-hex artifact_sha256: {rel}")
            return problems
        artifact = tree / artifact_rel
        if not artifact.is_file():
            problems.append(f"declared artifact missing: {artifact_rel} (for {rel})")
            return problems
        actual_sha = sha256_file(artifact)
        if actual_sha != artifact_sha:
            # F-1: the artifact changed after the SBOM was generated.
            problems.append(
                f"ARTIFACT CHANGED (F-1 STALE): {rel} was generated from "
                f"{artifact_rel} at sha256 {artifact_sha[:12]}..., but the artifact "
                f"now hashes to {actual_sha[:12]}... — the SBOM is evidence for the "
                f"old artifact only"
            )
        elif mtime_unix_ms(artifact) > file_ms + tolerance_ms:
            problems.append(
                f"ARTIFACT NEWER THAN SBOM (F-1 STALE): {artifact_rel} mtime postdates "
                f"{rel} beyond tolerance — regenerate before any go decision"
            )
        # A declared generation timestamp, if present, must agree with the file.
        declared = entry.get("generated_at_unix_ms")
        if declared is not None:
            if not isinstance(declared, int):
                problems.append(f"declared generated_at_unix_ms is not an integer: {rel}")
            elif abs(file_ms - declared) > tolerance_ms:
                problems.append(
                    f"DECLARATION MISMATCH: {rel} declares generated_at={declared} but its "
                    f"file mtime is {file_ms} (beyond the {tolerance_hours}h tolerance)"
                )

    if kind == KIND_ADVISORY_SCAN:
        # F-3: the advisory-window ceiling. A scan older than the window is
        # SCAN-STALE — never a clear.
        declared = entry.get("generated_at_unix_ms")
        if not isinstance(declared, int):
            problems.append(f"advisory-scan entry declares no integer generated_at_unix_ms: {rel}")
            return problems
        age_ms = now_ms - declared
        window_ms = window_days * 86_400_000
        if age_ms > window_ms:
            problems.append(
                f"SCAN-STALE (F-3): {rel} was generated {age_ms // 86_400_000} days ago, "
                f"beyond the {window_days}-day ceiling — never a clear (the P10.20 §8 "
                f"coverage-honesty rule)"
            )
        if age_ms < 0:
            problems.append(
                f"DECLARATION IN THE FUTURE: {rel} declares generated_at={declared} "
                f"after the injected now={now_ms} — an unverifiable declaration"
            )
        if abs(file_ms - declared) > tolerance_ms:
            problems.append(
                f"DECLARATION MISMATCH: {rel} declares generated_at={declared} but its "
                f"file mtime is {file_ms} (beyond the {tolerance_hours}h tolerance)"
            )

    return problems


def verify_tree(tree: Path, now_ms: int, window_days: int,
                tolerance_hours: int) -> list[str]:
    """Returns the COUNTED, named problems; an empty list means none were
    found (never a guarantee — only a statement about this check)."""
    manifest_path = tree / MANIFEST_NAME
    if not manifest_path.is_file():
        return [f"freshness manifest missing: {manifest_path}"]
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return [f"freshness manifest unreadable: {manifest_path}: {exc}"]
    if manifest.get("format") != MANIFEST_FORMAT:
        return [f"unsupported freshness-manifest format: {manifest.get('format')!r}"]
    entries = manifest.get("entries")
    if not isinstance(entries, list):
        return [f"freshness manifest carries no entries list: {manifest_path}"]

    problems: list[str] = []

    # Fail closed on declared entries: every declaration is checked; a
    # malformed entry is a problem, never a skip.
    declared: set[str] = set()
    for entry in entries:
        if not isinstance(entry, dict) or "path" not in entry:
            problems.append(f"malformed freshness entry (no path): {entry!r}")
            continue
        rel = str(entry["path"])
        if rel == MANIFEST_NAME or rel in declared:
            problems.append(f"duplicate or self-referential declaration: {rel}")
            continue
        declared.add(rel)
        problems.extend(
            _entry_problems(entry, tree, now_ms, window_days, tolerance_hours)
        )

    # Fail closed on undeclared files: an SBOM or scan present but not
    # declared is exactly the drift F-1 exists to catch (a coverage gap is
    # an unknown, never a clear).
    for path in sorted(tree.rglob("*")):
        if not path.is_file() or path.name == MANIFEST_NAME:
            continue
        rel = path.relative_to(tree).as_posix()
        if rel not in declared:
            problems.append(f"undeclared file: {rel} — declare it or remove it")

    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tree", required=True, type=Path, help="declared SBOM/evidence tree")
    parser.add_argument("--verify", action="store_true", help="verify the freshness manifest")
    parser.add_argument("--now-unix-ms", type=int, default=None, help="the injected now, unix ms")
    parser.add_argument("--now-iso", type=str, default=None, help="the injected now, ISO-8601 UTC")
    parser.add_argument(
        "--advisory-window-days", type=int, default=DEFAULT_ADVISORY_WINDOW_DAYS,
        help="the F-3 ceiling (default: the registered 90)",
    )
    parser.add_argument(
        "--declaration-tolerance-hours", type=int, default=DEFAULT_DECLARATION_TOLERANCE_HOURS,
        help="how far a declared timestamp may disagree with the file mtime (default 24h)",
    )
    args = parser.parse_args(argv)

    if args.now_unix_ms is None and args.now_iso is None:
        print(
            "sbom_freshness: no clock supplied — pass --now-unix-ms or --now-iso. "
            "The clock is INJECTED (never wall time): an unverifiable 'now' verifies nothing.",
            file=sys.stderr,
        )
        return 64
    now_ms = args.now_unix_ms if args.now_unix_ms is not None else parse_now_iso(args.now_iso)

    tree = args.tree
    if not tree.is_dir():
        print(f"sbom_freshness: not a directory: {tree}", file=sys.stderr)
        return 66

    if not args.verify:
        print(
            "sbom_freshness: nothing to do — pass --verify. The freshness manifest is "
            "MACHINE-GENERATED by the P19.08 §4/§5 procedure; this tool never writes one.",
            file=sys.stderr,
        )
        return 64

    problems = verify_tree(tree, now_ms, args.advisory_window_days, args.declaration_tolerance_hours)
    if problems:
        for problem in problems:
            print(f"VERIFY FAIL: {problem}", file=sys.stderr)
        print(
            f"sbom_freshness: {len(problems)} counted problem(s) under {tree} "
            f"(F-window {args.advisory_window_days}d, tolerance {args.declaration_tolerance_hours}h)",
            file=sys.stderr,
        )
        return 1

    print(
        f"VERIFY OK: no counted freshness problem under {tree} "
        f"(F-window {args.advisory_window_days}d, tolerance {args.declaration_tolerance_hours}h) — "
        "a statement about this check's coverage, never a certification"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
