# P7.31 — Handover to Phase 8 (research): research profiles and worker contracts

Date: 2026-11-13. Subtask: P7.31 of Phase 7. Owner role: Architect.
Report trace: §5.8; §13.4; §23 Stage 8; §22.1 (ADR-010). Status:
**WRITTEN — awaiting the Phase 8 lead's confirmation** (the ledger's
verification method is "Phase 8 lead confirms the note is sufficient"; no
human confirmation exists yet, so nothing here is "accepted").

Inputs consumed: `plugins/profiles/research-gnn.profile.json`,
`research-marl.profile.json`, `plugins/profiles/README.md`,
`python/workers/contracts/worker_api.proto`,
`python/workers/worker_template/` (config/errors/health/handler/server),
`ledger/phase-07-plugin-sdk/{02_scope_freeze,04_seam_inventory,
14_15_16_signing_profiles_recomposition,17_18_workers_supervision,
19_20_36_config_health_path,05_06_manifest_resolution,07_08_lifecycle_registry}.md`,
`ledger/phase-08-advanced-coordination/{PLAN.md,ledger_phase_8.md}`,
`apps/plugin-host/src/{broker,enforcement,health}.rs` (source), the kernel
crate (read-only reference), and `.agent/DECISIONS.md`.

---

## 1. What Phase 8 starts from (and what it must NOT assume)

Phase 8 (Advanced coordination research — ADMM/DMPC, GNN warm-start/
screening, MARL shadow, ANDES/HELICS dynamics; `ledger/phase-08-advanced-
coordination/PLAN.md` §1) starts from an SDK that ships **infrastructure
only**. The single most important handover fact:

> **No research provider of any kind exists in v0.5.0.** No GNN, MARL,
> DMPC/ADMM, ANDES, or HELICS code exists anywhere in this repository
> (`02_scope_freeze.md` §2 row "Research algorithm": "SDK IN / providers
> OUT"; §3 item P-5). A generic `Optimizer` WIT world and a generic gRPC
> worker contract exist; **a GNN provider contract does not**, and Phase 8
> must not read the former as the latter (the ledger's P7.31 risk, restated
> in §4 below).

## 2. The research profiles as ACTUALLY shipped

Restating the shipped state (`14_15_16_signing_profiles_recomposition.md`
§5; the profile files themselves), because the ledger's P7.31 risk is that
Phase 8 assumes more than shipped:

| Profile | Shipped state (v0.5.0) | Permission ceiling | Composes |
|---|---|---|---|
| `research-gnn` | Reviewed shell (`plugins/profiles/research-gnn.profile.json`), `control-authority: simulation-shadow` | `telemetry.read`, `asset.read` — **schedule.propose and every dispatch.* id are WITHHELD** (the profile's DEFAULT decision: even proposal permissions are withheld in the research shells) | `builtin.pandapower-grid`, `builtin.simulator-telemetry` ONLY. The research-worker/benchmark/shadow-logger selections are documented commented-out placeholders; no such manifest exists |
| `research-marl` | Reviewed shell, identical posture to `research-gnn` | `telemetry.read`, `asset.read` | Same two built-ins; same commented-out placeholders |

Properties Phase 8 inherits from these ceilings:

1. **No dispatch authority.** The ceiling withholds even `schedule.propose`
   and `dispatch.propose`; the enforcement gate refuses any request above
   the profile ceiling (`PERMISSION_ABOVE_PROFILE`,
   `apps/plugin-host/src/enforcement.rs`) and the broker re-refuses
   anything outside its closed vocabulary at grant AND check time
   (`apps/plugin-host/src/broker.rs::ALLOWED_PERMISSIONS`,
   `NEVER_GRANTED_PERMISSIONS`).
2. **The ceiling can only be raised by a reviewed, audited profile edit**
   with the P7.38 staged-rollout discipline (`research-gnn.profile.json`
   comment; `14_15_16` §5: a switch is an explicit, audited operation
   through the P7.07 lifecycle — every step gated, DEFAULT decision D-3).
   A Phase 8 experiment that needs proposal permissions changes the
   profile deliberately, never silently.
3. **Control-authority mode is untouched by profiles.**
   `control-authority: simulation-shadow` is descriptive; the mode itself
   is a deployment/per-site setting no profile can raise (AGENTS.md §4;
   `vpp-operator.profile.json` carries the same posture).

## 3. The worker contracts Phase 8 reuses (and must not fork)

The Class C worker base is `python/workers/contracts/worker_api.proto`
(`flexcore.worker.v1`) plus the template `python/workers/worker_template/`
(config, errors, health, handler, server, echo example). Phase 8 (P8.04)
extends this contract; it does not invent a parallel one (Phase 8 PLAN.md
non-goals: "Research workers reuse the Phase 7 gRPC worker template and
contracts (P8.04); if a contract gap is found, it is recorded as a Phase 7
follow-up").

What Phase 8 reuses as-is:

| Template feature | Where | Phase 8 use |
|---|---|---|
| Generic envelope: `payload_type` discriminator + opaque typed payload | `ExecuteRequest/ExecuteResponse` | Each research module carries its domain payload (GNN warm-start candidates, MARL actions, ADMM iterations) under a NEW `payload_type` (e.g. `flexcore.research.forecast_request.v1`), never by mutating the envelope |
| §9.2 provenance block | `ResultProvenance` (worker/model/solver version, input snapshot id, termination status, diagnostics, execution_ms) | Mandatory on every research OK result; Phase 8's §4 data contracts (ADMM/GNN/MARL result extensions) EXTEND provenance, they do not replace it |
| Determinism fields (the P7.42 keys) | `DeterminismMetadata`: `seed`, `request_id`, `idempotency_key`, `deterministic`, `seed_supported` | Phase 8's reproducibility subtask (P8.20) keys on exactly these: a worker that cannot honour a seed reports `seed_supported = false` (honest admission, not failure) and is EXCLUDED from the replay equivalence claim; `execution_ms` is advisory and excluded from comparison (timing is not a result) |
| Lifecycle + supervision vocabulary | `WorkerHealthState` (STARTING/READY/DRAINING/STOPPING), `Cancel`, `Shutdown` with grace | The P7.18 supervisor contracts (timeouts, retries only on idempotent calls, crash isolation, backpressure) apply unchanged to research workers |
| Error vocabulary | `WorkerErrorCode` (INVALID_REQUEST, UNSUPPORTED_CONTRACT_VERSION, UNSUPPORTED_PAYLOAD_TYPE, UNAVAILABLE, INTERNAL, CANCELLED, CONFIG_INVALID) | Research workers refuse unknown payload types and version skew structurally, never best-effort |

Contract-status honesty (inherited, not resolved by this handover): the
proto is WRITTEN-UNLINTED — `buf lint`/`buf breaking` have never run
against it and its registration into the `proto/` buf module is the
recorded activation step (`worker_api.proto` header; §7 below). Phase 8
extends the contract only after that registration exists.

## 4. The proposal-only boundary restated, with its EXACT enforcement points

ADR-010 (§22.1): ML/RL outputs are proposals only and cannot dispatch.
This is enforced in code at these points — Phase 8 adds no new exemption
and needs none:

| # | Enforcement point | Mechanism | Source |
|---|---|---|---|
| 1 | Manifest parse (kernel) | `NEVER_GRANTED_PERMISSIONS` (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) rejected at parse time; `PROTECTED_KERNEL_REPLACE` refused (`crates/flexcore-kernel/src/manifest.rs`; `05_06_manifest_resolution.md` refusal table) | kernel (read-only this wave) |
| 2 | Resolver | Missing/incompatible/cyclic/conflicting providers refused: `MISSING_PROVIDER`, `UNSUPPORTED_API_VERSION`, `CYCLIC_DEPENDENCY`, `CONFLICTING_PROVIDERS`, `SELECTION_UNRESOLVED` (`05_06_manifest_resolution.md` §refusals) | P7.06 |
| 3 | Broker grant time | Re-refusal of never-granted ids (`PermissionNeverGrantable`), unknown ids, over-broad (undeclared) requests (`PermissionNotDeclared`); `dispatch.validate.request` can carry ONLY the send-only scope targeting `protected-validator` (`broker.rs::validate_request_target`, `PROTECTED_VALIDATOR_TARGET`) | P7.11 |
| 4 | Broker check time | Per-call re-check — no cached ambient authority; never-granted ids refused again at check (`broker.rs::check_inner`) | P7.11 |
| 5 | Enforcement gate | `manifest permissions ∩ profile ceiling`; above-ceiling refusals (`PERMISSION_ABOVE_PROFILE`); vocabulary-drift refusal (`PERMISSION_VOCABULARY_DRIFT`); the enforced set is FROZEN at instantiation — later changes can only tighten (enforcement.rs header) | P7.12 |
| 6 | The never-granted set itself | `dispatch.authorize` / `dispatch.issue` do not exist as grantable ids in ANY manifest, profile, or broker call — the vocabulary contains no authorization id at all | P7.11/P7.12 |
| 7 | The forward boundary | `request_validation_forward` returns `()` — a verdict cannot flow back through a signature that has no verdict in it; the protected validation gate (§5.7) is never substituted | P7.11 |
| 8 | Path protection (P7.36) | A plugin-proposed command cannot be replayed, reordered, or used after expiry; duplicate delivery through the broker is idempotent — proposals enter the protected gate as first-class tracked candidates, still with zero authority | P7.36, `19_20_36_config_health_path.md` |
| 9 | Audit | Every grant, use, denial, enforcement verdict, and refusal is audited (audit-or-refuse); P7.33 adds per-plugin/per-session tamper-evidence over these records | P7.11–P7.13, P7.33 |

The one-line version for Phase 8 (§13.4): **research plugins return
proposals — schedule, candidate setpoint vector, ranking, forecast
distribution, uncertainty, diagnostics — and never an authorization
token. A proposal gains no authority from having been proposed.**

## 5. The seam table Phase 8 consumes

| Seam | Contract (source of truth) | Class | Phase 8 consumer |
|---|---|---|---|
| Optimizer | `crates/flexcore-optimization-api/src/port.rs` (`capabilities()` → `SolverCapabilities`, `solve(problem, config)`); WIT world `wit/flexcore-optimizer@1.0.0` | A (swap proven by P7.22) / C via worker template | ADMM/DMPC provider behind the contract (P8.07–P8.09); GNN warms the solver, never replaces it (PLAN.md §5) |
| GridModelProvider | `crates/flexcore-grid-api/src/port.rs` (`run_power_flow`, `evaluate_dispatch`; AC power flow, NOT AC-OPF); WIT world `wit/flexcore-grid@1.0.0` | A / C | Research grid-model providers (P8.16 dynamics stay OUT of this seam — dynamic results are scenario artifacts, never dispatch inputs) |
| TelemetrySource | `wit/flexcore-telemetry@1.0.0`; the closed `telemetry.read` capability | A/C | Research data access under the `research-*` ceilings (data-level tenant filtering is P7.39, cross-referenced below) |
| Class A registry | P7.08 built-in provider registry (the `builtin.*` manifests the research profiles already select) | A | The two built-ins the research shells compose today; research providers enter through NEW manifests, never by editing a built-in |
| gRPC worker path | `worker_api.proto` + `worker_template/` + P7.18 supervision | C | Every Phase 8 Python worker (ADR-004: Python isolated to workers) |
| Kernel lifecycle | `crates/flexcore-kernel/src/lifecycle.rs` (state machine, drain-before-replace, audited transitions) | protected kernel | Long-running research jobs: start/drain/replace through the audited lifecycle; no forced stop, no bypass |
| Host ↔ platform transport | The recorded P7.20 gRPC seam + the P7.34 trust boundary (mTLS, service identity; `apps/plugin-host/src/transport_trust.rs`) | recorded seam | Research results cross the trust boundary as UNTRUSTED INPUT; the platform validates each result (schema, §9.2 provenance, tenant scope) |
| Health | `apps/plugin-host/src/health.rs` (closed vocabulary, never-ready-before-first-observation) | implemented | Research worker/host health surfaces, consumed by the P7.25 console workspace |
| Audit | P7.11 broker/enforcement audit ports + the P7.33 chain (`apps/plugin-host/src/audit_chain.rs`); the authoritative store is the platform's append-only audit store (migration 0008 + `audit_read.rs`) | local + platform | Research-action audit trails: grants, uses, denials, and every proposal forward are tamper-evident locally and authoritative in the platform store |

Cross-references Phase 8 must honour: P7.37 (the §17.5 safety-invariant
map — every research proposal passes the same invariant gates) and P7.39
(cross-component isolation and DATA-LEVEL tenant filtering — the
`telemetry.read` grants of the research shells are tenant-scoped at the
data, not only at the call edge).

## 6. What is NOT handed over (the non-deliverables)

- **No research provider exists.** No GNN, MARL, ADMM/DMPC, ANDES, or
  HELICS code; no `research.*` manifest; no benchmark suite. All of it is
  Phase 8 (PLAN.md §1; `02_scope_freeze.md` §3 P-5).
- **No GNN/MARL result contract exists.** The envelope above is generic;
  the ADMM/GNN/MARL result extensions are P8.04's to define (Phase 8
  PLAN.md §4).
- **No benchmark cases or baselines.** §19.3 extension and the frozen
  baselines are P8.06/P8.19.
- **No research algorithm code in the SDK, ever as a plugin exemption:**
  the SDK provides no research code, and no Phase 8 module may bypass the
  §4 enforcement points (ADR-005: protected kernel non-replaceable; the
  MARL worker's no-live-dispatch property is PROHIBITED, not deferred —
  Phase 8 PLAN.md non-goals).
- **No field authority:** the entire line is simulation/shadow;
  per-feature qualification is Stage 9 (Phase 8 PLAN.md §1; P8.29 records
  that qualification is NOT granted).

## 7. Open items Phase 8 inherits (from the closed Phase 7 records)

| Item | Where recorded | What Phase 8 must do |
|---|---|---|
| wasmtime 50.0.1 never built; CVE rows re-check at implementation time | `03_sdk_license_audit.md`; `40_toolchain_pinning.md`; `apps/plugin-host/Cargo.toml` D4 note | First human build records the `cargo check` outcome per the pinning policy before any research component is hosted |
| rustls/rustls-pemfile UNPINNED-REVERIFY (edge precedent) | `06_07_identity_channel.md` §5; `TRANSPORT_WIRING_SEAM` in `transport_trust.rs` | If research workers need the host↔platform transport, the composition owner adds the recorded dependency set with D4 notes |
| `worker_api.proto` unlinted, unregistered in the `proto/` buf module | `worker_api.proto` header | Register/lint BEFORE P8.04 extends the contract |
| Bindings generation is a human `wit-bindgen` run (`generated-bindings` feature) | `wit/TOOLCHAIN.md`; `40_toolchain_pinning.md`; `apps/plugin-host/src/bindings.rs` | Research WASM components require the materialised bindings; the default build fails closed at the seam |
| Kernel-vocabulary mirror (alignment seam) | `10_11_plugin_host_broker.md` §6; `broker.rs`/`enforcement.rs` headers | If the kernel vocabulary changes, the mirror update is deliberate (fail-closed in both directions until then) |
| Per-call duration histogramming deferred to the metrics pipeline | `19_20_36_config_health_path.md` D-P7.20-4 | Research benchmarking (P8.19+) consumes the recorded gRPC seam when the metrics pipeline lands |

## 8. Lift-ready table for the Phase 8 `PLAN.md` seam column

Phase 8's component table (PLAN.md §3) has a seam/contract column; these
rows lift directly:

| Phase 8 component | Starts from (Phase 7 seam) | Contract artifact | Boundary it inherits |
|---|---|---|---|
| ADMM/DMPC provider | Optimizer seam, Class C gRPC worker | `worker_api.proto` envelope + new `payload_type`; `SolverCapabilities` declarations | Proposals only (§4); termination/residuals honest; centralized fallback (P8.08) |
| GNN worker | Optimizer seam as WARM-START source; `research-gnn` profile shell | new GNN result extension of `ResultProvenance`; `seed_supported` honoured | Warm-starts the solver, does not replace it; screening rankings are proposals; ceiling `telemetry.read`/`asset.read` until a reviewed profile change |
| MARL worker | `research-marl` profile shell; CTDE in isolated Python worker | new MARL result extension; `DeterminismMetadata` for replay | Shadow only; safety-filter outcome recorded; NO live-dispatch code path (prohibited, not deferred) |
| Dynamic simulator worker | Scenario-artifact channel (NOT the GridModelProvider dispatch seam) | scenario artifact with model references | Steady-state power flow ≠ transient stability evidence (ADR-012); results never dispatch inputs |
| Co-simulation federation | HELICS time-sync only | federation config artifacts | No federate physics claims; HELICS is not the power-system model |
| Independent validation harness | Separate ownership from the modules it validates; re-checks capability, AC power flow, SOC/ramp/duration, command invariants | Phase 8-owned harness | Never trusts the researcher; consumes §9.2 provenance, verifies independently |
| Benchmark set + baselines | `benchmarks/`, `scenarios/` | versioned artifacts citing contract versions | Every evaluation cites the artifact version; not-tested list mandatory |

## 9. Acceptance-criteria position (honest)

Ledger P7.31 exit check: "Phase 8 can start from the note without
re-deriving the worker contract; the proposal-only boundary is stated;
non-deliverables are explicit." §3–§6 above are that note; §4 states the
boundary with its code-level enforcement points; §6 lists the
non-deliverables. What remains open, honestly: the Phase 8 lead's
confirmation is pending; the worker contract is unlinted (§7); and every
"implemented" claim this note cites is code-level only — awaiting the
human build/test gate recorded in the respective subtask records. Nothing
here is "verified" in the AGENTS.md rule 7 sense.
