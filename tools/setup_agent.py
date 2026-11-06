#!/usr/bin/env python3
"""Expose .agent/skills through the discovery paths used by common agents.

The skill files stay in .agent/skills. This script creates one directory link
per skill in the target discovery directory:

    .agents/skills/<id>    Gemini CLI, OpenCode, and tools using the alias
    .claude/skills/<id>    Claude Code (symlinked skill folders are supported)
    .opencode/skills/<id>  OpenCode only (optional)

On Windows it uses a directory junction, which does not require administrator
rights. On POSIX it uses a symlink. If linking is unavailable, it copies the
directory and says so; rerun after editing a skill to refresh the copy.

    python tools/setup_agent.py --list
    python tools/setup_agent.py --all
    python tools/setup_agent.py --link claude
    python tools/setup_agent.py --unlink all
"""

from __future__ import annotations

import argparse
import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / ".agent" / "skills"

TARGETS: dict[str, Path] = {
    "agents": ROOT / ".agents" / "skills",
    "claude": ROOT / ".claude" / "skills",
    "opencode": ROOT / ".opencode" / "skills",
}


def skill_dirs() -> list[Path]:
    if not SOURCE.exists():
        return []
    return sorted(path for path in SOURCE.iterdir() if (path / "SKILL.md").is_file())


def is_link(path: Path) -> bool:
    if path.is_symlink():
        return True
    if os.name == "nt" and path.exists():
        attributes = os.lstat(path).st_file_attributes
        return bool(attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT)
    return False


def points_to(path: Path, target: Path) -> bool:
    try:
        return Path(os.path.realpath(path)) == Path(os.path.realpath(target))
    except OSError:
        return False


def make_link(source: Path, destination: Path) -> str:
    destination.parent.mkdir(parents=True, exist_ok=True)

    if destination.exists() or destination.is_symlink():
        if is_link(destination):
            return "exists"
        return "skipped"

    if os.name == "nt":
        result = subprocess.run(
            ["cmd", "/c", "mklink", "/J", str(destination), str(source)],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            return "linked"
    else:
        try:
            os.symlink(source, destination, target_is_directory=True)
            return "linked"
        except OSError:
            pass

    shutil.copytree(source, destination)
    return "copied"


def remove_link(path: Path) -> str:
    if not (path.exists() or path.is_symlink()):
        return "absent"
    if not is_link(path) or not points_to(path, SOURCE / path.name):
        return "skipped"
    # rmdir removes the link or junction only; it never deletes the target.
    os.rmdir(path)
    return "removed"


def link(target_names: list[str]) -> int:
    if not skill_dirs():
        print(f"No skills found in {SOURCE.relative_to(ROOT)}")
        return 1

    failures = 0
    for name in target_names:
        destination_root = TARGETS[name]
        print(f"{name}: {destination_root.relative_to(ROOT)}")
        for skill in skill_dirs():
            result = make_link(skill, destination_root / skill.name)
            if result == "skipped":
                failures += 1
                print(f"  ! {skill.name}: a real directory already exists; left untouched")
            else:
                print(f"  {result}: {skill.name}")

    if failures:
        print("\nSome entries were not linked. Remove them manually if they are stale.")
    return 0


def unlink(target_names: list[str]) -> int:
    for name in target_names:
        destination_root = TARGETS[name]
        print(f"{name}: {destination_root.relative_to(ROOT)}")
        if not destination_root.exists():
            print("  absent")
            continue
        for child in sorted(destination_root.iterdir()):
            result = remove_link(child)
            print(f"  {result}: {child.name}")
        try:
            destination_root.rmdir()
        except OSError:
            pass
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--link", metavar="TARGET", help="agents, claude, opencode, or all")
    group.add_argument("--unlink", metavar="TARGET", help="agents, claude, opencode, or all")
    group.add_argument("--list", action="store_true", help="list skills and targets")
    parser.add_argument(
        "--all",
        action="store_true",
        help="link the two portable targets: agents and claude",
    )
    args = parser.parse_args()

    if args.list or (not args.link and not args.unlink and not args.all):
        print(f"source: {SOURCE.relative_to(ROOT)}")
        for skill in skill_dirs():
            print(f"  {skill.name}")
        print("targets:")
        for name, path in TARGETS.items():
            print(f"  {name}: {path.relative_to(ROOT)}")
        return 0

    if args.all:
        return link(["agents", "claude"])

    requested = args.link or args.unlink
    assert requested is not None
    names = list(TARGETS) if requested == "all" else [requested]
    unknown = [name for name in names if name not in TARGETS]
    if unknown:
        print(f"Unknown target: {', '.join(unknown)}")
        return 1

    return link(names) if args.link else unlink(names)


if __name__ == "__main__":
    sys.exit(main())
