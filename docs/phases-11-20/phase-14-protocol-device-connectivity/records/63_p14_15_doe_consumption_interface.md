# P14.15 — DOE consumption interface

| Field | Value |
|---|---|
| Subtask | P14.15 — per-connection-point / per-group import-export envelopes ingested as optimizer constraint data with issue timestamps, expiry, and update-cadence handling (EDGE Workstream 3 [S-158]); lapse behavior reverting to the static limit (SAPN [S-159]); breach / curtailment telemetry [S-160] |
| Wave / agent | Phase 14 wave 2, Agent B (P14.13–P14.16) |
| Status | WRITTEN-UNRUN: the consumption module and its lapse/bounds/telemetry tests are written, never compiled, never executed. No DOE was ingested, no envelope was received from any operator, no optimizer consumed any bounds produced here, no device was involved. |
| Sources | [S-158] AEMO Project EDGE — Determining the Impact of Update Frequency on Operating Envelope Efficacy, Workstream 3; [S-159] SA Power Networks Flexible Exports; [S-160] Amber Electric flexible-exports note. **Every DOE semantic below is UNVERIFIED against these primary sources** — the gap analysis cites them; their texts were not consulted (§4). |
| Files touched | `crates/flexcore-domain/src/doe_envelope.rs` (NEW), `crates/flexcore-domain/src/lib.rs` (additive module registration only). `crates/flexcore-simulator/` NOT touched (the wave-3 ownership boundary — §5 records the handoff). |

## 1. What landed (deliverable class: domain consumption vocabulary)

| Piece | Where | Honest status |
|---|---|---|
| `ConnectionPointId` (validated non-empty), `EnvelopeScope` (Single / non-empty duplicate-free Group) with deterministic `connection_points()` listing and `covers` | `doe_envelope.rs` | Implemented; UNRUN |
| `ImportExportLimits` (canonical MW, non-negative, zeros are meaningful bounds), `StaticConnectionLimit` (per-connection-point static fallback with citation + provenance) | same | Implemented; UNRUN |
| `DynamicOperatingEnvelope` — envelope id, scope, limits, issue timestamp, expiry (strictly after issue), declared `UpdateCadence` (expected interval + staleness budget, `freshness_limit_seconds()`), latest-update timestamp (≥ issue), issuer, provenance, provenance note; every field checked | same | Implemented; UNRUN |
| The consumption evaluation: `evaluate_envelope(envelope, fallback, now)` → `EffectiveEnvelope::Dynamic { limits }` or `EffectiveEnvelope::Lapsed { reason, fallback }` with the deterministic reason precedence `NotYetInEffect` → `Expired` → `Stale`; pure function of declared data, no clock read | same | Implemented; UNRUN |
| The optimizer constraint seam: `effective_envelope_bounds_mw(effective) → (p_min, p_max)` = `[−import, +export]` (the frozen P > 0 = export rule) — CONSTRAINT DATA ONLY (phase-14 context §3 constraint 3) | same | Implemented; UNRUN |
| Breach/curtailment telemetry: `EnvelopeBreachRecord` (direction, observed `ActivePowerMW`, effective bound, observation time; a record that does not breach is a construction refusal), `CurtailmentRecord` (strictly positive magnitude) — records for the P11 taxonomy to consume, never protection signals | same | Implemented; UNRUN |
| Tests: lapse-on-expiry / not-yet-in-effect / stale-inside-window (all to the static limit), reason precedence, export-sign bounds, zero-limit semantics, group scope determinism, construction refusals, breach/curtailment validation | `#[cfg(test)] mod tests` | Written complete; UNRUN |

## 2. The three DOE behaviors, as encoded

| Behavior | Source label | Encoded as |
|---|---|---|
| Issue timestamps + expiry + update cadence (EDGE Workstream 3 [S-158]) | UNVERIFIED semantics | `issued_at_unix_ms` / `expires_at_unix_ms` / `UpdateCadence::freshness_limit_seconds()`; staleness = age of the latest received update exceeds the cadence budget, evaluated INSIDE the validity window |
| Lapse to the static limit (SAPN [S-159]) | UNVERIFIED transcription | `EffectiveEnvelope::Lapsed { reason, fallback }` for all three lapse reasons; the fallback is MANDATORY declared data — `evaluate_envelope` refuses to evaluate without it (a DOE without a static fallback never silently becomes "unconstrained") |
| Breach / curtailment telemetry [S-160] | UNVERIFIED framing | `EnvelopeBreachRecord` / `CurtailmentRecord`; the P11 observability taxonomy CONSUMES these (trace_id correlation and the alert-rule shape are defined once in Phase 11 and never redefined — gap analysis §9 dependency rule) |

## 3. What this module is NOT

1. **Not an issuance interface** — envelopes arrive from the
   DNSP/operator surface as declared data with provenance; nothing here
   creates, extends, approves, or modifies a DOE.
2. **Not a schedule, not a dispatch decision** — the module's only
   optimizer-facing output is the `(p_min, p_max)` bounds pair. Solver
   discipline is unaffected (context §3 constraint 3): constraint data
   only, no new problem class.
3. **Not a device interface** — no device, no connection, no message; the
   breach/curtailment records are monitoring-path data about simulation
   telemetry at most.
4. **Not a static-limit fabricator** — every `StaticConnectionLimit`
   carries a citation and provenance; an unknown-provenance static limit
   is `Provenance::Unknown` and the P3.02 narrowing rule applies wherever
   it feeds an envelope.

## 4. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | The EDGE Workstream 3 update-frequency semantics (what "missed update" means operationally; the staleness threshold convention) [S-158] | `UpdateCadence`, `LapseReason::Stale` |
| 2 | The exact SAPN lapse conditions and the static-limit fallback definition (does a stale DOE lapse immediately, or after grace?) [S-159] | `evaluate_envelope`, `StaticConnectionLimit` |
| 3 | Whether group envelopes carry per-member or group-aggregate limits in the reference deployments [S-158][S-159] | `EnvelopeScope::Group` (this module carries per-point static limits only — the honest granularity; a group fallback is the consumer's declared composition) |
| 4 | The Amber/CSIP-AUS breach and curtailment reporting framing [S-160] | the telemetry records |

A confirmed mismatch is a recorded edit, never a silent reinterpretation
(plan §7 note 6).

## 5. The wave-3 simulator handoff (recorded; flexcore-simulator untouched)

`crates/flexcore-simulator/src/` is wave-3 Agent A's territory (plan §3).
The DOE profile SHAPE for the simulator lives in THIS module so the
simulator never redefines the semantics:

- wave-3 Agent A (P14.17) extends `crates/flexcore-simulator/src/
  profiles.rs` with a DOE behaviour profile that, per tick, (a) samples
  `evaluate_envelope(envelope, static_limit, tick_time)` for its
  connection point, (b) clips the synthetic asset output with
  `effective_envelope_bounds_mw(&effective)`, and (c) emits a
  `CurtailmentRecord`-shaped value when clipping engages. The scenario
  layer supplies the `DynamicOperatingEnvelope` + `StaticConnectionLimit`
  fixtures (synthetic, TEST-ONLY data — simulated envelopes are never
  evidence about real operator behavior, the P14.17 simulated-test
  discipline).
- No handoff action is required from wave-3 Agent A beyond consuming the
  public API above; `doe_envelope.rs` carries the same statement in its
  module header (the "SIMULATOR HANDOFF" section).

## 6. Honest boundaries

No DOE was ingested; no operator or DNSP issued anything; no optimizer
ran; no breach occurred outside test data. All written-unrun; no
capability claim about real envelopes or real networks.

## 7. Handoff notes

1. **Wave-3 Agent A**: §5 (the profile seam and the fixtures).
2. **Phase 15/16**: the bounds pair is the natural input shape for
   envelope-aware scheduling (gap analysis §3 #4) and for any DOE-aware
   market bidding; consume `effective_envelope_bounds_mw`, never a
   parallel bounds type.
3. **Human gates**: first build; the primary-source verification pass (§4).
