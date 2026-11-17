# P16.03 — Registration data management: PJM per-DER + aggregation with EDC 45-day review tracking, ERCOT PUCT+ERCOT dual registration, ISO-NE CAMS/eMarket lifecycle, attestations as signed records

| Field | Value |
|---|---|
| Subtask | P16.03 (Per-ISO registration records — PJM per-DER + aggregation registration with EDC 45-day review tracking [S-90]; ERCOT PUCT+ERCOT dual registration [S-100]; ISO-NE CAMS/eMarket asset lifecycle [S-103]; attestations modeled as signed records) |
| Wave / agent | Phase 16 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the record vocabulary is written, never compiled, never executed. **No registration exists, none is filed, none is transmitted, and no qualification is claimed anywhere** (`NO_REGISTRATION_FILED_STATEMENT`; ADR-011; conventions §7.3). The registry is empty everywhere. |
| Sources | [S-90] (PJM FERC 2222 participation model overview — per-DER + aggregation registration, EDC 45-day review), [S-100] (ERCOT ADER pilot — PUCT+ERCOT dual registration), [S-102] (ERCOT registration & qualification), [S-103] (ISO-NE Order 2222 participant readiness — CAMS/eMarket) |
| Files touched | `crates/flexcore-domain/src/market_registration_records.rs` (NEW); `crates/flexcore-domain/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `MarketJurisdiction` | Closed vocabulary at design level: `PJM`, `ERCOT`, `ISO_NE`, `CAISO` — the four the phase plan names; a fifth enters as a NEW variant in a jurisdiction record pack (ADR-011 — additive, never a widened generic) |
| `PjmAggregationRegistration` + `PjmDerRegistration` | The PJM structure: per-DER registrations (DER reference + EDC name + initial review state) under an aggregation reference [S-90]; at least one DER (an aggregation of zero DERs is not registrable) |
| `EdcReviewTracker` + `EdcReviewUpdate` | The EDC 45-day review tracking: an append-only journal per DER (BTreeMap by (der_ref, seq) — deterministic), the closed review vocabulary `NOT_STARTED`→`AWAITING_EDC`→`REVIEW_IN_PROGRESS`→`ACCEPTED_RECORDED`/`REJECTED_RECORDED` (terminal pair), closed-edge validation on append, `review_window_start` + the recorded `PJM_EDC_REVIEW_WINDOW_DAYS = 45` [S-90] — the due date is DERIVED AT USE TIME (re-verified then), never stored as a claim |
| `ErcotDualRegistration` + `RegistrationLeg` | The ERCOT structure [S-100]: the PUCT leg AND the ERCOT leg, EACH with its own reference, state (`INITIATED`/`SUBMITTED_RECORDED`/`ACCEPTED_RECORDED`/`REFUSED_RECORDED`/`WITHDRAWN`), and timestamp — neither leg implies the other |
| `IsoNeAssetRegistration` | The ISO-NE structure [S-103]: the CAMS asset reference and the eMarket listing reference (`None` = not listed yet — recorded as missing, never assumed), closed lifecycle `CAMS_RECORDED`/`EMARKET_LISTED`/`ACTIVE_RECORDED`/`RETIRED_RECORDED` |
| `AttestationRecord` | Attestations as SIGNED RECORDS: who signed, what statement (verbatim), the algorithm LABEL, the signature DIGEST — recorded data, never validated claims; test-grade algorithm labels (`fnv1a-test-grade`) are REFUSED at construction (the LR-203 discipline: a test-grade label is never a release signature; production crypto is a DEPLOYMENT REQUIREMENT) |
| `MarketRegistrationRegistry` | BTreeMap-backed registry keyed by (jurisdiction, tenant, registration_id): register-only + the supersede model (a status change is a NEW record superseding the old; every version retained; scope mismatch refused), the EDC journal, the attestation map — deterministic reads |

## 2. Recorded decisions

1. **No registration exists and none is filed** (`NO_REGISTRATION_FILED_STATEMENT`, pinned). The types model what a registration process WOULD track, written before any jurisdiction is named (D1, OPEN) and before any counterparty exists.
2. **Jurisdiction-specific records, never a generic registration** (ADR-011): the three record types are shaped by their cited jurisdiction materials; a fourth jurisdiction is a NEW record type, never a widened form.
3. **External windows and dates are SOURCED DATA with the re-verify-at-use rule** (`SOURCED_WINDOW_STATEMENT`; plan risk R-P16-1): the 45-day window cites [S-90] and is re-verified against the primary source on the day of use — never treated as a stable fact.
4. **Attestations are signed records — recorded as data, never validated as claims** (`ATTESTATION_IS_RECORD_STATEMENT`): the record stores that a signature exists; it does NOT verify it and is NEVER a qualification or conformance claim.
5. **The P11 audit chain is consumed, never redefined**: every registration record reuses the P16.02 `AuditChainLink` (one home).

## 3. Honest boundaries

1. No registration record, review journal entry, or attestation exists anywhere; the registry has never held a row.
2. The CAMS/eMarket state names are the DESIGN-level closed vocabulary, not transcribed CAMS/eMarket status enums — the exact market-facing status names are transcribed at use from the primary sources [S-103] (marked UNVERIFIED at secondary-source level in the record pack discipline).
3. Every test discipline is the write-unrun class; execution is a human gate. The permitted-tool outputs are quoted in the close-out record (07).
