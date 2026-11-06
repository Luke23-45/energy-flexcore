# Open Decisions

Status of every foundational decision that the report deliberately leaves to
Neryva. Source: report §25.2. Nothing here is decided unless the status says so.

## Rule

An agent may **use** a recommended default for simulation work, but must not
describe it as decided, approved, or committed. When a decision is made, update
this file and write an ADR (see the `adr-authoring` skill).

## Gates

| ID | Decision | Why it is deferred | Gate | Owner | Status |
|---|---|---|---|---|---|
| D1 | Target jurisdiction and market | Drives interconnection, market adapter, data residency, and liability | Phase 0 | Product + legal | OPEN |
| D2 | First portfolio and service | Determines asset mix, baseline method, and acceptance criteria | Phase 0 | Product | OPEN |
| D3 | Network class and model fidelity | Balanced radial, unbalanced three-phase, and meshed transmission need different models and solvers | Phase 0 | Power systems | OPEN |
| D4 | License and dependency audit | Apache-2.0 compatibility of protocol stacks, solvers, models, and datasets | Before v0.1.0 | Legal + engineering | OPEN |
| D5 | First protocol and asset mix | Selects the pilot adapter and conformance tests | Before v0.4.0 | Product + integration | OPEN |
| D6 | Cyber and OT boundary review | Site-specific; not derivable from the architecture alone | Before any read-only pilot | Security | OPEN |

## Recommended defaults (not approved)

These are engineering defaults for planning only. They are wrong the moment a
customer, jurisdiction, or pilot says otherwise.

| Area | Default | Revisit when |
|---|---|---|
| Network class for 0.x | Balanced radial distribution feeders | A pilot requires unbalanced or meshed modelling |
| Primary power-flow provider | pandapower | Model fidelity or performance requires OpenDSS/GridLAB-D |
| Unbalanced three-phase | Enters the track from v0.4.0 | A pilot needs it earlier |
| Meshed transmission | Out of scope | A transmission-level use case is approved |
| Core license | Apache-2.0 | D4 audit finds incompatible dependencies |
| Implementation language | Rust core, Python workers, TypeScript UI | Team cannot sustain Rust depth (report §25.1) |

## Decision record

| Date | ID | Decision | ADR |
|---|---|---|---|
| — | — | No decisions recorded yet | — |

## Related

- Report §25 — recommended baseline and deferred decisions
- Report §22.1 — the initial ADR list (ADR-001 … ADR-012)
- `.agent/WRITING.md` — how to write the rationale
