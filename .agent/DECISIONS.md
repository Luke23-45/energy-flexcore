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
| D1 | Target jurisdiction and market | Drives interconnection, market adapter, data residency, and liability | Phase 0 | Product + legal | SCHEDULED (Phase 0, 2026-11-07: owner + venue recorded in `ledger/phase-00-product-freeze/03_decision_records.md` §1; named person + date assigned at the Stage 0 gate meeting — P0.28) |
| D2 | First portfolio and service | Determines asset mix, baseline method, and acceptance criteria | Phase 0 | Product | SCHEDULED (Phase 0: owner + venue recorded, `03_decision_records.md` §2; first-portfolio sheet in `02_first_buyer_use_cases.md`; baseline-method question OPEN pending D1 input) |
| D3 | Network class and model fidelity | Balanced radial, unbalanced three-phase, and meshed transmission need different models and solvers | Phase 0 | Power systems | SCHEDULED (Phase 0: owner + venue recorded, `03_decision_records.md` §3; 0.x balanced-radial default is a RECOMMENDATION, not a decision) |
| D4 | License and dependency audit | Apache-2.0 compatibility of protocol stacks, solvers, models, and datasets | Before v0.1.0 | Legal + engineering | SCHEDULED (Phase 0: owner + venue + gate recorded, `03_decision_records.md` §4; core license OPEN, Apache-2.0 recommended only) |
| D5 | First protocol and asset mix | Selects the pilot adapter and conformance tests | Before v0.4.0 | Product + integration | SCHEDULED (Phase 0: owner + venue + gate recorded, `03_decision_records.md` §5; OpenEMS priority is not a decision) |
| D6 | Cyber and OT boundary review | Site-specific; not derivable from the architecture alone | Before any read-only pilot | Security | SCHEDULED (Phase 0: owner + venue + gate recorded, `03_decision_records.md` §6) |

Gate-table skeleton with named-owner/date blanks and the escalation rule:
`ledger/phase-00-product-freeze/03_decision_records.md` §7 (P0.28). Named
owners and target dates are assigned at the Stage 0 gate meeting; the ledger
forbids inventing them.

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
