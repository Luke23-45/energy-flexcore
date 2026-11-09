# P0.08 — Telemetry quality vocabulary (six states, exactly one per value)

| Field | Value |
|---|---|
| Subtask | P0.08 — Freeze the telemetry quality vocabulary (six states, exactly one per value) |
| Report trace | §10.4 (normative source), §3.4 (telemetry arrives with quality flags), §10.1 (AssetState carries quality), §15.3 (`TelemetryQualityChanged` event); foundations §6.4 (governs on conflict) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`; `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.pdf` / `.docx` |
| Status | Draft for Stage 0 review |
| Consumers | Phase 2 (telemetry model and adapter contract tests enforce the six-state enum, per ledger P0.08 risk note); Phase 4 (estimation constraint on authorization) |

---

## 1. The six canonical quality states (§10.4, normative)

Every telemetry value carries exactly one canonical quality state:

| Canonical state | Meaning (§10.4) |
|---|---|
| `GOOD` | Valid, fresh, within range, and from a trusted source |
| `SUSPECT` | Received and parsed, but a plausibility, range, or consistency check failed |
| `BAD` | The source reports the value invalid, or a hard validation rule failed |
| `MISSING` | No value was received for the expected interval |
| `ESTIMATED` | Derived, interpolated, substituted, or model-generated rather than measured |
| `STALE` | Last known value whose age exceeds the use-case freshness limit |

There is no seventh state anywhere in the Phase 0 freeze. No adapter, simulator, or plugin may introduce an additional canonical quality state.

## 2. Exactly-one rule (§10.4)

Every telemetry value carries **exactly one** canonical quality state. A value is never simultaneously unlabelled and never carries two states.

## 3. Source-state mapping rule (§10.4)

Richer source states — for example `SUBSTITUTED`, `OUT_OF_RANGE`, `UNSYNCHRONIZED`, `COMMUNICATION_LOSS`, and `UNRELIABLE` — must be **mapped to one of the six canonical states at the adapter boundary**, with the **original source code retained as diagnostic metadata**. The canonical vocabulary does not grow; source detail survives as metadata.

## 4. Non-silencing rule (§10.4)

A quality flag must **not** be silently converted into a normal reading. Downgrading, dropping, or omitting a quality flag at any boundary is prohibited.

## 5. Per-use-case acceptability rule (§10.4)

Every use case defines which quality states are acceptable. This is to be specified per use case in later phases; **Phase 2 defines the defaults for the current-state and historical views**. There is no universal acceptability set.

## 6. Foundations §6.4 estimation rule and Phase 4 carry-forward

Foundations §6.4 ("Data quality and freshness", governing on conflict per AGENTS.md §2) states: **estimation/interpolation is explicit, tagged, bounded by policy, and disallowed for safety-critical authorization unless approved.** This is consistent with the §10.4 `ESTIMATED` definition and is carried forward by this freeze.

**Phase 4 carry-forward:** the disallowance has no carrier in Phase 2 (telemetry only) — it is recorded here and handed to Phase 4, where command authorization first exists, as a constraint on any authorization decision that would rely on `ESTIMATED` values (ledger P0.08 step 6).

Foundations §6.4 cross-check (feeds P0.26):

| This freeze | Foundations §6.4 | Outcome |
|---|---|---|
| Six states with §10.4 meanings (§1) | "Every measurement MUST carry a quality status: GOOD, SUSPECT, BAD, MISSING, ESTIMATED, or STALE (or a mapped equivalent with consistent semantics)." | Consistent; the §10.4 mapping rule (§3) is the mapped-equivalent mechanism |
| STALE = age exceeds the use-case freshness limit (§1) | "Freshness limits are configurable by point type, service, protocol and operating mode. There is no universal stale timeout." | Consistent |
| Non-silencing rule (§4) | "Bad or stale values are not silently replaced by zeros." | Consistent |
| Estimation rule (§6) | "Estimation/interpolation is explicit, tagged, bounded by policy, and disallowed for safety-critical authorization unless approved." | Consistent |
| Foundations adds: corrections/late arrival append-only or versioned for replay | Not restated in report §10.4 | Foundations rule adopted; no report conflict; recorded for Phase 2 telemetry design |

Outcome: zero unresolved conflicts.

## 7. Related vocabulary (definitions only, from the report trace)

- §10.1 `AssetState` carries a quality field alongside timestamp, SOC, active/reactive power, availability, operating mode, and health.
- §15.3 defines the `TelemetryQualityChanged` domain event; quality changes are facts, not imperative commands.
- §3.4 requires telemetry to arrive with timestamps, units, source identity, quality flags, and sequence information; the platform validates, normalizes, and stores it.

## 8. Acceptance criteria coverage (ledger P0.08)

- [x] Exactly six states, each with the §10.4 meaning reproduced (§1).
- [x] The exactly-one rule (§2) and the source-mapping rule (§3) are stated.
- [x] The non-silencing rule is stated (§4).
- [x] The foundations §6.4 estimation rule is stated, including that estimation is disallowed for safety-critical authorization unless approved, with the Phase 4 carry-forward recorded (§6).
- [x] No seventh state is introduced anywhere in this Phase 0 artefact (§1); the only enumerated quality names are the six canonical states plus the §10.4 source-state examples retained as mapped source metadata (§3).
