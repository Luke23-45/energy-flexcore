---
name: flexcore-context
description: Loads the FlexCore product, scope, and architecture context before working in this repository. Use when starting a task, when the request touches product scope, release definitions, control authority, domain terminology, or when you need to know which document is authoritative.
metadata:
  author: neryva
  version: "1.0"
---

# FlexCore context

Use this skill at the start of any non-trivial task in this repository.

## 1. Read the authoritative sources

In order:

1. `AGENTS.md` — repository rules and product state.
2. `.agent/CONTEXT.md` — one-page platform digest.
3. The report section that covers your task, in
   `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`.

Do not rely on this skill alone for a factual claim. It points; it does not
prove.

## 2. Locate the task in the report

| Task type | Sections |
|---|---|
| Product scope, audience, release definition | §0–§2 |
| System architecture, modules, plugins | §3–§5 |
| Software patterns, Rust stack, workspace | §6–§9 |
| Domain model, units, telemetry quality | §10 |
| Flexibility, scheduling, optimization, feasibility | §11–§12 |
| Research modules (DMPC, GNN, MARL) | §13 |
| Command lifecycle, APIs, events | §14–§15 |
| Storage, tenancy, security, edge, simulation | §16–§19 |
| Operator UI, deployment, governance | §20–§22 |
| Roadmap, first build, decisions, glossary | §23–§28 |

## 3. Hold these facts constant

- The 0.x line is simulation-only. Field writes start at v1.0.0 with
  operator-approved control.
- Canonical units are MW, MVAr, MVA, MWh; `P > 0` exports; reactive-power sign
  is declared per model, never assumed.
- Telemetry quality is exactly one of GOOD, SUSPECT, BAD, MISSING, ESTIMATED,
  STALE.
- `SUPERSEDED` is not `CANCELLED`. Validation, feasibility, and authorization
  are different gates.
- Acknowledgement is not physical delivery.
- Control mode is a deployment setting; no plugin can select or raise it.

## 4. Common mistakes to avoid

- Treating the foundations PDF and the report as interchangeable. The
  foundations document's mandatory conventions win.
- Describing v0.1.0 as delivering scheduling or dispatch. It delivers the
  canonical foundation, simulator, telemetry, registry, portfolio views, replay.
- Adding a technology choice as if it were decided. Technology is provisional
  (report §25.1).
- Answering an open product question from `.agent/DECISIONS.md` as though it
  were settled.

## 5. When the task is about a decision

Check `.agent/DECISIONS.md`. If the gate is still OPEN, say so and record
options in an ADR rather than silently choosing.
