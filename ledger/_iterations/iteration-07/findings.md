# Iteration 7 findings — Data, contracts, interfaces (Phase 10 + cross-phase consistency)

Reviewer: iterations 7–10 subagent. Date: 7 November 2026.
Scope: `ledger/phase-10-v1-release-hardening/` (patched) against report §10, §15, §16.3, §21.3
and the cross-phase contract owners (P0.07, P0.09, P0.14, P1.09, P1.12, P1.14–P1.17, P2.05, P2.09,
P2.16, P2.20, P9.08, P9.10).
Method: read P10 ledger (1216 lines pre-patch), P10 RESEARCH/CONTEXT/PLAN/CHECKLIST, report
§10.1–§10.5, §15.1–§15.5, §16.3, §21.3; traced every §10/§15 contract element to the subtask
that freezes or re-verifies it on the release candidate; scripted orphan-reference check
(0 orphans across the whole ledger).

Severity scale: Critical = release would ship a wrong/unsafe contract or lose data;
High = a contract class is never frozen or never re-verified on the candidate;
Medium = a real gap in one check or one surface; Low = wording/coverage polish.

Counts: **High 3, Medium 5, Low 2. Critical 0.**

## (a) Missing subtasks

None. Every contract class below is owned by an existing subtask once extended; no new
subtask was needed. P10 subtask total stays **30**.

## (b) Issues / errors / contradictions

### H-7.1 — No OpenAPI breaking-change detection; §15.2 coverage and the forbidden endpoint unchecked
- **Severity: High.**
- **File + subtask:** `ledger/phase-10-v1-release-hardening/ledger_phase_10.md`, **P10.02**.
- P10.02 froze contracts with Buf lint/breaking only. Buf covers Protobuf — it does not cover
  REST. Nothing in Phase 10 ran an OpenAPI diff, verified the §15.2 candidate resources
  (16 resources under `/api/v1`) were all present, confirmed the spec was generated (not
  hand-written, P1.17), or checked that no generic "execute arbitrary command" endpoint exists
  (§15.2). A breaking REST change could ship silently while the Buf gate passed.
- **Patch applied:** P10.02 step 4 (OpenAPI diff/breaking check, §15.2 resource coverage,
  generated-spec check, forbidden-endpoint check), acceptance (e), Verification method extended.

### H-7.2 — NATS event vocabulary and JetStream stream configuration never frozen or verified at release
- **Severity: High.**
- **File + subtask:** `ledger_phase_10.md`, **P10.02** (freeze) and **P10.11** (restore).
- The §15.3 event vocabulary (26 events, AssetRegistered … DispatchFallbackActivated,
  PluginActivated/Failed, OperatorActionRecorded) and the §15.4 per-stream JetStream policy
  (retention, replication, acknowledgement, replay, dead-letter/quarantine) were designed in
  P1.09/P1.14 and published in P2.16, but no Phase 10 subtask froze them or re-verified them
  on the candidate. An event renamed, dropped, or re-subjected between P1 and v1.0.0 would
  have been a silent breaking change. P10.11's restore test likewise never checked the
  restored stream configuration.
- **Patch applied:** P10.02 steps 1, 6, 7 + acceptance (g) (vocabulary + subject mapping +
  stream policy frozen; Inputs cross-reference P0.14/P1.09/P1.14/P2.16); P10.11 step 5 +
  acceptance (e) (restored stream config matches P1.14 design).

### H-7.3 — §16.3 retention/residency never re-verified on the release candidate
- **Severity: High.**
- **File + subtask:** `ledger_phase_10.md`, **P10.04**.
- Retention was designed in P0.16 (principles), P1.12/P1.14 (configuration), P2.05/P2.20
  (storage/partition maintenance), and P9.10 ("Data residency, retention, and privacy design",
  per-data-class retention, §16.2–§16.3). P10.04's Inputs cross-referenced P9.08/P9.10, but
  its steps and acceptance had **no** retention or residency criterion at all. A candidate
  with a hard-coded universal retention period (the exact risk P0.16 names) could pass the
  release gate.
- **Patch applied:** P10.04 step 6 + acceptance (e): per-data-class retention configured from
  the customer contract (no hard-coded universal period), residency setting, and the
  personal-data export/deletion procedure present on the candidate.

### M-7.1 — §10.1 canonical-entity coverage never verified on the frozen contracts
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.02**.
- The fourteen §10.1 entities (Tenant, Portfolio, Site, Asset, AssetState, CapabilityEnvelope,
  GridModel, ForecastBundle, Schedule, DispatchCommand, DispatchExecution, MarketCommitment,
  PluginInstance, AuditRecord) were defined in P0.07 and built in P1/P2, but the freeze never
  confirmed all fourteen were represented in the frozen public contracts.
- **Patch applied:** P10.02 step 5 + acceptance (f).

### M-7.2 — Canonical units/sign and the §10.3 time model not re-verified on the candidate
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.02**.
- §10.2 (MW/MVAr/MVA/MWh, `P > 0` export, reactive-power sign declared per grid model/device
  profile, SOC as a fraction in [0,1] at the API boundary) and §10.3 (`event_time` vs
  `ingest_time`, `valid_from`/`valid_until`, source sequence/version, UTC internally) had no
  release-time check. These are the cheapest mistakes to ship and the hardest to fix after
  v1.0.0 freezes the contracts.
- **Patch applied:** P10.02 step 5 + acceptance (f) (same step as M-7.1).

### M-7.3 — Outbox/inbox idempotency state never verified across restore or upgrade
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.11** and **P10.12**.
- The outbox/inbox design is sound in P1.12 (idempotency key unique), P1.15 (Nats-Msg-Id =
  idempotency key), and P2.09 (duplicate/out-of-order handling), and P10.11 step 2 verifies
  idempotent replay — but neither the restore test (P10.11) nor the upgrade/rollback test
  (P10.12) checked that pending outbox entries and inbox/dedup records **survive** the
  restore/upgrade without loss or duplicate publication. That is exactly the state an
  upgrade destroys.
- **Patch applied:** P10.11 step 5 + acceptance (e); P10.12 step 5 + acceptance (e).

### M-7.4 — `ESTIMATED` checked in the console only, not in API responses
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.06**.
- P10.06 step 5 verified `ESTIMATED` values are visually distinct in the console (§20.2).
  An API consumer — the party most likely to feed an estimate into a schedule or a market
  commitment — had no equivalent check. The §10.4 quality state (exactly one of
  GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE) must be observable at the API boundary, not
  only on screen.
- **Patch applied:** P10.06 step 6 + acceptance (c) extended to telemetry/query API responses.

### M-7.5 — JetStream restore is the CVE-2026-33222 surface and P10.11 never tested its authorization
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.11**.
- CVE-2026-33222 is a JetStream stream-restore authorization bypass (CVSS 4.9; fixed in
  2.11.15/2.12.6 — see iteration-09 findings for the P1/P2 wording correction). P10.11
  performed a restore but never required scoped restore authorization or a refused restore
  to an unauthorized stream name.
- **Patch applied:** P10.11 step 5 + acceptance (e) (scoped restore authorization; unauthorized
  stream-name restore refused; cross-reference to Phase 1 `RESEARCH.md`).

### L-7.1 — §10.5 "do not expose raw DB schemas as public API" never checked at release
- **Severity: Low.**
- **File + subtask:** `ledger_phase_10.md`, **P10.02**.
- **Patch applied:** folded into P10.02 step 5 + acceptance (f).

### L-7.2 — P10.13 breaking-change record covered Protobuf only
- **Severity: Low.**
- **File + subtask:** `ledger_phase_10.md`, **P10.13**.
- A migration that renames a NATS subject or drops a REST resource is a breaking change even
  if no Protobuf file changed; P10.13 step 4 recorded only the Buf result.
- **Patch applied:** P10.13 step 4 extended + acceptance (d) reworded to "no silent breaking
  Protobuf, REST, or event-subject change"; step 4 cross-references the P10.02 breaking record.

## (c) Suggestions

- When P10.02 runs, file the OpenAPI diff and the event-vocabulary list as artifacts next to
  the Buf report so a later phase can diff against v1.0.0 the same way (no new subtask; a
  filing convention for whoever executes P10.02).
- P10.11's scoped-restore check should use a restore credential that is authorized for one
  stream only, so the refusal is exercised rather than asserted.

## (d) Stale research

None in iteration-7 scope. The §10/§15/§16.3 grounding is report text, not versioned research;
versioned claims are covered in iteration 9.

## Checked and clean (re-verified, no finding)

- Telemetry quality states (§10.4): P10.06 injects stale/missing/estimated cases and requires
  hold/reject with zero silent substitutions; the six-state enumeration is stated in the
  P10 header glossary.
- Protobuf/gRPC governance (§10.5): Buf lint + breaking detection in P10.02 and P10.18 (§22.2
  CI gates), plus the P10.13 migration check.
- REST/OpenAPI governance (§15.1–§15.2): P1.17 (Axum + utoipa, generated spec) is the build;
  the release-time gap was the missing diff, now H-7.1.
- NATS vocabulary/outbox design: P1.09, P1.12, P1.14, P1.15, P2.09, P2.16 are mutually
  consistent; the release-time gap was the missing freeze/verification, now H-7.2/M-7.3.
- No contradiction found between §10/§15/§16.3 and any P10 subtask's stated trace.

## Applied vs deferred log

| Item | Disposition |
|---|---|
| H-7.1 | **Applied** — P10.02 step 4 + acceptance (e) + Verification method |
| H-7.2 | **Applied** — P10.02 steps 1/6/7 + acceptance (g); P10.11 step 5 + acceptance (e) |
| H-7.3 | **Applied** — P10.04 step 6 + acceptance (e) |
| M-7.1 | **Applied** — P10.02 step 5 + acceptance (f) |
| M-7.2 | **Applied** — P10.02 step 5 + acceptance (f) |
| M-7.3 | **Applied** — P10.11 step 5 + acceptance (e); P10.12 step 5 + acceptance (e) |
| M-7.4 | **Applied** — P10.06 step 6 + acceptance (c) |
| M-7.5 | **Applied** — P10.11 step 5 + acceptance (e) |
| L-7.1 | **Applied** — P10.02 step 5 + acceptance (f) |
| L-7.2 | **Applied** — P10.13 step 4 + acceptance (d) |

Deferred: none. No iteration-7 item required an edit outside phase-10 files.
