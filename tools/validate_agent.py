#!/usr/bin/env python3
"""Validate the FlexCore agent harness.

Checks Agent Skills against the public format and role prompts against this
repository's conventions. Standard library only.

    python tools/validate_agent.py

References:
    https://agentskills.io/specification
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
AGENT_DIR = ROOT / ".agent"
SKILLS_DIR = AGENT_DIR / "skills"
ROLES_DIR = AGENT_DIR / "roles"
README = AGENT_DIR / "README.md"

NAME_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")

errors: list[str] = []
warnings: list[str] = []


def parse_frontmatter(text: str) -> dict[str, str]:
    """Parse flat top-level scalar keys of a YAML frontmatter block."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}
    try:
        end = lines.index("---", 1)
    except ValueError:
        return {}

    data: dict[str, str] = {}
    for line in lines[1:end]:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line[0].isspace() or line.startswith("-"):
            continue
        key, sep, value = line.partition(":")
        if not sep:
            continue
        data[key.strip()] = value.strip().strip('"').strip("'")
    return data


def strip_frontmatter(text: str) -> str:
    lines = text.splitlines()
    if lines and lines[0].strip() == "---":
        try:
            end = lines.index("---", 1)
        except ValueError:
            return text
        return "\n".join(lines[end + 1 :])
    return text


def check_skill(path: Path) -> None:
    skill_id = path.parent.name
    label = path.relative_to(ROOT).as_posix()
    text = path.read_text(encoding="utf-8-sig")
    meta = parse_frontmatter(text)

    if not meta:
        errors.append(f"{label}: missing YAML frontmatter")
        return

    name = meta.get("name", "")
    description = meta.get("description", "")

    if not name:
        errors.append(f"{label}: missing required 'name'")
    else:
        if len(name) > 64:
            errors.append(f"{label}: name is {len(name)} characters (max 64)")
        if not NAME_RE.match(name):
            errors.append(
                f"{label}: name {name!r} must be lowercase alphanumeric with single hyphens"
            )
        if name != skill_id:
            errors.append(
                f"{label}: name {name!r} does not match directory {skill_id!r}"
            )

    if not description:
        errors.append(f"{label}: missing required 'description'")
    elif len(description) > 1024:
        errors.append(f"{label}: description is {len(description)} characters (max 1024)")
    elif len(description) < 40:
        warnings.append(f"{label}: description is short; the model uses it to decide relevance")

    compatibility = meta.get("compatibility", "")
    if compatibility and len(compatibility) > 500:
        errors.append(f"{label}: compatibility exceeds 500 characters")

    body_lines = len(strip_frontmatter(text).splitlines())
    if body_lines > 500:
        warnings.append(f"{label}: body is {body_lines} lines; consider moving detail to references/")

    if "version:" not in text:
        warnings.append(f"{label}: no metadata.version recorded")

    if not (path.parent / "references").exists() and "references/" in text:
        warnings.append(f"{label}: mentions references/ but the directory does not exist")


def check_role(path: Path) -> None:
    label = path.relative_to(ROOT).as_posix()
    text = path.read_text(encoding="utf-8-sig")
    lines = text.splitlines()

    if not lines:
        errors.append(f"{label}: empty file")
        return

    if lines[0].startswith("---"):
        warnings.append(f"{label}: has YAML frontmatter; roles should stay vendor-neutral")

    if not lines[0].startswith("# Role: "):
        errors.append(f"{label}: first line must be '# Role: <title>'")

    for required in ("**Access:**", "**Use this role when:**"):
        if required not in text:
            errors.append(f"{label}: missing {required}")

    if len(lines) > 400:
        warnings.append(f"{label}: {len(lines)} lines; keep role prompts focused")


def check_readme(skills: list[str], roles: list[str]) -> None:
    if not README.exists():
        errors.append(".agent/README.md: missing")
        return
    text = README.read_text(encoding="utf-8-sig")
    for skill_id in skills:
        if f"`{skill_id}`" not in text:
            errors.append(f".agent/README.md: skill {skill_id!r} is not listed")
    for role in roles:
        if f"`{role}`" not in text:
            errors.append(f".agent/README.md: role {role!r} is not listed")


def main() -> int:
    if not SKILLS_DIR.exists():
        errors.append(f"missing skills directory: {SKILLS_DIR.relative_to(ROOT)}")
        skill_paths: list[Path] = []
    else:
        skill_paths = sorted(SKILLS_DIR.glob("*/SKILL.md"))

    if not ROLES_DIR.exists():
        errors.append(f"missing roles directory: {ROLES_DIR.relative_to(ROOT)}")
        role_paths: list[Path] = []
    else:
        role_paths = sorted(ROLES_DIR.glob("*.md"))

    if not skill_paths:
        errors.append("no .agent/skills/*/SKILL.md found")
    if not role_paths:
        errors.append("no .agent/roles/*.md found")

    for path in skill_paths:
        check_skill(path)
    for path in role_paths:
        check_role(path)

    check_readme(
        [path.parent.name for path in skill_paths],
        [path.stem for path in role_paths],
    )

    print(f"skills: {len(skill_paths)}  roles: {len(role_paths)}")
    for warning in warnings:
        print(f"warning: {warning}")

    if errors:
        print(f"\nFAIL ({len(errors)}):")
        for error in errors:
            print(f"  - {error}")
        return 1

    print("OK: skills, roles, and README index are valid")
    return 0


if __name__ == "__main__":
    sys.exit(main())
