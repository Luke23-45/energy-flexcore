# P10.06 — Stale/Missing Telemetry Handling Verification (record)

- **Subtask:** P10.06 (§22.3 release gate 5; report §10.4, §17.4–§17.5).
- **Status:** WRITTEN-UNRUN — implemented, awaiting human build/test (ENTRY_NOTE
  boundary 3). No `cargo` command was executed in this loop; zero observed
  failures is NOT claimed. Ledger tick = "implemented, awaiting human build/test".
- **Dependencies honesty:** P10.01/P10.18 are concurrent waves; the suite must
  be re-run against the frozen release-candidate identifier. P9.18 (telemetry
  verification and the recorded per-use-case freshness limits) is a
  cross-reference only: this subtask proves the limits are per-use-case
  CONFIGURATION (never hard-coded defaults); it does not invent or re-record
  the P9.18 values.

## 1. The six quality states → documented handling (verified by test)

| State | Real producer seam (cited) | Verified handling | Test |
|---|---|---|---|
| `GOOD` | `flexcore_domain::quality_rules::assign_quality` (`PassedAllChecks`) | accepted; the control command creates and validates on a fresh envelope | `p10_06_telemetry_quality_handling.rs` (`the_control_fresh_envelope_validates…`, `the_good_suspect_bad_boundary…`) |
| `SUSPECT` | `assign_quality` soft range/consistency failure | stored WITH its flag, never downgraded or dropped (reason + evidence travel); a quality TRANSITION on a scoped asset invalidates the envelope the schedule consumed | `the_good_suspect_bad_boundary…`, `every_quality_state_transition_on_a_scoped_asset_invalidates_the_envelope` |
| `BAD` | `assign_quality` hard schema/SOC/source failure (first-failure-wins order) | not a usable measurement; the quality event invalidates the envelope and the P4.11 validation gate REJECTS the command (fail-closed) | `a_command_does_not_proceed_on_bad_telemetry`, `the_good_suspect_bad_boundary…` |
| `MISSING` | `quality_rules::expected_intervals` gap detector (derived `flexcore:derived:` markers, NO value field) | the marker cannot carry a substituted value by construction; the quality event invalidates the envelope and the command gate REJECTS | `missing_gaps_are_derived_markers_with_no_value_field`, `a_command_does_not_proceed_on_missing_telemetry` |
| `STALE` | `quality_rules::staleness` (age from `event_time`, never ingest time — distinct contract types) | hand-computed boundary: `age == limit` NOT stale, `age > limit` stale; `stale_quality()` yields `Some(STALE)` (no silent re-labelling); the STALE event invalidates the envelope and the command gate REJECTS | `the_staleness_boundary…`, `freshness_limits_are_per_use_case_configuration…`, `a_command_does_not_proceed_on_stale_telemetry` |
| `ESTIMATED` | `quality_rules::derive_estimated` (the ONLY producer; requires a `DerivationRule`) | unreachable without an explicit configured rule; explicitly tagged with rule id + detector; distinguished from measured (`quality_state_changed`) | `estimated_is_unreachable…` |

Adapter-boundary mapping (`flexcore-contracts::map_source_state`, P0.08): the
§10.4 source states map into the six canonical states and NEVER to `GOOD`
(`COMMUNICATION_LOSS` → `STALE`, `SUBSTITUTED` → `ESTIMATED`, range/consistency
source faults → `SUSPECT`) — pinned in both P10.06 test files.

## 2. What was produced (files)

| File | Content |
|---|---|
| `crates/flexcore-application/tests/p10_06_telemetry_quality_handling.rs` | The six-state handling sweep over the REAL P3.13 staleness evaluation (`evaluate_envelope_trigger`: ANY quality transition on a scoped asset invalidates — non-silencing in both directions; foreign-tenant / out-of-scope events never invalidate); the command-gate fail-closed proof: STALE / BAD / MISSING quality events each lead to `passed=false` with the check named `envelope_stale:<reason>`, PROPOSED → REJECTED journalled, `DispatchRejected` emitted (the harness mirrors the P4.21 fixtures with a controlled clock; the control fixture validates, so every rejection is designed); the §14.3 superseded-revision creation rejection; hand-computed boundaries (staleness `age==limit`/`+1`; SOC domain `[0,1]` inclusive vs 1.1/-0.1/NaN; rated-limit inclusive at ±2.0 MW vs 2.4 MW; hand-computed gap markers at 60 s/180 s; producer separation over all six states; ESTIMATED unreachable without a rule; per-use-case freshness limits as configuration). |
| `crates/edge-agent/tests/p10_06_clock_quality_stale_interaction.rs` | The clock-quality interaction: `COMMUNICATION_LOSS` arrives `STALE` and no source state upgrades to `GOOD`; the hold-on-drift rule (expiry checks trusted ONLY on `Synchronized`; `Degraded`/`Unsynchronized`/`Unknown` hold, with the named reason; the composed `audited_expiry_check` records `EDGE_CLOCK_HOLD_ON_DRIFT`); the STALE × Degraded-clock interaction proven as two independent fail-closed surfaces (the stale sample is never substituted; the degraded clock never trusts expiry checks); hand-computed boundaries (pilot tolerance inclusive at 100 ms, Degraded at 101 ms; sync-timeout inclusive at 30 000 ms, `Unsynchronized` at 30 001 ms; a corrupt pair ≥ one day refused, never estimated, state untouched); health rendering `DEGRADED_CLOCK_DRIFT` / `DEGRADED_UNVERIFIED` (never healthy-by-default) and the P6.15 verifier-seam mapping. |

## 3. Reviewer-verification procedure (the human gate)

```sh
cargo test -p flexcore-application --test p10_06_telemetry_quality_handling
cargo test -p flexcore-edge-agent --test p10_06_clock_quality_stale_interaction
# Independent re-run (P10.06 verification method): re-run one stale-injection
# and one missing-injection case against the release candidate once P10.18
# records the candidate identifier.
```

## 4. Acceptance-criteria coverage and honest gaps

| P10.06 criterion | Covered here | NOT executable in this loop |
|---|---|---|
| (a) every injection case has a recorded hold/reject outcome — zero silent substitutions | The STALE/BAD/MISSING command-gate rejections and the never-substitute proofs (no value field on gap markers; `stale_quality()` re-labelling; `map_source_state` never GOOD) | The per-pilot-asset-class injection runs (step 2's per-asset-class matrix) need the deployed candidate; the injection CASES are proven, the per-class RUN table below is EMPTY. |
| (b) no feasibility check passes on stale/missing input | The validation-gate rejections on STALE/BAD/MISSING envelope staleness; the P8.05 harness side (stale/unknown snapshot → refusal with zero gate attempts) is pinned in the P10.05 research suite (`snapshot_freshness` stage) — cross-referenced | A run over every feasibility PATH in the deployed candidate (not the gate seam) is a candidate-build gate. |
| (c) `ESTIMATED` visually distinct in the console AND flagged in the API (§20.2, §15.2) | The DATA layer is proven: `ESTIMATED` is a distinct canonical state, tagged with its derivation rule, and `quality_state_changed` fires the `TelemetryQualityChanged` event the API/console surface consumes | The console VISUAL rendering and the §15.2 response-shape check need the running console/API build — NOT EXECUTABLE HERE; recorded as the open step (steps 5–6 of the subtask). |
| Freshness limits are the recorded ones (step 3) | Proven structurally: limits are per-use-case parameters, `try_new`/`from_seconds` validate positivity, the same age flips stale/fresh across two configured limits | The P9.18 recorded VALUES must be re-confirmed against the candidate's configuration on deployment — human gate. |
| A stale/missing model is never a passing feasibility check (step 4) | Pinned in `crates/flexcore-research/tests/p10_05_independent_validation.rs` (`SNAPSHOT_UNKNOWN` / `SNAPSHOT_SUPERSEDED` / `PROPOSAL_EXPIRED` refusals with zero gate attempts) and the FailClosed/limit-violation refusals — cross-referenced from this record | — |

## 5. Injection-outcome table (EMPTY — human gate input on the candidate)

| Asset class | Use case | Injection (state) | Freshness limit applied | Outcome | Recorded by |
|---|---|---|---|---|---|
| (none) | — | — | — | — | — |

## 6. Findings (recorded, not fixed — src logic is outside this subtask's ownership)

- **F-P10.06-1 (observation, no action taken):** the staleness rule is
  `age > limit` (frozen P2.08 boundary, re-pinned here). A future-dated
  `event_time` yields a negative age and is therefore "fresh" in the staleness
  detector — clock skew is deliberately a SEPARATE fault (SUSPECT per the
  P2.14 path / `IngestBeforeEvent` at the time-model boundary, pinned by the
  P4.21 F-ST-6 fixture). The interaction with a DRIFTED edge clock is covered
  by the hold-on-drift rule in the edge suite: a drifted clock does not trust
  expiry checks at all. Recorded so reviewers do not read "fresh" as "trusted".
- **F-P10.06-2 (observation, no action taken):** the envelope-invalidation rule
  invalidates on ANY quality transition, including improvements (e.g. SUSPECT →
  GOOD re-computes the envelope). This is the recorded non-silencing decision
  (P3.13); it is conservative and correct for the gate, but consumers should
  expect recomputation advice on every quality flip.
- **F-P10.06-3 (recorded constraint):** the edge clock-quality state maps
  `Degraded` → `ClockQuality::Unsynchronized` for the P6.15 verifier (the
  contracts vocabulary has no `Degraded`). The mapping is a true statement
  ("not synchronized within tolerance") and is recorded in the module; the
  richer `Degraded` fact survives in the health record (`DEGRADED_CLOCK_DRIFT`)
  and the audit event. No defect; recorded for audit-query expectations.

## 7. Ledger tick

`P10.06` in `ledger_phase_10.md` was ticked in the same wave as this record,
with the ENTRY_NOTE boundary 3 meaning: **implemented, awaiting human
build/test**; the console/API visual checks (steps 5–6) and the per-asset-class
injection runs are recorded as open human-gate steps, not as done.
