# Neryva FlexCore — Agent Instructions

This file is the entry point for every coding agent working in this repository.
It follows the open [AGENTS.md](https://agents.md) format, which Cursor, Codex,
OpenCode, Claude Code (v2.1.277+) and GitHub Copilot agents read natively. Two
thin adapters cover clients that do not: `CLAUDE.md` imports this file for older
Claude Code sessions, and `.gemini/settings.json` points Gemini CLI at it. Any
other agent can simply be told to read `AGENTS.md`; the harness assumes no tool.
The verified per-client matrix is in `.agent/README.md`.

## 1. What this repository is

`flexenergy` holds the **design baseline for Neryva FlexCore**, an open, modular
Virtual Power Plant (VPP) orchestration and distribution-aware DER coordination
platform.

It currently contains **documentation only**. There is no application code, no
build system, and no test suite yet. Do not invent them, and do not scaffold
crates "to be ready" — the report explicitly says to introduce crates only when
they have real code, clear ownership, and a stable dependency direction.

## 2. Source of truth, in order

| Rank | Document | Authority |
|---|---|---|
| 1 | `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.pdf` / `.docx` | Canonical system-design baseline. Mandatory conventions govern. |
| 2 | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` | Product definition, architecture, delivery plan, open decisions. |
| 3 | `.agent/*.md` | Agent-facing digests derived from 1 and 2. Never authoritative on their own. |
| 4 | `README.md` | Human-facing summary. |

If 2, 3, or 4 conflicts with 1, **the foundations document wins** unless a
deviation is recorded as an approved design decision.

## 3. Non-negotiable rules

1. **No hallucinated facts.** Every external claim (protocol version, standard
   clause, solver capability, release date, API behaviour) must be traceable to
   a primary source or to the foundations document. If you cannot verify it,
   label it explicitly as an assumption or a recommendation.
2. **No safety or certification claims.** FlexCore is not certified, not a
   safety case, and not permission for live grid control. Never write text that
   implies otherwise.
3. **No protection-grade claims.** FlexCore is a supervisory orchestration
   layer. It does not perform primary frequency response, synthetic inertia,
   relay protection, inverter inner-loop control, or sub-cycle control.
4. **Never conflate solver classes.** HiGHS is LP/QP/MIP, not SOCP. Clarabel is
   convex conic, not MISOCP. AC power flow is not AC-OPF. IPOPT is local, not a
   global-optimality guarantee.
5. **Simulation first.** The entire 0.x line is simulation-only. Field write
   authority begins at v1.0.0 with operator approval. Bounded automation is
   post-1.0, per deployment, site, and asset class.
6. **Minimal, surgical edits.** This is a reviewed baseline, not a draft to
   rewrite. Preserve structure, numbering, and citation IDs.
7. **Verify before you claim done.** Run the checks in §6 and report their
   actual output. Do not say "verified" without running anything.

## 4. Product state you must not contradict

**Release ladder**

| Release | Meaning | Control status |
|---|---|---|
| v0.1.0 | First public release: canonical models, simulator, telemetry, registry, portfolio views, replay | Simulation only |
| v0.2.0 | Flexibility envelopes, rolling-horizon scheduling | Simulation only |
| v0.3.0 | Validation, feasibility, command lifecycle, approvals, audit | Simulation and shadow |
| v0.4.0 | First site integration (OpenEMS), protocol adapters | Read-only / test-lab by default |
| v0.5.0 | Plugin SDK, WASM host | Simulation / shadow |
| v1.0.0 | Pilot-ready | Operator-approved control by default |
| v1.x / v2.x | Advanced coordination / grid intelligence | Per-feature qualification |

**Canonical units:** MW, MVAr, MVA, MWh. `P > 0` means export at the point of
connection. Reactive-power sign is **declared per grid model and device
profile**, never assumed. Voltage is kV at the nominal base, per-unit in solver
representation. Current is A or per-unit on an explicit base. SOC is a fraction
in [0,1] at the API boundary.

**Telemetry quality states (exactly one per value):** `GOOD`, `SUSPECT`, `BAD`,
`MISSING`, `ESTIMATED`, `STALE`.

**Command lifecycle:** `PROPOSED → VALIDATED → FEASIBLE → APPROVED → ISSUED →
ACKNOWLEDGED → TRACKING → COMPLETED`, plus `REJECTED`, `FAILED`, `EXPIRED`,
`CANCELLED`, `SUPERSEDED`, `FALLBACK`. Validation, feasibility, and
authorization are distinct gates. Acknowledgement is not physical delivery.

**Control-authority modes:** Simulation, Read-only/shadow, Operator-approved,
Bounded automation. Mode is a deployment and per-site setting; a plugin can
never select it.

## 5. Repository map

```text
docs/foundations/   Canonical specification (PDF + DOCX). Do not edit.
docs/plan/          Product and architecture report (Markdown, authoritative).
.agent/             Tool-neutral agent harness.
  README.md         Harness layout and per-tool wiring.
  CONTEXT.md        Product and architecture digest.
  DECISIONS.md      Open decisions and recommended defaults.
  WRITING.md        Evidence, citation, units, terminology rules.
  PROVENANCE.md     Sources used and the license audit.
  skills/           On-demand procedures in Agent Skills format.
  roles/            Portable role prompts (architect, docs, Rust, Python, security).
tools/              Verification and harness setup scripts.
CLAUDE.md           Claude Code adapter. Imports AGENTS.md.
.gemini/            Gemini CLI adapter. Reads AGENTS.md as its context file.
.github/            GitHub Copilot instructions.
```

The harness is vendor-neutral. Skills and roles are plain files that any agent
can read; native discovery links are optional and created by
`tools/setup_agent.py`. Never commit a vendor-locked configuration directory as
the only way to use the harness.

**Using the harness.** For a task covered by a skill, read
`.agent/skills/<id>/SKILL.md` before starting. For a specialised review, follow
the relevant file in `.agent/roles/`. Load the one the task needs, not the whole
library.

## 6. Commands

Requires Python 3.11+ (standard library only).

```sh
# Structural, citation, table, fence, and TOC checks on the report
python tools/verify_docs.py

# Agent Skills and role validation
python tools/validate_agent.py

# Optional: expose .agent/skills at native discovery paths
python tools/setup_agent.py --all
```

Run both after any documentation change. A task is not complete while either
reports a failure.

## 7. Documentation conventions

- One canonical report; do not fork copies. Version bumps happen in the same
  file with an updated header and revision note.
- Section references use `§N` or `§N.M` and must resolve to a real heading.
- External references use stable IDs `[R1]`…`[R29]`, defined once in §27 and
  cited at least once in the body. New sources get the next free ID.
- Research cut-off is **7 November 2026**. If you add a source after that date,
  update the cut-off and say what changed.
- Use "license" (US spelling) in this repository.
- Prefer tables over prose for enumerations; keep pipe counts consistent.
- Do not add a claim to the report that is only supported by another summary
  document — go to the primary source.

Full rules: `.agent/WRITING.md`.

## 8. Engineering conventions (for when code exists)

The target stack is **Rust** for the control plane, edge agent, plugin host, and
CLI; **Python** for numerical workers; **TypeScript** for the operator console.
These choices are provisional (report §25.1).

- Architecture: DDD + hexagonal (ports and adapters) + modular monolith.
- Dependency rule: domain depends on nothing infrastructure; adapters depend on
  ports; apps are composition roots.
- Core crates follow the layout in report §8; do not create empty crates.
- Use `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` before
  claiming a Rust change is complete.
- No arbitrary Rust `dylib` plugin loading. Third-party extensions use WASM/WIT
  or out-of-process gRPC workers.
- Plugins may never bypass authorization, tenant isolation, audit, command
  lifecycle, or final dispatch validation.

## 9. Working agreement

1. **Read before writing.** Load `.agent/CONTEXT.md` and, for document work,
   the report section you are changing.
2. **Plan first** for anything beyond a one-line fix. State the intended change
   and the files touched.
3. **Make the smallest correct change.** No drive-by rewrites, no reformatting
   of untouched sections.
4. **Verify.** Run the commands in §6 and quote the result.
5. **Report honestly.** If something is unverified, say so. If you removed or
   changed a claim, say which and why.

## 10. Commits

Conventional Commits, matching the existing history:

```text
docs: add v1.1 product and architecture report for Neryva FlexCore
```

Use `docs:`, `feat:`, `fix:`, `chore:`, `refactor:`, `test:`. Imperative mood,
72-character subject, body only when the "why" is not obvious.

## 11. Do not

- Do not edit `docs/foundations/*` — treat it as read-only reference.
- Do not rename or renumber sections or reference IDs.
- Do not add product promises (mobile app, hard real-time, certified protocol
  conformance, autonomous field dispatch) that the report excludes.
- Do not copy third-party agent skills into this repository without checking
  their license first. See `.agent/PROVENANCE.md`.
- Do not commit generated artifacts, virtual environments, or `__pycache__`.
