# P0.09 — Time model

| Field | Value |
|---|---|
| Subtask | P0.09 — Freeze the time model |
| Report trace | §10.3 (normative source), §3.4 (telemetry timestamps), §14.3 (valid-from/valid-until on commands), §17.4 (clock drift threat) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`; `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.pdf` / `.docx` (§6.2 Time row governs on conflict) |
| Status | Draft for Stage 0 review |
| Consumers | Phase 1/2 schema reviews cite this definition; `event_time` and `ingest_time` must be distinguished in every later schema (ledger P0.09 acceptance) |

---

## 1. Time-field vocabulary (§10.3, normative)

Every time-dependent record must distinguish the following fields. (Note: ledger P0.09 step 1 says "five time fields" but its own list and report §10.3 both enumerate six items; this freeze defines all six per §10.3.)

| Field | Definition (§10.3) |
|---|---|
| `event_time` | When the measurement or event occurred |
| `ingest_time` | When FlexCore received it |
| `valid_from` / `valid_until` | Validity window where applicable |
| Source sequence or version | Ordering/version identifier from the source |
| Timezone/UTC offset | Where relevant |
| Clock-quality/synchronization status | Recorded if available |

`event_time` and `ingest_time` are distinct fields and must never be merged or substituted for one another in a later schema. Dispatch commands additionally carry valid-from/valid-until timestamps as a §14.3 command invariant.

## 2. UTC rule (§10.3)

FlexCore uses **UTC internally** and retains the **original site timezone** for:

- operator display; and
- market calendar calculations.

Local time is never stored as the canonical timestamp.

## 3. DST rule (§10.3)

Do **not** assume that a local calendar day always equals 24 hours, because of daylight-saving changes. Schedule horizons, interval counts, and market-day boundaries that are expressed in local calendar terms must handle 23-hour and 25-hour days explicitly.

## 4. Clock-quality rule (§10.3, §17.4)

Clock drift is both:

1. a **threat-model item** (§17.4 lists clock drift, controlled by time-quality monitoring, bounded command expiry, and synchronization checks); and
2. a **data-quality input** — clock-quality or synchronization status is recorded as a field where available (§1), never assumed.

Synchronisation status is a recorded field, not an assumption.

## 5. Foundations §6.2 Time-row cross-check (feeds P0.26)

| This freeze | Foundations §6.2 (Time row) | Outcome |
|---|---|---|
| UTC internally; original site timezone retained (§2) | "UTC epoch/time plus original timezone metadata where relevant." | Consistent |
| §1 field list (§10.3) | Foundations §6.1 Measurement entity carries event time, receive time, sequence and provenance | Consistent; `event_time`/`ingest_time` match event time / receive time |
| Clock-quality rule (§4) | Foundations §6.4: clock offset and event-time skew are monitored | Consistent and mutually reinforcing |
| Foundations adds: "Simulation time is separate from wall-clock time." | Not restated in report §10.3 | Foundations rule adopted; no report conflict. Recorded here so Phase 2/3 simulation work keeps simulated and wall-clock timestamps distinct |

Outcome: zero unresolved conflicts.

## 6. Acceptance criteria coverage (ledger P0.09)

- [x] All §10.3 time fields are defined (§1); `event_time` and `ingest_time` are distinguished, and later Phase 1/2 schemas must reference this definition.
- [x] UTC-internal rule (§2) and DST rule (§3) are stated.
- [x] Clock-quality/synchronization status is a recorded field, not an assumption (§4), with the §17.4 clock-drift threat cited.
