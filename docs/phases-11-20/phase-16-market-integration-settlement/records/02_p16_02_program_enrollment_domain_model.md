# P16.02 — Program/enrollment domain model: Program, Contract, Enrollment, QualificationEvidence, ParticipationWindow with lifecycle states

| Field | Value |
|---|---|
| Subtask | P16.02 (Program/enrollment domain model — Program, contract, enrollment, qualification-evidence, per-asset participation-window types with lifecycle states; the GE Program Manager analog [S-41]) |
| Wave / agent | Phase 16 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the domain vocabulary is written, never compiled, never executed. **No program is enrolled, no contract is recorded, no qualification is claimed, and no market is connected anywhere** (ADR-011; conventions §7.3). The stores are empty everywhere. |
| Sources | [S-41] (GE Vernova GridOS DERMS — program management analog), [S-43] (Uplight VPP program management), [S-48] (CPower curtailment plans); `crates/flexcore-domain/src/` model patterns (the P15.15 `interconnection_records` store pattern mirrored) |
| Files touched | `crates/flexcore-domain/src/market_program_model.rs` (NEW); `crates/flexcore-domain/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `MarketProgram` | Program record: declared `jurisdiction` (recorded data — ADR-011), closed lifecycle vocabulary `DECLARED`/`ACTIVE`/`SUSPENDED`/`RETIRED` with a closed transition table (RETIRED terminal), the program's OWN `BaselineMethodDeclaration` (LR-073 — the typed carrier of the program's method choice; absent is recorded absent, never defaulted), declared key/value rules (BTreeMap — deterministic), supersede link, P11 audit-chain LINK, recorded timestamp + recorder |
| `MarketContract` | Contract record: external `contract_ref`, closed vocabulary `RECORDED`/`ACTIVE`/`TERMINATED` (TERMINATED terminal), declared key/value terms (revenue-share splits enter as recorded data when P16.13 lands — never interpreted here), supersede link, audit-chain LINK |
| `MarketEnrollment` | Per-asset enrollment record: closed vocabulary `REQUESTED`/`ACCEPTED`/`PARTICIPATING`/`SUSPENDED`/`WITHDRAWN`/`REFUSED` (WITHDRAWN/REFUSED terminal) with a closed transition table, the per-asset `ParticipationWindow` (END-EXCLUSIVE §10.3; `None` = not declared, never "always"), `QualificationEvidence` references (declared evidence kinds — no universal taxonomy), terminal-state evidence refusal, supersede link, audit-chain LINK |
| Stacking rules | `StackingRule` (recorded per asset + rule id — jurisdiction record-pack DATA): a mutually-exclusive program set; the enrollment store REFUSES a PARTICIPATING enrollment that would violate a recorded rule (plan risk R-P16-7 — the double-counting hazard is refused by name, never allowed silently) |
| Stores | `MarketProgramStore` / `MarketContractStore` / `MarketEnrollmentStore`: BTreeMap-backed, register-only + the supersede model (a transition is a NEW record superseding the old; every version retained; one current version per identity), closed-transition-table validation, tenant-scoped deterministic reads; no update, no delete, no wall-clock input |

## 2. Recorded decisions

1. **The program's rules are DATA, never FlexCore's decisions** (`PROGRAM_RULES_ARE_DATA_STATEMENT`). The `BaselineMethodDeclaration` carries the program's method name as recorded text — support is the P16.04 engine's decision, never this constructor's.
2. **Records are immutable; state changes are new superseding records** (`MARKET_RECORDS_IMMUTABLE_STATEMENT`) — the P16.01 store's supersede model mirrored in the domain, so the domain vocabulary and the persistence layer state the same thing.
3. **The P11 audit chain is consumed, never redefined**: every record carries `AuditChainLink` (one home — reused by P16.03; the chain treatment itself stays on `audit_records`, P11.15).
4. **Stacking rules are recorded per jurisdiction pair and enforced by refusal** (R-P16-7): the model never widens a rule and never allows a violating combination.
5. **No universal taxonomy invention**: jurisdictions, evidence kinds, and program rules are declared/recorded data with bounds — the ADR-011 boundary.

## 3. Honest boundaries

1. No program, contract, or enrollment record exists anywhere; the stores have never held a row; no workflow has run.
2. The lifecycle transition tables are the domain's design oracle — they model the program-management lifecycle generically (GE/Uplight/CPower analogs [S-41][S-43][S-48]); per-jurisdiction state-name mappings are record-pack data, transcribed at use, never invented here.
3. Every test discipline is the write-unrun class; execution is a human gate. The permitted-tool outputs are quoted in the close-out record (07).
