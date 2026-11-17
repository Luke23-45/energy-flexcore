# P16.18 — Market jurisdiction register: register-as-data with ZERO markets enabled

| Field | Value |
|---|---|
| Subtask | P16.18 (Market jurisdiction register — per-tenant market enablement matrix, NONE enabled by default; regulatory-calendar data (FERC 2222 timelines [S-26]); ERCOT ADER pilot posture [S-100]; dates as sourced data with the re-verify rule. Register-as-data: types + a checked-in data file with zero markets enabled) |
| Wave / agent | Phase 16 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the register types, the checked-in data file, and the honesty tests are written, never compiled, never executed. **The checked-in register ships with ZERO market enablements — no market is enabled by default.** No market is connected; the register enables nothing by itself (ADR-011; conventions §7.3). |
| Sources | [S-26] (FERC Order 2222 explainer — the timeline rows), [S-27] (NARUC 2222 tracker — the state/RERRA calendar row), [S-100] (ERCOT ADER pilot — the pilot posture), [S-101] (ERCOT ADER Phase 3.3 report — the document date), [S-103] (ISO-NE participant readiness), [S-97] (CAISO Tariff §11 as of Nov 1 2024), [S-36][S-37][S-114][S-115] (GB LFM), [S-107][S-108][S-109] (EU EBGL); `ledger/phase-09-pilot-qualification/11_market_program_requirements.md` §7 (the Phase 9 capture rows, checked 2026-11-07); the P13 tenant model (`crates/flexcore-domain/src/tenant_context.rs`, consumed by citation — enablements are tenant-scoped rows) |
| Files touched | `crates/flexcore-application/src/jurisdiction_register.rs` (NEW — the types); `crates/flexcore-application/src/jurisdiction_register_data.rs` (NEW — the checked-in data file); `crates/flexcore-application/src/lib.rs` (additive module lines) |

## 1. What landed

| Item | Content |
|---|---|
| `EnablementState` | The closed enablement vocabulary: {DISABLED, PENDING_D1_DECISION, ENABLED_BY_D1}. DISABLED is the default; ENABLED_BY_D1 is constructable ONLY through `TenantMarketEnablement::enabled_by_d1` with the human owner's D1-class decision reference AND the P11 audit-chain link (the single construction path; no default, silent, or inferred enablement is constructable) |
| `JurisdictionEntry` + `TimelineEntry` | The register rows: named jurisdiction, participation-model label, closed posture vocabulary {FERC_2222, ERCOT_ADER_PILOT, UK_LFM, EU_BALANCING}, and a sourced timeline — every date a `SourcedDate` (the P16.16 type; the re-verify rule is mandatory on the type itself). A row without a timeline entry is not a register row |
| `MarketJurisdictionRegister` | The register: BTreeMap-backed rows + per-tenant enablement rows; `validate()` checks the structural invariants; `enabled_jurisdictions_for_tenant` reads only ENABLED_BY_D1 rows |
| `jurisdiction_register_data.rs` | The CHECKED-IN DATA FILE: ten jurisdiction rows — CAISO (live 1 Nov 2024 [S-26][S-97]), ISO-NE (1 Nov 2026 + FCA19 Feb 2026 [S-103]), NYISO (TWO CONFLICTING rows carried with notes on both sides: end-2026 [S-26] vs 30 April 2027 [Phase 9 §7]), PJM (the LR-075 conflict carried on both sides: Feb 2027 proposed [S-26] vs 1 Feb 2028 [Phase 9 §7]), MISO (June 2027/2029 [S-26]), SPP (Q2 2030 [S-26]), ERCOT_ADER (pilot posture, NOT under FERC jurisdiction [S-100][S-101]), GB_LFM (P483 live 27 Nov 2025 [S-115]; Piclo [S-36][S-114]), EU_EBGL (15-minute ISP [S-107][S-108][S-109]), and the FERC_2222_CALENDAR tracker row [S-26][S-27] — with ZERO enablement rows |
| Honesty tests | The data file's tests pin: zero enablements; every date carries the re-verify rule and the research cut-off verification date; the NYISO and PJM conflicts are carried on BOTH sides; the ten rows are present; the ERCOT posture records the non-FERC pilot status |

## 2. Recorded decisions

1. **Register-as-data, never logic** (`REGISTER_AS_DATA_STATEMENT`, pinned by tests): timelines and postures are records with source labels; no date is baked into any code path (plan risk R-P16-1). The data file is a Rust data module (`jurisdiction_register_data.rs`) — checked in, versioned, and diffable, with no runtime I/O.
2. **NONE is enabled by default, structurally** (`NONE_ENABLED_BY_DEFAULT_STATEMENT`, pinned by tests): the shipped data registers no enablement rows, and an enablement decision is a recorded HUMAN act (the D1-class decision, conventions §4 — the human owner is the closer; P16-E-1 stays OPEN). The register can RECORD a decision; it can never MAKE one.
3. **Conflicts are carried, never silently resolved** (the LR-075 pattern, generalized): the NYISO rows (gap analysis vs Phase 9 capture) are carried as TWO entries with conflict notes on both sides, exactly as the PJM/LR-075 conflict is. No code path selects a side.
4. **Jurisdiction specificity is the whole point** (`JURISDICTION_SPECIFIC_STATEMENT`, pinned by tests): the register NAMES jurisdictions as data rows; it is never a generic "Order 2222 adapter" or universal market API (ADR-011). A new jurisdiction is a NEW data row with its own sources.
5. **The P13 tenant model is consumed by citation**: enablement rows are tenant-scoped (the P1.06 tenant-context pattern governs who may record one at run time); no tenant-scoping type is redefined here.

## 3. Honest boundaries

1. The register enables nothing; no D1 decision has been made or recorded; the store carries zero enablements.
2. Every date is verified as of the research cut-off 2026-11-07 and re-verified at use; the Phase 9 §7 rule is inherited verbatim. The Elexon P483 date is sourced via industry coverage [S-115] and carries that provenance label honestly.
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the close-out record (47).
