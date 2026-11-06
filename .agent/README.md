# FlexCore Agent Harness

The tool-neutral agent harness for this repository. Everything an agent needs
lives here: context digests, on-demand skills, and role prompts. No file in this
directory depends on a specific vendor or client.

## Layout

```text
AGENTS.md              Instructions for any agent (open AGENTS.md format)
.agent/                This harness
  README.md            This file
  CONTEXT.md           One-page product and architecture digest
  DECISIONS.md         Open decisions and recommended defaults
  WRITING.md           Evidence, citation, units, terminology rules
  PROVENANCE.md        Sources used and the license audit
  skills/              On-demand procedures (Agent Skills format)
  roles/               Portable role prompts (plain Markdown)
docs/plan/             Authoritative product and architecture report
docs/foundations/      Canonical specification (read-only)
tools/                 Verification and harness setup scripts
```

The harness works with **zero tool integration**: every agent can read
`AGENTS.md`, open a skill file, and follow a role prompt. Native discovery paths
are a convenience, not a requirement.

## How any agent uses it

1. Read `AGENTS.md` at the repository root. It defines the source-of-truth
   order, the non-negotiable rules, and the verification commands.
2. For a task covered by a skill, read `.agent/skills/<id>/SKILL.md` before
   starting. Skills are procedures, not background text; load only the one you
   need.
3. For a review or a specialised pass, read the relevant file in
   `.agent/roles/` and follow it for the rest of the task.

No agent-specific file is required for these three steps.

## Skills

An Agent Skill is a directory containing `SKILL.md` with YAML frontmatter
(`name`, `description`) and Markdown instructions. The format is the open
[Agent Skills](https://agentskills.io/specification) specification.

| ID | Use when |
|---|---|
| `flexcore-context` | Starting a non-trivial task; scope, release, units, or authority questions |
| `docs-verification` | After any change under `docs/`; proving a documentation task complete |
| `adr-authoring` | Recording a decision, resolving an open gate, or writing an ADR |
| `evidence-first-research` | Adding any external fact, version, clause, or capability claim |
| `rust-module-boundaries` | Adding a crate, port, adapter, domain type, or reviewing layering |

Supporting files live beside each `SKILL.md` and are read only when the skill
points to them.

## Roles

A role is a plain Markdown prompt with no vendor frontmatter. Any agent can be
told to follow one; tools with subagents can wrap it.

| Role | Access | Use when |
|---|---|---|
| `architect` | Read-only | Reviewing module boundaries, trade-offs, or a design decision |
| `docs-guardian` | Docs only | Editing the report or agent context; enforcing citation rules |
| `rust-engineer` | Write | Implementing or reviewing Rust for the control plane, edge, or plugin host |
| `numerical-engineer` | Write | Working on power flow, optimisation, forecasting, or ML workers |
| `security-reviewer` | Read-only | Reviewing authorization, tenancy, command paths, edge exposure, and OT boundaries |

## Why `.agent` and not `.agents`

`.agent/` is this repository's chosen, human-readable home for the harness. It
is not a vendor path.

`.agents/` (plural) is a different, cross-tool discovery convention: Gemini CLI
and OpenCode both read `.agents/skills/`. Claude Code does not read either
`.agents/` or `.agent/`. That is why native discovery needs a link, and why the
link target is always this directory.

## Native discovery (optional)

Verified against vendor documentation on **7 November 2026**. Run
`python tools/setup_agent.py --all` to create the discovery links. The script
uses a directory symlink on POSIX and a junction on Windows, so the skill files
stay in one place and cannot drift.

| Tool | Instruction file it reads | Skill discovery it supports |
|---|---|---|
| Claude Code | `AGENTS.md` (v2.1.277+); `CLAUDE.md` takes precedence, and this repo's `CLAUDE.md` imports `AGENTS.md` | `.claude/skills/<id>/SKILL.md`; symlinked skill folders are supported |
| OpenCode | `AGENTS.md` | `.opencode/skills`, `.claude/skills`, `.agents/skills` |
| Gemini CLI | `GEMINI.md` by default; this repo sets `context.fileName` to `AGENTS.md` in `.gemini/settings.json` | `.gemini/skills`, `.agents/skills` |
| Cursor | `AGENTS.md` (native, including nested files) | Not wired to this harness |
| GitHub Copilot | Repository-wide: `.github/copilot-instructions.md`. AI agents and code review also read the nearest `AGENTS.md` in the directory tree | Not wired to this harness |
| Codex | `AGENTS.md` | Not verified |

Targets created by the script:

```text
.agents/skills/<id>   -> ../../.agent/skills/<id>    (Gemini CLI, OpenCode)
.claude/skills/<id>   -> ../../.agent/skills/<id>    (Claude Code)
.opencode/skills/<id> -> ../../.agent/skills/<id>    (OpenCode only; optional)
```

The generated link directories are git-ignored. They contain links, never
copies.

## Adding a skill

1. Create `.agent/skills/<id>/SKILL.md`.
2. Use lowercase kebab-case for `<id>` and make the frontmatter `name` match.
3. Write a `description` that says **what it does and when to use it**; that is
   the only text the model sees before loading the skill.
4. Keep the body under roughly 500 lines. Move detail into `references/`.
5. Add a row to the skills table above.
6. Run `python tools/validate_agent.py` and
   `python tools/setup_agent.py --all` if you use native discovery.

## Adding a role

1. Create `.agent/roles/<name>.md`.
2. Start with `# Role: <title>`, then **Access** and **Use this role when**.
3. Keep it prompt-shaped: what to read, what to check, how to answer.
4. Do not add vendor frontmatter. If a tool needs metadata, add it in that
   tool's own adapter, not here.
5. Add a row to the roles table and run `python tools/validate_agent.py`.

## Source of truth

The harness never overrides the product baseline. In order of authority:

1. `docs/foundations/` — canonical specification (read-only).
2. `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`.
3. `AGENTS.md`, then this directory.

If a skill or role conflicts with the report, the report wins and the skill is
the defect.

## Provenance and license

Everything here was written for this repository. No third-party skill body,
prompt, or script was copied. The license audit and the sources used to verify
the formats are in `PROVENANCE.md`.
