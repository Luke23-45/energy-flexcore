# Neryva FlexCore

Design baseline for **FlexCore**, an open, modular Virtual Power Plant (VPP)
orchestration and distribution-aware DER coordination platform.

This repository currently contains the product definition and architecture
baseline. It does not yet contain application code.

## Documents

| Path | Contents |
|---|---|
| `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.pdf` / `.docx` | Canonical system-design specification |
| `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` | Product definition, architecture, delivery plan, open decisions |
| `.agent/` | Tool-neutral agent harness: context, skills, roles, provenance |

Read the report first. Section 0 is the executive decision; §2 is scope by
release; §25.2 lists the decisions that are still open.

## Status

- Version: **1.1** (research cut-off 7 November 2026)
- Status: proposed baseline for design review
- Not a certification, not a safety case, and not permission for live grid control
- The 0.x line is simulation-only; field write authority begins at v1.0.0 with
  operator-approved control

## Agent harness

The harness is tool-neutral. `AGENTS.md` is the single instruction file; the
files under `.agent/` are read on demand. None of it requires a specific client.

| File | Purpose |
|---|---|
| `AGENTS.md` | Instructions for any coding agent (open AGENTS.md format) |
| `.agent/README.md` | Harness layout and per-tool wiring |
| `.agent/CONTEXT.md` | Product and architecture digest |
| `.agent/DECISIONS.md` | Open decisions and recommended defaults |
| `.agent/WRITING.md` | Evidence, citation, units, and terminology rules |
| `.agent/skills/` | On-demand procedures in Agent Skills format |
| `.agent/roles/` | Portable role prompts: architect, docs, Rust, numerical, security |
| `.agent/PROVENANCE.md` | Sources used and the license audit |

Thin adapters, only where a tool does not read `AGENTS.md`:

| File | Tool |
|---|---|
| `CLAUDE.md` | Claude Code (imports `AGENTS.md`; never loads it twice) |
| `.gemini/settings.json` | Gemini CLI (`context.fileName` set to `AGENTS.md`) |
| `.github/copilot-instructions.md` | GitHub Copilot repository-wide instructions |

Cursor, OpenCode, Codex, and Claude Code v2.1.277+ read `AGENTS.md` directly and
need no adapter.

## Checks

Requires Python 3.11+; no third-party packages.

```sh
python tools/verify_docs.py      # headings, § refs, tables, fences, citations, TOC
python tools/validate_agent.py   # skills, roles, and README index
python tools/setup_agent.py --all  # optional native skill-discovery links
```

## License

Not yet decided. The report recommends Apache-2.0 for core software; the
license and dependency audit is an open gate (D4).
