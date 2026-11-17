"""P12.02 — tests for snapshot_manifest.py (stdlib unittest).

WRITTEN-UNRUN: complete test files are written by the loop and EXECUTED by
the human build/test gate (`01_PROGRAM_CONVENTIONS.md` §1 rule 3) — no test
in this file has ever run in the authoring loop, and no pass is claimed.

Run (human gate):
    python -m unittest deploy.compose.backup.test_snapshot_manifest -v
    (or: python test_snapshot_manifest.py)
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import snapshot_manifest as sm


class ManifestBuildTests(unittest.TestCase):
    def _make_root(self, tmp: str, files: dict[str, bytes]) -> Path:
        root = Path(tmp) / "artifact-root"
        for rel, payload in files.items():
            target = root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(payload)
        return root

    def test_manifest_lists_all_files_with_digests(self) -> None:
        files = {
            "a.bin": b"alpha",
            "sub/b.bin": b"beta",
            "sub/deep/c.bin": b"gamma",
        }
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, files)
            target = sm.write_manifest(root)
            self.assertEqual(target.name, sm.MANIFEST_NAME)
            manifest = json.loads(target.read_text(encoding="utf-8"))
            self.assertEqual(manifest["format"], sm.MANIFEST_FORMAT)
            paths = [e["path"] for e in manifest["entries"]]
            self.assertEqual(paths, ["a.bin", "sub/b.bin", "sub/deep/c.bin"])
            for entry in manifest["entries"]:
                self.assertEqual(entry["sha256"], sm.sha256_file(root / str(entry["path"])))
                self.assertEqual(entry["size"], (root / str(entry["path"])).stat().st_size)

    def test_manifest_is_deterministic_and_excludes_itself(self) -> None:
        files = {"b.bin": b"one", "a.bin": b"two"}
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, files)
            first = sm.write_manifest(root)
            first_text = first.read_text(encoding="utf-8")
            second = sm.write_manifest(root)
            self.assertEqual(first_text, second.read_text(encoding="utf-8"))
            manifest = json.loads(first_text)
            self.assertNotIn(sm.MANIFEST_NAME, [e["path"] for e in manifest["entries"]])

    def test_empty_root_yields_empty_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "empty"
            root.mkdir()
            sm.write_manifest(root)
            manifest = json.loads((root / sm.MANIFEST_NAME).read_text(encoding="utf-8"))
            self.assertEqual(manifest["entries"], [])


class ManifestVerifyTests(unittest.TestCase):
    def _make_root(self, tmp: str, files: dict[str, bytes]) -> Path:
        root = Path(tmp) / "artifact-root"
        for rel, payload in files.items():
            target = root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(payload)
        return root

    def test_verify_passes_on_untouched_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, {"a.bin": b"payload", "sub/b.bin": b"more"})
            sm.write_manifest(root)
            self.assertEqual(sm.verify_manifest(root), [])

    def test_verify_counts_modified_file_as_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, {"a.bin": b"original"})
            sm.write_manifest(root)
            (root / "a.bin").write_bytes(b"tampered")
            problems = sm.verify_manifest(root)
            self.assertEqual(problems, ["digest mismatch: a.bin"])

    def test_verify_counts_missing_and_unexpected_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, {"a.bin": b"keep", "old.bin": b"gone"})
            sm.write_manifest(root)
            (root / "old.bin").unlink()
            (root / "new.bin").write_bytes(b"added")
            problems = sm.verify_manifest(root)
            self.assertEqual(
                problems,
                ["missing file: old.bin", "unexpected file: new.bin"],
            )

    def test_verify_fails_closed_without_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, {"a.bin": b"data"})
            problems = sm.verify_manifest(root)
            self.assertEqual(len(problems), 1)
            self.assertTrue(problems[0].startswith("manifest missing:"))

    def test_verify_rejects_unknown_format(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, {"a.bin": b"data"})
            (root / sm.MANIFEST_NAME).write_text(json.dumps({"format": 999, "entries": []}), encoding="utf-8")
            problems = sm.verify_manifest(root)
            self.assertEqual(problems, ["unsupported manifest format: 999"])


class ManifestMainTests(unittest.TestCase):
    def _make_root(self, tmp: str, files: dict[str, bytes]) -> Path:
        root = Path(tmp) / "artifact-root"
        for rel, payload in files.items():
            target = root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(payload)
        return root

    def test_main_build_and_verify_round_trip(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, {"a.bin": b"roundtrip"})
            self.assertEqual(sm.main(["--root", str(root)]), 0)
            self.assertEqual(sm.main(["--root", str(root), "--verify"]), 0)

    def test_main_verify_returns_nonzero_on_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = self._make_root(tmp, {"a.bin": b"intact"})
            sm.write_manifest(root)
            (root / "a.bin").write_bytes(b"corrupt")
            self.assertEqual(sm.main(["--root", str(root), "--verify"]), 1)

    def test_main_returns_66_for_missing_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "does-not-exist"
            self.assertEqual(sm.main(["--root", str(missing)]), 66)


if __name__ == "__main__":
    unittest.main()
