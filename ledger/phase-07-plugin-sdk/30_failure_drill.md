# P7.30 — Failure-mode drill (plugin host, workers, registry): procedure + EMPTY drill report

Date: 2026-11-13. Subtask: P7.30 (`ledger_phase_7.md` §P7.30). Owner role:
SDK engineer. Report trace: §19.2(6) fault set; §21.2 (failure modes and
recovery); §21.3 (correlation). Inputs: P7.07 lifecycle, P7.10 host,
P7.16 recomposition, P7.17/P7.18 worker template + supervision, P7.19
config drift, P7.33 audit-integrity host-kill survival, P7.36 path
protection, P7.13/P7.23 negatives.

## 0. Run-status honesty — READ FIRST

**The drill has NOT been run.** Every deliverable this wave is WRITTEN,
NOT BUILT, NOT RUN (the loop bans hold; the delivery loop never executed
any cargo command, never started a worker process, never started the
plugin host, never injected a fault). **Every measured cell in the §3
report table is deliberately EMPTY (UNRUN — human lab input).** No pass,
no recovery time, no data-loss count, no duplication finding is claimed
anywhere in this document. The per-fault coverage mapping below is a
DESIGN statement about which unit suites already cover each fault's
mechanics — it is not evidence that the drill ran, and the drill does
NOT re-run those suites (no re-runs, no drift: the drill adds ONLY the
integration layer, §2).

## 1. What the drill covers

One combined drill over the plugin-layer fault set of §19.2(6) as it
manifests on the plugin host (`apps/plugin-host/`), the Class C gRPC
workers (`apps/worker/src/supervision.rs` + the worker template), and
the plugin kernel/registry (`crates/flexcore-kernel/`). The ledger
P7.30 step-1 fault list and the recorded P7.33/P7.36/P7.16 seams are
merged into the single fault table below; each fault names the unit
suite that already covers its MECHANISM (cited, not re-run) and the
integration question the drill answers that no unit test can.

| # | Fault | Mechanism covered by (suite; cited, NOT re-run) | Integration layer the drill adds |
|---|---|---|---|
| F1 | Worker crash mid-call (Class C gRPC worker dies during `Execute`) | `apps/worker/tests/supervision_p7_18.rs` (crash → `FAILED`, removed from ready set, pool `DEGRADED`, audited); `crates/flexcore-kernel/tests/lifecycle_p7_07.rs` (`crash()` edges, `failed_from` evidence) | End-to-end: a crashed worker mid-call against a live schedule request — is the in-flight call cancelled visibly, is the request never silently lost or duplicated, does the platform schedule degrade instead of hanging |
| F2 | Supervisor timeout exhaustion (worker answers late until retries are exhausted) | `apps/worker/tests/supervision_p7_18.rs` (`record_timeout` → audited failure + bounded retry decision; non-idempotent calls never retried) | End-to-end: timeout chain under a real gRPC transport — the recorded failure surfaces, the call is not retried past the bound, and no schedule is issued from a partial result |
| F3 | Backpressure overflow (in-flight cap exceeded) | `apps/worker/tests/supervision_p7_18.rs` (`acquire` refuses with the named `BackpressureRefused`, audited with depth; never queued unboundedly) | End-to-end: saturation under live load — refusals are visible to the caller, nothing is silently dropped, and the audit trail reconciles accepted vs refused |
| F4 | Plugin host process kill mid-drain (the P7.33 recorded seam — host-kill survival) | `apps/plugin-host/tests/audit_integrity_p7_33.rs` (host kill mid-write: no completed-decision audit event lost, no partial event presented as complete); `crates/flexcore-kernel/tests/plugin_path_protection_p7_36.rs` (sequence checkpoint survives restart; replayed request after restart refused) | End-to-end: kill the host process during an active drain — the operator-visible state after restart matches the last fully-recorded state, the sequence checkpoint restores, and nothing resumes on trust |
| F5 | Lifecycle deadline abuse (drain deadline exceeded / a plugin that never quiesces) | `crates/flexcore-kernel/tests/lifecycle_p7_07.rs` (`DrainDeadlineExceeded` → stop refused fail-closed, incumbent restored to `Ready`, refusal audited; no forced stop) | End-to-end: a hostile/stuck plugin holding the drain past the deadline — the composition stays at its last fully-applied state and the operator decision path is reachable |
| F6 | Checkpoint corruption (path-protection sequence checkpoint damaged on disk) | `crates/flexcore-kernel/tests/plugin_path_protection_p7_36.rs` (corrupt tail truncated and COUNTED via `corrupt_tail_truncated`; foreign file refused `CHECKPOINT_FILE_FOREIGN`; never silent) | End-to-end: real on-disk corruption injected between host restarts — the counted loss matches the reconciled request history; nothing replays past the last durable record |
| F7 | Profile-switch partial failure (kernel recomposition aborts mid-switch) | `crates/flexcore-kernel/tests/recomposition_p7_16.rs` (`StepRefused` → composition stands at the last fully-applied state, abort audited, recorded active profile unchanged; restart fallback) | End-to-end: a switch aborting mid-drain against live providers — departing plugin state, surviving provider state, and the restart-fallback recovery are all consistent |
| F8 | Manifest/config drift mid-flight (configuration resolved against a manifest version the running instance was not instantiated against) | `crates/flexcore-kernel/tests/plugin_config_p7_19.rs` (`CONFIG_DRIFT` version-anchored refusal; `UNKNOWN_PROPERTY`; secret-value refusals); `crates/flexcore-kernel/tests/manifest_p7_05.rs` (validator negatives) | End-to-end: a manifest updated while an instance runs — the running instance keeps its resolved configuration, the drift is refused at the next resolution, and re-instantiation goes through drain-before-replace |
| F9 | Registry unavailable (plugin registry/host not composable at startup) | `apps/plugin-host/tests/host_p7_10.rs` (`ComponentUnconfigured` — an unconfigured host refuses to run, never idles permissively); `crates/flexcore-kernel/tests/registry_p7_08.rs` | End-to-end: a missing/unavailable registry at platform start — startup fails closed with a named error and no partial composition is activated |
| F10 | Plugin crash mid-schedule (a Class B component crashes inside a schedule computation) | `apps/plugin-host/tests/sandbox_p7_13.rs` (hostile-component suite: limit exceeded → named error, instance torn down, host process survives); `crates/flexcore-kernel/tests/lifecycle_p7_07.rs` (crash edges from any non-terminal state) | End-to-end: crash containment under a real schedule — the host survives, the instance is torn down, health degrades (P7.20), and no partial result enters the protected path |
| F11 | Malformed plugin result (forged or structurally invalid result from a compromised host/plugin) | `apps/plugin-host/tests/untrusted_plugin_p7_23.rs` + `apps/plugin-host/tests/transport_trust_p7_34.rs` (compromised-host results rejected and reported, never acted on) | End-to-end: a malformed result injected into the live path — refused with the named error, audited, and the host reported failed; nothing downstream consumes it |
| F12 | Oversized plugin result (output above the declared `max-output-mb`) | `apps/plugin-host/tests/sandbox_p7_13.rs` + `apps/plugin-host/tests/health_p7_20.rs` (`OutputLimitExceeded` → instance torn down, `DEGRADED_OUTPUT_LIMIT` latched) | End-to-end: oversized output under load — the call is refused, nothing partially enters the path, and health recovery is observable |
| F13 | Profile with a missing plugin (selection names a manifest not in the composition) | `crates/flexcore-kernel/tests/profile_p7_15.rs` (`PROFILE_SELECTION_UNRESOLVABLE`) + `crates/flexcore-kernel/tests/resolve_p7_06.rs` (`MISSING_PROVIDER`) | End-to-end: activating a profile that references a missing plugin — activation is refused before anything starts; no half-composed profile runs |

## 2. Drill procedure (how each cell is measured — the executable spec)

1. **Environment.** Lab host; the plugin stack built per the P7.28 exit-gate
   run book (`28_exit_gate_run_record.md`): one WASM component (one Class B
   host process), one gRPC worker (Class C), the kernel composition root,
   the published profile set. Record versions BEFORE any fault: git SHA,
   FlexCore build, wasmtime pin (P7.40), worker template version, plugin
   versions, WIT world pins, OS, hardware. Simulator-only or unit-test-only
   runs are NOT acceptable as the drill record (the ledger risk row:
   "drill run only with polite plugins" — the drill MUST include at least
   one hostile/stuck plugin and one oversized-output plugin, taken from the
   P7.13/P7.23 hostile fixtures).
2. **Baseline.** Start a clean run. Record: request sequence numbers per
   plugin/channel (`PluginPathVerifier::last_accepted`), audit event counts
   on every sink (lifecycle, broker, enforcement, health, supervision),
   and the composition plan currently active. The P7.36 reconciliation
   identity holds at baseline: every accepted request has exactly one
   checkpoint record; every completed decision has exactly one audit chain.
3. **Per fault.** Inject the fault once, with the injection instant
   recorded from the operator's log; run a fixed observation window;
   remove the fault; allow recovery; then:
   - **Detection time** = first audit/log line naming the condition MINUS
     the injection instant. Timestamps come FROM THE LOGS/audit trails
     with correlation ids (§21.3) — never from memory.
   - **Behaviour** = the named outcome observed (refused / failed /
     degraded / fallback / torn-down), quoted from the named error table
     (`MANIFEST_*`, `LIFECYCLE_*`, `SEQUENCE_*`, `BackpressureRefused`,
     `HostError::*`, `DEGRADED_*`/`FAILING_*`) by correlation id.
   - **Recovery time** = last audit line of the recovery transition
     (drain completed, health returned, checkpoint restored, pool
     reinstated) MINUS the fault-removal instant.
   - **Data loss (records)** = count reconciliation: accepted requests
     baseline + injected − refused − checkpoint-lost
     (`corrupt_tail_truncated`) = processed; completed decisions vs audit
     records (the P7.33 identity: no completed decision without its audit
     event, no partial event presented as complete).
   - **Command-or-request issued-or-duplicated** = grep the audit trails
     for duplicate sequence admission (a request admitted twice under one
     correlation id would mean `SEQUENCE_REPLAYED` did not fire), for
     orphaned commands (any `ISSUED` without its lifecycle counterpart —
     noting v0.5.0 is simulation/shadow only: no field dispatch exists,
     the platform-side counterpart is the command-lifecycle record), and
     for duplicated proposal envelopes under one sequence.
4. **Independent witness (human step).** A second engineer re-runs TWO of
   the faults from this procedure alone and reproduces the recorded
   outcomes (the ledger verification method). Which two is the witness's
   choice.
5. **Fix loop.** Any fault causing silent data loss or a duplicate/orphaned
   command/request is fixed and re-drilled until clean (ledger step 3).

## 3. Per-fault REPORT table (ALL MEASURED CELLS EMPTY — UNRUN, human lab input)

The P6.28 drill-report pattern (`ledger/phase-06-edge-protocol/
26_27_28_docs_ci_drill.md` §3.3) is reused verbatim: the measured cells
are EMPTY until a human lab run fills them from actual output. The
"mechanism suite" column is a citation for traceability — the drill does
not re-run those suites and adds no unit-level claim.

| Fault | Mechanism suite (cited, not re-run) | Detection time (measured) | Behaviour (observed) | Recovery time (measured) | Data loss (records) | Command/request issued-or-duplicated |
|---|---|---|---|---|---|---|
| F1 Worker crash mid-call | `supervision_p7_18.rs` | (EMPTY — UNRUN) | | | | |
| F2 Supervisor timeout exhaustion | `supervision_p7_18.rs` | (EMPTY — UNRUN) | | | | |
| F3 Backpressure overflow | `supervision_p7_18.rs` | (EMPTY — UNRUN) | | | | |
| F4 Plugin host kill mid-drain | `audit_integrity_p7_33.rs`, `plugin_path_protection_p7_36.rs` | (EMPTY — UNRUN) | | | | |
| F5 Lifecycle deadline abuse | `lifecycle_p7_07.rs` | (EMPTY — UNRUN) | | | | |
| F6 Checkpoint corruption | `plugin_path_protection_p7_36.rs` | (EMPTY — UNRUN) | | | | |
| F7 Profile-switch partial failure | `recomposition_p7_16.rs` | (EMPTY — UNRUN) | | | | |
| F8 Manifest/config drift mid-flight | `plugin_config_p7_19.rs`, `manifest_p7_05.rs` | (EMPTY — UNRUN) | | | | |
| F9 Registry unavailable | `host_p7_10.rs`, `registry_p7_08.rs` | (EMPTY — UNRUN) | | | | |
| F10 Plugin crash mid-schedule | `sandbox_p7_13.rs`, `lifecycle_p7_07.rs` | (EMPTY — UNRUN) | | | | |
| F11 Malformed plugin result | `untrusted_plugin_p7_23.rs`, `transport_trust_p7_34.rs` | (EMPTY — UNRUN) | | | | |
| F12 Oversized plugin result | `sandbox_p7_13.rs`, `health_p7_20.rs` | (EMPTY — UNRUN) | | | | |
| F13 Profile with a missing plugin | `profile_p7_15.rs`, `resolve_p7_06.rs` | (EMPTY — UNRUN) | | | | |

## 4. Acceptance rule (recorded)

**Any fault causing silent data loss or a duplicate/orphaned
command-or-request blocks the exit gate until fixed and re-drilled**
(ledger P7.30 step 3 and the P6.28 acceptance rule, restated). Until the
table in §3 is filled from actual lab output, zero-faults claims are
forbidden; the drill report being EMPTY is the honest current state.
"Recovery times recorded as measured values" (the ledger acceptance item)
means the recovery-time column is filled from log/audit timestamps, not
estimated — an EMPTY cell means the ledger acceptance criterion for
P7.30 is NOT yet met, and the P7.32 retrospective must record P7.30 as
open on exactly that ground.
