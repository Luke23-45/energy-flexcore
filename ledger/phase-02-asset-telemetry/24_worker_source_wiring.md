# P2.24 — Worker source wiring: the simulator composed as the worker's `TelemetrySource`

| Field | Value |
|---|---|
| Subtask | P2.24/P2.31 follow-up — land the recorded composition wiring that closes the two coordinator blockers: `compose_ingestion` returning `None` (recorded in `25_exit_gate_runbook.md` §0) and the contract-suite instantiation against the real simulator (recorded in `27_ci_gate_inventory.md`) |
| Report trace | §8.1/§8.3 (worker as composition root), §9.1 (`TelemetrySource`), §6.3 (outbox-only publishing) |
| Inputs | P2.10 step 5 (worker task composition), P2.11 (the port), P2.13/P2.14/P2.15 (the simulator), P2.25 step 2 (registration = the key → ID mapping record) |
| Status | CODE LANDED, NOT RUN — this subtask is build-banned (`cargo` not executed); the first `cargo run -p flexcore-worker` against the registered stack records the OBSERVED outcomes in the P2.26 demonstration record |
| Scope note | Simulation-only 0.x. The simulator emits synthetic telemetry only; nothing here grants control authority or implies any safety, certification, or field-control property |

---

## 1. What landed

| File | Change |
|---|---|
| `apps/worker/src/main.rs` | `compose_ingestion` implemented (§2 below); the pull-side source bridge (`run_source_bridge`) spawned next to the ingestion task; composition errors abort startup (fail-fast) |
| `apps/worker/src/config.rs` | Four `FLEXCORE_*` variables (§3 below), matching the fail-fast/no-secret-logging conventions |
| `apps/worker/src/ingestion.rs` | Module-header wiring note updated (composition landed); additive `IngestionQueue::is_closed` so the bridge does not retry offers into a closed channel (a silent loss). Nothing else changed — the pipeline stays port-only |
| `apps/worker/tests/wiring.rs` | NEW: config parsing (absent → no ingestion configuration; present → full composition input; invalid values fail fast with the variable named), query-construction rejection cases, and a pure composition smoke test (the v2 scenario through the port: 48,960 canonical records, deterministic reproduction) |
| `apps/sim-runner/tests/contract.rs` | NEW: the reusable contract suite (`flexcore_ports::telemetry_source_contract`, UNCHANGED) instantiated against the real `SimulatorTelemetrySource` — full suite on the clean v2 scenario, plus runs with `out_of_range` and `source_invalid` enabled so the declared quality mapping is exercised non-vacuously (with non-vacuity assertions) |

## 2. How `compose_ingestion` composes (decision, made once)

With `FLEXCORE_SIMULATOR_SCENARIO` configured:

1. **Scenario load (all-or-nothing, P2.13).** The scenario file is read and
   validated whole; the optional master-seed override
   (`FLEXCORE_SIMULATOR_SEED`) is applied by rewriting only
   `seeds.master_seed` in the parsed JSON — an override is a
   replay-relevant change (P2.24) and is never a default. The optional
   horizon override (`FLEXCORE_SIMULATOR_HORIZON_TICKS`) bounds the query
   window; the default is the scenario's recorded horizon (full 24 h).
2. **Tenant and asset resolution from the registry (P2.25 step 2).** The
   tenant is resolved by its identity-provider reference
   (`FLEXCORE_SIMULATOR_TENANT_REFERENCE`, the registration uniqueness
   anchor) through the `TenantRepository` port; the scenario's asset keys
   are matched against the tenant's registered asset names through the
   `AssetRepository` port (the registration records the scenario key as
   the asset's name, P2.25 §2.4). Every scenario key must resolve — a
   missing key fails composition (never a silently reduced portfolio). The
   key → registry-ID mapping exists ONLY here and at registration, never
   in the scenario file (P2.13 §1). Read-only unit of work, dropped
   without commit.
3. **The port object and the query.** `SimulatorTelemetrySource::from_scenario`
   receives the validated scenario and the key → ID mapping; the query is
   built through `TelemetryQuery::new` over the registered tenant context,
   the mapped asset set, the horizon window
   (`start .. start + ticks × cadence`), and the full frozen quantity
   vocabulary (all nine quantities — the exit-gate run covers every
   registered asset's full quantity set, P2.25 §3).

Without the scenario variable the composition returns `Ok(None)` and the
worker starts WITHOUT the ingestion task with the recorded startup log —
never a silent start. Composition failures (unreadable/rejected scenario,
unregistered tenant, unmapped asset key, overflowed horizon) abort startup
with a named error.

**The source bridge.** The port is sync and pull-based (P2.14 step 5), so
the worker spawns a bridge task that pulls one frame at a time from the
concrete source and offers each to the pipeline's bounded `IngestionQueue`
(the shed path retries the SAME bytes with backoff — the pull already
consumed them from the deterministic stream, so re-pulling is impossible
and dropping would be a silent loss; `IngestionQueue::is_closed` ends the
bridge honestly when the pipeline's channel is gone). The bridge stops at
the horizon, at the shutdown flag, or on a source error; the pipeline's
own subscription happens inside `IngestionTask::run` as before.

**Port-only rule unchanged.** Only `main.rs` (composition root) names the
simulator crate; `flexcore_worker::ingestion` compiles against the
`TelemetrySource` port only; the dependency-rule checker still passes
(apps are unrestricted, the library crate list is untouched).

## 3. Configuration (all optional; documented defaults)

| Variable | Meaning | Default |
|---|---|---|
| `FLEXCORE_SIMULATOR_SCENARIO` | Scenario file path; SET = ingestion task composed and started | unset → no ingestion task (recorded honest log) |
| `FLEXCORE_SIMULATOR_TENANT_REFERENCE` | The registered tenant's identity-provider reference (required when the scenario is set) | unset + scenario set = startup error |
| `FLEXCORE_SIMULATOR_SEED` | Master-seed override (replay-relevant, P2.24) | the scenario's recorded seed |
| `FLEXCORE_SIMULATOR_HORIZON_TICKS` | Subscription horizon in ticks (1..=10_000_000) | the scenario's recorded horizon |

## 4. Source authentication status — unchanged, recorded

The wiring does NOT change what authentication is asserted:
`flexcore_worker::ingestion::SOURCE_AUTH_STATUS` remains the recorded
Phase 2 STUB (provider declaration + optional trusted-source-id check
only; no cryptographic source authentication). Real source
authentication is a later phase. The optional
`FLEXCORE_INGEST_TRUSTED_SOURCE_ID` knob continues to apply unchanged
(the composed simulator source declares
`sim://synthetic-portfolio-exit-gate` for the v2 scenario).

## 5. The contract-suite instantiation (closes the `27_ci_gate_inventory.md` follow-up)

The suite itself is UNCHANGED (it is generic over the port — that is the
point). The instantiation lives in `apps/sim-runner/tests/contract.rs`:
one suite run on the clean v2 scenario (`site-north-bess-1`, full 1440-tick
horizon) and one per mapped fault kind (`out_of_range` → SUSPECT,
`source_invalid` → BAD), each with a non-vacuity assertion that the fault
window really emitted the mapped canonical state and its declared source
code. The duplicate / out-of-order / conflicting fault kinds are
deliberately excluded from suite runs: they violate the arrival-order
contract BY DESIGN (that is what P2.09 dedup handles), and the suite
holds providers to the arrival contract the source respects outside those
faults.

**CI note (one line, for the workflow owner):** add
`cargo test -p flexcore-sim-runner --test contract` to the existing
`telemetry-contract` job (or a sibling step) — no other workflow change is
needed; `.github/workflows/ci.yml` was deliberately not touched here.

## 6. Verification method

This subtask ran no build (loop bans). The recorded verification sequence:
`python tools/verify_docs.py`, `python tools/validate_agent.py`, then
`cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test -p
flexcore-worker --test wiring`, `cargo test -p flexcore-sim-runner --test
contract`, and the P2.25 runbook steps 1–7 on the registered stack
(48,960 records, replay verdict pass).
