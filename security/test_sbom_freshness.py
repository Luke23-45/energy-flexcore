"""P19.17 — tests for sbom_freshness.py (stdlib unittest).

WRITTEN-UNRUN: complete test files are written by the loop and EXECUTED by
the human build/test gate (``01_PROGRAM_CONVENTIONS.md`` §1 rule 3) — no test
in this file has ever run in the authoring loop, and no pass is claimed.

Run (human gate):
    python -I -m unittest discover -s security -p "test_sbom_freshness.py" -v
    (or from the security/ directory: python test_sbom_freshness.py)

What is pinned (against the P19.08 §6 thresholds and the P12.02 verifier
pattern): manifest structure and format gate; F-1 artifact-digest currency
and the artifact-newer-than-SBOM rule; F-3 the 90-day advisory ceiling as
SCAN-STALE; declaration-vs-file-mtime agreement; fail-closed counting of
missing, undeclared, malformed, duplicated, and future-dated declarations;
the injected-clock requirement (no wall clock, ever); and the CLI exit
codes. Nothing here runs a scanner, audit tool, or advisory query, and no
freshness RESULT exists anywhere — the fixtures below are synthetic.
"""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path

import sbom_freshness as sf


def sha_of(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


class FreshnessFixtureMixin:
    """Builds a declared tree: a manifest plus the files it declares."""

    NOW_MS = 1_700_000_000_000  # the INJECTED clock — never wall time
    DAY_MS = 86_400_000

    def _make_tree(self, tmp: str, files: dict[str, bytes], mtimes: dict[str, int] | None = None,
                   manifest: dict[str, object] | None = None) -> Path:
        tree = Path(tmp) / "evidence-tree"
        tree.mkdir(parents=True, exist_ok=True)
        for rel, payload in files.items():
            target = tree / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(payload)
        for rel, ms in (mtimes or {}).items():
            target = tree / rel
            stat = target.stat()
            os.utime(target, ns=(stat.st_atime_ns, int(ms * 1_000_000)))
        if manifest is None:
            manifest = {"format": sf.MANIFEST_FORMAT, "entries": []}
        (tree / sf.MANIFEST_NAME).write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        return tree

    def _sbom_entry(self, rel: str, artifact: str, artifact_payload: bytes,
                    generated_ms: int | None = None) -> dict[str, object]:
        entry: dict[str, object] = {
            "path": rel,
            "kind": sf.KIND_SBOM,
            "artifact": artifact,
            "artifact_sha256": sha_of(artifact_payload),
        }
        if generated_ms is not None:
            entry["generated_at_unix_ms"] = generated_ms
        return entry


class ManifestStructureTests(FreshnessFixtureMixin, unittest.TestCase):
    def test_missing_manifest_is_a_counted_problem(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tree = Path(tmp) / "empty"
            tree.mkdir()
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("freshness manifest missing", problems[0])

    def test_unsupported_format_fails_closed(self) -> None:
        manifest = {"format": 999, "entries": []}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {}, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("unsupported freshness-manifest format", problems[0])

    def test_manifest_without_entries_list_fails_closed(self) -> None:
        manifest = {"format": sf.MANIFEST_FORMAT}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {}, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("carries no entries list", problems[0])

    def test_malformed_entry_is_a_problem_never_a_skip(self) -> None:
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": ["not-a-dict", {"kind": "sbom"}]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {}, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 2)
            self.assertIn("malformed freshness entry", problems[0])
            self.assertIn("malformed freshness entry", problems[1])

    def test_undeclared_file_is_counted_fail_closed(self) -> None:
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": []}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {"control-plane.cdx.json": b"sbom"}, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("undeclared file: control-plane.cdx.json", problems[0])

    def test_duplicate_and_self_referential_declarations_are_counted(self) -> None:
        entry = {"path": "dup.cdx.json", "kind": sf.KIND_SBOM,
                 "artifact": "a.bin", "artifact_sha256": sha_of(b"a")}
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry, dict(entry)]}
        files = {"dup.cdx.json": b"sbom", "a.bin": b"a"}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertTrue(any("duplicate" in p for p in problems), problems)

        self_entry = {"path": sf.MANIFEST_NAME, "kind": sf.KIND_ADVISORY_SCAN,
                      "generated_at_unix_ms": self.NOW_MS}
        manifest2 = {"format": sf.MANIFEST_FORMAT, "entries": [self_entry]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {}, manifest=manifest2)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertTrue(any("self-referential" in p for p in problems), problems)

    def test_unknown_kind_is_a_problem(self) -> None:
        manifest = {"format": sf.MANIFEST_FORMAT,
                    "entries": [{"path": "mystery.bin", "kind": "vibe"}]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {"mystery.bin": b"x"}, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("unknown kind", problems[0])


class F1ArtifactCurrencyTests(FreshnessFixtureMixin, unittest.TestCase):
    def test_current_sbom_with_matching_digest_and_mtime_verifies(self) -> None:
        payload = b"artifact-v1"
        files = {"control-plane.cdx.json": b"sbom-json", "binaries/control-plane": payload}
        entry = self._sbom_entry("control-plane.cdx.json", "binaries/control-plane", payload,
                                 generated_ms=self.NOW_MS - 3_600_000)
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        mtimes = {"control-plane.cdx.json": self.NOW_MS - 3_600_000,
                  "binaries/control-plane": self.NOW_MS - 7_200_000}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, mtimes=mtimes, manifest=manifest)
            self.assertEqual(sf.verify_tree(tree, self.NOW_MS, 90, 24), [])

    def test_changed_artifact_is_f1_stale(self) -> None:
        files = {"control-plane.cdx.json": b"sbom-json", "binaries/control-plane": b"artifact-v2"}
        entry = self._sbom_entry("control-plane.cdx.json", "binaries/control-plane", b"artifact-v1")
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("ARTIFACT CHANGED (F-1 STALE)", problems[0])

    def test_artifact_newer_than_sbom_is_f1_stale(self) -> None:
        payload = b"artifact-v1"
        files = {"control-plane.cdx.json": b"sbom-json", "binaries/control-plane": payload}
        entry = self._sbom_entry("control-plane.cdx.json", "binaries/control-plane", payload)
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        mtimes = {"control-plane.cdx.json": self.NOW_MS - 7_200_000,
                  "binaries/control-plane": self.NOW_MS - 1_800_000}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, mtimes=mtimes, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("ARTIFACT NEWER THAN SBOM (F-1 STALE)", problems[0])

    def test_missing_declared_artifact_or_digest_fails_closed(self) -> None:
        entry = {"path": "s.cdx.json", "kind": sf.KIND_SBOM, "artifact": "gone.bin",
                 "artifact_sha256": sha_of(b"a")}
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {"s.cdx.json": b"sbom"}, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("declared artifact missing", problems[0])

        entry = {"path": "s2.cdx.json", "kind": sf.KIND_SBOM,
                 "artifact": "a.bin", "artifact_sha256": "tooshort"}
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {"s2.cdx.json": b"sbom", "a.bin": b"a"}, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("no 64-hex artifact_sha256", problems[0])


class F3AdvisoryWindowTests(FreshnessFixtureMixin, unittest.TestCase):
    def _scan_entry(self, rel: str, generated_ms: int) -> dict[str, object]:
        return {"path": rel, "kind": sf.KIND_ADVISORY_SCAN,
                "generated_at_unix_ms": generated_ms}

    def test_scan_within_the_window_verifies(self) -> None:
        files = {"cargo-audit-run.json": b"run"}
        entry = self._scan_entry("cargo-audit-run.json", self.NOW_MS - 30 * self.DAY_MS)
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        mtimes = {"cargo-audit-run.json": self.NOW_MS - 30 * self.DAY_MS}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, mtimes=mtimes, manifest=manifest)
            self.assertEqual(sf.verify_tree(tree, self.NOW_MS, 90, 24), [])

    def test_scan_beyond_the_ceiling_is_scan_stale_never_a_clear(self) -> None:
        files = {"cargo-audit-run.json": b"run"}
        entry = self._scan_entry("cargo-audit-run.json", self.NOW_MS - 91 * self.DAY_MS)
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        mtimes = {"cargo-audit-run.json": self.NOW_MS - 91 * self.DAY_MS}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, mtimes=mtimes, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("SCAN-STALE (F-3)", problems[0])

    def test_a_tighter_window_makes_a_younger_scan_stale(self) -> None:
        files = {"cargo-audit-run.json": b"run"}
        entry = self._scan_entry("cargo-audit-run.json", self.NOW_MS - 10 * self.DAY_MS)
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        mtimes = {"cargo-audit-run.json": self.NOW_MS - 10 * self.DAY_MS}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, mtimes=mtimes, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 7, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("SCAN-STALE (F-3)", problems[0])

    def test_scan_without_a_generated_at_fails_closed(self) -> None:
        files = {"cargo-audit-run.json": b"run"}
        manifest = {"format": sf.MANIFEST_FORMAT,
                    "entries": [{"path": "cargo-audit-run.json", "kind": sf.KIND_ADVISORY_SCAN}]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("no integer generated_at_unix_ms", problems[0])

    def test_future_dated_declaration_is_a_problem(self) -> None:
        files = {"cargo-audit-run.json": b"run"}
        entry = self._scan_entry("cargo-audit-run.json", self.NOW_MS + self.DAY_MS)
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        mtimes = {"cargo-audit-run.json": self.NOW_MS + self.DAY_MS}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, mtimes=mtimes, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertTrue(any("IN THE FUTURE" in p for p in problems), problems)


class DeclarationVsFileTests(FreshnessFixtureMixin, unittest.TestCase):
    def test_declaration_far_from_the_file_mtime_is_a_problem(self) -> None:
        files = {"cargo-audit-run.json": b"run"}
        entry = {"path": "cargo-audit-run.json", "kind": sf.KIND_ADVISORY_SCAN,
                 "generated_at_unix_ms": self.NOW_MS - 2 * self.DAY_MS}
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        mtimes = {"cargo-audit-run.json": self.NOW_MS - 10 * self.DAY_MS}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, mtimes=mtimes, manifest=manifest)
            problems = sf.verify_tree(tree, self.NOW_MS, 90, 24)
            self.assertEqual(len(problems), 1)
            self.assertIn("DECLARATION MISMATCH", problems[0])

    def test_declaration_within_tolerance_is_accepted(self) -> None:
        files = {"cargo-audit-run.json": b"run"}
        entry = {"path": "cargo-audit-run.json", "kind": sf.KIND_ADVISORY_SCAN,
                 "generated_at_unix_ms": self.NOW_MS - 6 * 3_600_000}
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        mtimes = {"cargo-audit-run.json": self.NOW_MS - 10 * 3_600_000}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, mtimes=mtimes, manifest=manifest)
            self.assertEqual(sf.verify_tree(tree, self.NOW_MS, 90, 24), [])


class CliExitCodeTests(FreshnessFixtureMixin, unittest.TestCase):
    def test_exit_64_when_no_clock_is_supplied(self) -> None:
        # The clock is INJECTED — the tool refuses to invent one (never wall
        # time; a run whose 'now' cannot be reviewed verifies nothing).
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {})
            self.assertEqual(sf.main(["--tree", str(tree), "--verify"]), 64)

    def test_exit_66_when_tree_is_not_a_directory(self) -> None:
        self.assertEqual(
            sf.main(["--tree", "definitely/not/there", "--verify",
                     "--now-unix-ms", str(self.NOW_MS)]),
            66,
        )

    def test_exit_64_when_verify_is_omitted(self) -> None:
        # The tool never WRITES a freshness manifest (machine-generated by
        # the P19.08 procedure only) — an invocation without --verify is a
        # usage error, not a build step.
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, {})
            self.assertEqual(sf.main(["--tree", str(tree), "--now-unix-ms", str(self.NOW_MS)]), 64)

    def test_exit_0_on_a_clean_tree_and_1_on_counted_problems(self) -> None:
        payload = b"artifact-v1"
        files = {"s.cdx.json": b"sbom-json", "a.bin": payload}
        entry = self._sbom_entry("s.cdx.json", "a.bin", payload)
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [entry]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, manifest=manifest)
            self.assertEqual(
                sf.main(["--tree", str(tree), "--verify", "--now-unix-ms", str(self.NOW_MS)]), 0
            )

        stale = self._sbom_entry("s.cdx.json", "a.bin", b"different-artifact")
        manifest = {"format": sf.MANIFEST_FORMAT, "entries": [stale]}
        with tempfile.TemporaryDirectory() as tmp:
            tree = self._make_tree(tmp, files, manifest=manifest)
            self.assertEqual(
                sf.main(["--tree", str(tree), "--verify", "--now-unix-ms", str(self.NOW_MS)]), 1
            )

    def test_now_iso_parses_utc_z_suffix(self) -> None:
        # The expected value is COMPUTED (never a hand-transcribed epoch):
        # the Z form and the explicit-offset form must parse identically.
        import datetime

        expected = int(
            datetime.datetime(2026, 11, 17, tzinfo=datetime.timezone.utc).timestamp() * 1000
        )
        self.assertEqual(sf.parse_now_iso("2026-11-17T00:00:00Z"), expected)
        self.assertEqual(sf.parse_now_iso("2026-11-17T00:00:00+00:00"), expected)


if __name__ == "__main__":
    unittest.main()
