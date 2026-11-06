# Agent Setup Provenance

Record of how this repository's agent setup was researched, what was taken from
where, and what was deliberately not used. Keep this file current when adding
skills, roles, or adapters.

Setup date: **7 November 2026**.

## Standards and formats used

| Source | What it is | How it was used |
|---|---|---|
| [agents.md](https://agents.md/) | Open instruction-file format, stewarded by the Agentic AI Foundation under the Linux Foundation | Shape and scope of `AGENTS.md`. No text copied. |
| [Agent Skills specification](https://agentskills.io/specification) | Open `SKILL.md` format: frontmatter fields, directory layout, progressive disclosure | Structure of `.agent/skills/*`. Skills written from scratch. |
| [agentskills/agentskills](https://github.com/agentskills/agentskills) | Apache-2.0 reference repository for the specification and the `skills-ref` validator | Cloned to a temporary directory to validate the skills. Nothing vendored. |
| [Claude Code documentation](https://code.claude.com/docs/en/memory) | `AGENTS.md` precedence, `CLAUDE.md` imports, `.claude/skills/` discovery | Adapter design and skill discovery path. |
| [Gemini CLI documentation](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/skills.md) | `context.fileName`, `.gemini/skills/`, `.agents/skills/` alias | Adapter design and skill discovery path. |
| [OpenCode documentation](https://opencode.ai/v2/docs/skills) | Skill discovery paths including `.agents/skills` and `.claude/skills` | Confirmed the cross-tool path; no OpenCode-only files are committed. |
| [GitHub Copilot documentation](https://docs.github.com/en/copilot/concepts/response-customization) | Repository-wide and agent instruction files | `.github/copilot-instructions.md` adapter. |
| [Cursor documentation](https://cursor.com/docs/context/rules) | Native `AGENTS.md` support | No Cursor-specific adapter needed. |

## Inspected and rejected

| Source | Finding | Decision |
|---|---|---|
| [anthropics/skills](https://github.com/anthropics/skills) | Popular reference skills (`docx`, `pdf`, `xlsx`, `mcp-builder`, and others), but each skill carries a proprietary `LICENSE.txt`: "users may not extract these materials ... reproduce or copy ... create derivative works ... distribute". The repository itself has no open-source license. | **Not copied and not imported.** Using them from the Anthropic service is one thing; vendoring them into this repository would breach the license. |

If Neryva later wants document-format skills, write them locally or obtain
written permission. Do not copy the Anthropic implementations.

## What is original here

Everything under `.agent/`, `tools/`, `AGENTS.md`, `CLAUDE.md`,
`.github/copilot-instructions.md`, and `.gemini/settings.json` was written for
this repository. No third-party skill body, script, or prompt was copied.

The architecture ideas attributed to DeepSeek Harness in the product report are
recorded in the report's references (`[R2]`) and are not part of this setup.

## License status of this setup

The repository has **no license file yet**, and the core license is an open
decision (`D4` in `.agent/DECISIONS.md`). The report recommends Apache-2.0 for
core software. Until that decision is made, treat these files as internal
project material and do not distribute them separately.

## Verification status

| Check | Result |
|---|---|
| `python tools/verify_docs.py` | Report structure, references, and TOC pass |
| `python tools/validate_agent.py` | Skill and role frontmatter pass |
| Official `skills-ref validate` (Apache-2.0, temporary install) | All skills valid |
| Native discovery in a live client | Not re-run after the move to `.agent/`; the links are created by `tools/setup_agent.py` |

The last row is deliberate: this file records what was actually executed, not
what is expected to work.

## Updating

1. Edit `AGENTS.md`, `.agent/CONTEXT.md`, a skill, or a role.
2. Run:
   ```sh
   python tools/verify_docs.py
   python tools/validate_agent.py
   ```
3. Bump the `metadata.version` in the changed skill's frontmatter.
4. If you add a third-party source, record it in this file **with its license**
   before committing anything derived from it.
5. If you add a skill, run `python tools/setup_agent.py --all` to refresh the
   discovery links.

Optional: the official validator can be installed from the Apache-2.0
`agentskills/agentskills` repository (`skills-ref`). It is not a dependency of
this repository; `tools/validate_agent.py` performs the local checks with the
standard library only.

## Configuration surface

| File | Purpose |
|---|---|
| `AGENTS.md` | Cross-agent instructions (the portable standard). |
| `.agent/README.md` | Harness layout and per-tool wiring. |
| `.agent/CONTEXT.md` | Product and architecture digest. |
| `.agent/DECISIONS.md` | Open gates and recommended defaults. |
| `.agent/WRITING.md` | Evidence, citation, units, and terminology rules. |
| `.agent/skills/*/SKILL.md` | On-demand procedures. |
| `.agent/roles/*.md` | Portable role prompts. |
| `CLAUDE.md` | Claude Code compatibility: imports `AGENTS.md`. |
| `.gemini/settings.json` | Tells Gemini CLI to read `AGENTS.md` as its context file. |
| `.github/copilot-instructions.md` | Repository-wide Copilot instructions. |
| `tools/verify_docs.py` | Report structure, citation, table, fence, and TOC checks. |
| `tools/validate_agent.py` | Skill and role validation. |
| `tools/setup_agent.py` | Creates native skill-discovery links. |
