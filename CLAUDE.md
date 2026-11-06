# Claude Code instructions

@AGENTS.md

## Claude Code specifics

`AGENTS.md` is the single source of instruction. This file imports it so that
older Claude Code sessions, which cannot read `AGENTS.md` directly, still get
the same rules. The import is not loaded twice.

- Skills live in `.agent/skills/`. Run
  `python tools/setup_agent.py --link claude` to expose them at
  `.claude/skills/` for native `/skill-name` invocation.
- Roles live in `.agent/roles/` as plain prompts. Read the relevant file when a
  task calls for a specialised review.
- Do not add Claude-only rules here that contradict `AGENTS.md`.
