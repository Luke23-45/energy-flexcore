# P4.03 + P4.04 + P4.05 — Decision note: the versioned model + model health,
# and feasibility-gate pipeline steps 1–4

- **Date:** 2026-11-07.
- **Subtasks:** P4.03, P4.04, P4.05 (`ledger_phase_4.md` §§P4.03–P4.05).
  Report trace: §12.5 steps 1–4 (and 6 via the scope limitations), §12.4,
  §10.2–§10.4, §20.1 item 5 (display contract only), §17.5 invariant 9.
- **Deliverable crate:** `crates/flexcore-grid-api` (new modules
  `model_health.rs`, `feasibility.rs`; tests `tests/model_health.rs`,
  `tests/feasibility_gate.rs`).
- **P4.08 relationship:** this note pre-wires the P4.08 triggers it touches
  (stale model, BAD/MISSING/STALE baseline, non-convergence); the
  reject-vs-HOLD policy table itself is P4.08's — where this pipeline stops,
  it records ABORT/rejection evidence and decides nothing.

## 1. Decisions recorded here

| # | Decision | Where implemented |
|---|---|---|
| D-P4.03-1 | **Model lifecycle states, decided once:** `Draft`, `Active`, `Retired`. Transitions: Draft→Active, Draft→Retired, Active→Retired. No transitions out of Retired, no re-activation (tested). ONLY an `Active` model may back a feasibility check. | `model_health.rs` (`ModelLifecycleState`, `ModelVersionRegistry::transition`) |
| D-P4.03-2 | **Content hash:** FNV-1a 64-bit over the `serde_json` serialization of the model IR (`fnv1a64:<16 hex>`), ASSIGNED by the versioning layer at registration — a caller-supplied hash is overwritten, never trusted. Versions are immutable: ANY re-registration of an existing `(model_id, version)` is refused (a change = a NEW version). `VersionedModel::verify_content_hash` recomputes against the live content: a mismatch invalidates prior evidence (the P4.12 hook) and aborts the gate (the P4.05 pre-run hook). ASSUMPTION (recorded): hash determinism holds for a given Rust toolchain + serde_json version (ryu shortest float formatting); cross-toolchain stability is NOT claimed and is re-verified when the durable store lands. | `model_health.rs` (`model_content_hash`, `VersionedModel::register`, `verify_content_hash`) |
| D-P4.03-3 | **Model health = exactly the three ledger axes, reported ALL at once:** completeness (structural validation + a non-empty coverage statement), staleness (any source-data change recorded after `last_verified_at` is unverified; plus the configured max age since last verification — `age == limit` NOT stale, the P2.08/P3.13 convention; default 24 h is an ASSUMPTION pending the P4.08 review), and coverage (scope-vs-registry: every registry element the caller declares must be in the model's declared coverage — an uncovered element is a FINDING, never silently dropped). The content-hash check is deliberately NOT part of the health report: it is the separate pre-run hook so the two fail-closed conditions carry distinct, attributable evidence. `assert_usable_for_feasibility` refuses a model with ANY finding — the gate calls this BEFORE applying a candidate. | `model_health.rs` (`check_model_health`, `assert_usable_for_feasibility`, `ModelHealthConfig`) |
| D-P4.03-4 | **Storage seam:** flexcore-grid-api depends only on flexcore-contracts + serde/serde_json/time (the dependency-direction test pins the set), so this subtask ships the in-crate append-only registry; the durable (PostgreSQL) model-version store is a composition-root/adapter deliverable that must re-verify hashes through `model_content_hash`/`verify_content_hash`. | `model_health.rs` (module header) |
| D-P4.04-1 | **Candidate intake (step 1) happens BEFORE any model is touched:** `validate_candidate` receives the candidate + seams only — the model IR is not a parameter. Checks: refs present (schedule id, revision ≥ 1, envelope version, result-record snapshot ref), setpoints complete and non-empty, units canonical (STRUCTURAL — the frozen `ActivePowerMW`/`ReactivePowerMVAr` contract types make kW/kWh unrepresentable at the boundary; the P3.04 `validate_unit` text rule is its upstream counterpart), timestamp not in the past beyond the configured tolerance (default 5 min, CONFIGURATION/ASSUMPTION pending P4.08), validity window ordered + duration consistent, setpoint intervals inside the window, envelope fresh, revision current. | `feasibility.rs` (`validate_candidate`, `CandidateValidationConfig`) |
| D-P4.04-2 | **The four injected seams (the recorded solution to "grid-api must not depend on flexcore-storage/application/domain"):** (1) `capability_bounds` — per asset + interval, the DECLARED bounds; the caller supplies P3.04-validated capability data (`flexcore_domain::capability_validate::validate_capability_model` is REUSED, not reimplemented — the P3.01 interval constraints are the source of the numbers); every bound REQUIRED (undeclared is never unconstrained, the P3.02 rule). (2) `envelope_freshness` — the P3.13 staleness gates, mirrored as `EnvelopeFreshness`. (3) `revision_superseded` — mirrors `ScheduleRevisionChainStore::is_revision_superseded`: Active/Superseded/Unknown; BOTH superseded and unknown are REJECTED (`None` is never "usable", the P3.13 statement). (4) `ramp_baseline` — the latest telemetered state WITH its quality flag. Any seam ERROR rejects fail-closed; no seam error degrades to a pass or a skip. | `feasibility.rs` (`GateSources`, `GATE_SEAM_STATEMENT`) |
| D-P4.04-3 | **Injection-quality policy (P4.08 pre-wiring):** the ramp baseline is the latest telemetered state WITH its quality flag; `GOOD`, `SUSPECT`, `ESTIMATED` are accepted; `BAD`, `MISSING`, `STALE` BLOCK the check (exactly the P4.08 trigger enumeration). The quality is recorded alongside EVERY derived injection (in the `DerivedInjection.baseline` and in the deterministic derivation reference `derived_injection:<id>:asset=<a>:baseline@<t>:quality=<Q>`); a flag is never silently converted (§10.4). | `feasibility.rs` (`INJECTION_QUALITY_POLICY`, `derive_injections`) |
| D-P4.04-4 | **Ramp semantics:** allowed ΔP = `ramp_mw_per_min × (stated_duration_millis / 60000)` — the candidate-level change over the candidate's own stated duration; a finer per-interval ramp profile is the scheduler's (P3.22) concern. The Q convention declared by the setpoint must EQUAL the capability model's declared convention — a mismatch rejects (the Q sign is never inferred or silently converted, §10.2). The derived injection must round-trip the OWNED translation layer (canonical → worker → canonical, exact) — a mismatch is a contract-broken rejection, never expected, always fatal. | `feasibility.rs` (`derive_injections`) |
| D-P4.05-1 | **Step 3 — application is health- and hash-gated:** the P4.03 health check runs FIRST (a stale/incomplete/uncovered model aborts BEFORE the run — tested: the backend call count stays 0), then the pre-run content-hash hook re-verifies the live artefact against the recorded hash — a mid-gate model change aborts fail-closed with `MODEL_HASH_MISMATCH`. The model version + hash are recorded on the attempt (evidence references THE HASH). The limit configuration carried for P4.06 is validated at this boundary (an invalid limit set never enters an attempt record). | `feasibility.rs` (`apply_and_run_power_flow`) |
| D-P4.05-2 | **Step 4 — the AC power flow runs through the PORT only.** An AC power-flow evaluation of specified injections/controls; NOT AC-OPF (stated in `AC_POWER_FLOW_STATEMENT`, carried in the crate docs; the terms are never interchanged). Non-convergence and provider-reported errors are in-band INCONCLUSIVE: recorded as inconclusive evidence with diagnostics, fail-closed — never a pass, never a rejection-by-limit (P4.07 owns the verdict category). **Retry policy: NONE** (`NO_RETRY_POLICY`) — no code path retries with altered settings; adding one is a recorded decision. A port `GridPortError` (backend/contract failure) is an ABORT, not a power-flow outcome. | `feasibility.rs` (`AttemptOutput::Inconclusive`, `NO_RETRY_POLICY`, `AC_POWER_FLOW_STATEMENT`) |
| D-P4.05-3 | **Unbalanced/asymmetric limitation:** the balanced-radial default is a recommendation (D3 OPEN); the declared scope's `unbalanced_three_phase` limitation surfaces in every result's coverage limitations, and every attempt record carries `UNBALANCED_DEFERRAL_NOTE` pointing at the P4.34 unbalanced three-phase track. | `feasibility.rs` (`UNBALANCED_DEFERRAL_NOTE`) |
| D-P4.05-4 | **`FeasibilityAttemptRecord` is EVIDENCE ONLY:** inputs (candidate refs, model version + hash, per-injection derivation refs with baseline quality), output (`PowerFlowResult` or `Inconclusive` with reason), timestamps, provider id + REQUIRED version, and the unbalanced note. NO feasibility verdict is carried or implied — the limit check is P4.06, the reject/constrain/return routing is P4.07. `validate_complete` enforces: refs non-empty, revision ≥ 1, model ref valid, derivation refs present, timestamps ordered, provider pin non-empty, output branch consistent with the carried result's status (a converged result under `PowerFlowEvaluated`; a non-converged result + non-empty reason under `Inconclusive`), and the carried result's own completeness gate. The pipeline refuses to publish an incomplete record (fail-closed). | `feasibility.rs` (`FeasibilityAttemptRecord`, `AttemptRecordError`) |

## 2. Gaps and limitations (recorded, none silently filled)

- **Result-record snapshot ref resolvability:** step 1 checks presence
  (non-empty) of `result_record_snapshot_ref`; its resolvability against
  the persisted schedule-result store is enforced by the composition root
  (flexcore-grid-api cannot name flexcore-storage) — the same seam pattern
  as the four closures above.
- **`interval_start` representation:** the port's
  `CandidateInjection::interval_start` is `Option<OffsetDateTime>`; a
  setpoint interval that cannot convert to a representable UTC instant
  rejects at step 2 (`TIMESTAMP_MALFORMED`) — a derived injection never
  silently loses its interval (§10.3).
- **Attempt timestamps:** the gate stamps `started_at == finished_at` from
  the caller-supplied gate clock (deterministic/replayable); the wall-clock
  runtime of the evaluation is carried by the `PowerFlowResult.runtime`
  itself. A finer attempt-timing split is a P4.06/P4.07 wiring concern.
- **Ramp baseline freshness bound:** the baseline's `observed_at` is
  recorded but no maximum-age bound is applied in this subtask — the STALE
  quality state is the telemetry layer's own freshness verdict (§10.4) and
  blocking it is the recorded policy (D-P4.04-3). A separate gate-side age
  bound would duplicate that decision; it is deferred to the P4.08 policy
  review with the quality table.

## 3. Review status and verification

- **Power-systems review sign-off: PENDING** (the standing status for this
  phase). A human power-systems review must confirm the decisions in §1 —
  in particular D-P4.03-1 (the lifecycle table), D-P4.04-3 (the accepted/
  blocked quality sets), D-P4.04-4 (the ramp-over-stated-duration
  semantics), and D-P4.05-2 (retry policy NONE) — before the ledger
  P4.03/P4.04/P4.05 checkboxes are ticked.
- **Not run (VERIFY-BUILD):** per the task constraint, `cargo build`,
  `cargo test`, `cargo clippy`, and `cargo fmt` were NOT executed for these
  files. The tests are written to compile against the recorded APIs
  (flexcore-grid-api P4.02 surface, flexcore-contracts frozen types); the
  first `cargo test -p flexcore-grid-api` run must confirm, and any fix
  must keep the decisions in §1 unchanged.
- **No frozen-schema change:** flexcore-contracts, flexcore-domain,
  flexcore-storage, flexcore-events, and all manifests are untouched; the
  Phase 3 rules are consumed via the recorded seams, not restated.
- **No feasibility-pass claim anywhere:** the pipeline produces evidence
  rows, rejections, aborts, and attempt records; the words pass/feasible
  appear only as "NO feasibility verdict is implied" statements and the
  per-check `Passed` outcome of step 1/2 validation rows.
