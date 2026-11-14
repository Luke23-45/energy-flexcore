# P8.04 — Research worker contracts: result, provenance, and the proposal-only boundary

Date: 2026-11-13. Subtask: P8.04 (`ledger_phase_8.md` §P8.04). Owner
role: Research lead + Rust engineer. Report trace: §13.4; §9.2; §5.7;
§22.1 (ADR-010); §7.4. Inputs: the P7.31 handover (`ledger/phase-07-
plugin-sdk/31_phase8_handover.md`); the P7.17 worker template
(`python/workers/worker_template/`); the Phase 7 contract
(`python/workers/contracts/worker_api.proto`); ENTRY_NOTE.md boundary 3;
the P8.02 scope freeze (`02_research_scope_freeze.md`).

## 1. Status of this record — WRITTEN-UNRUN

Everything below was written under the loop bans: nothing installed,
nothing built, no buf toolchain, no pytest run. The code and its test
suite exist and are expected to pass at the first human build/test pass;
no acceptance check that requires execution has been evidenced here.

## 2. The REUSE decision — no parallel contract (ENTRY_NOTE boundary 3)

Research workers reuse the Phase 7 worker contract and template, in
full. No parallel contract is defined, and none may be invented later: a
contract gap is recorded as a Phase 7 follow-up (§5), never fixed by
forking the schema.

| Reused artifact | What research workers get from it unchanged |
|---|---|
| `python/workers/contracts/worker_api.proto` (`FlexcoreWorkerService`: Check / Execute / Cancel / Shutdown, package `flexcore.worker.v1`) | The wire contract: versioned envelope, `payload_type` discriminator, §9.2 `ResultProvenance`, `DeterminismMetadata` (seed / request_id / idempotency_key), structured error vocabulary, cancellation and graceful shutdown |
| `worker_template/handler.py` | Envelope validation, correlation echo, determinism stamping from the REQUEST (P7.42), the `WorkerLogic` hook |
| `worker_template/config.py` | Fail-closed configuration (invalid value = fatal, missing key = recorded DEFAULT) |
| `worker_template/errors.py` | The structured error vocabulary (name-aligned with the proto `WorkerErrorCode`) |
| `worker_template/health.py` / `server.py` | The lifecycle state machine and the single grpcio seam (`server.require_grpc` — imports `grpc` INSIDE the function with the named ImportError `GRPC_DEPLOYMENT_REQUIREMENT`) |

What P8.04 ADDS is exactly what the proto delegates to the payload
schema ("the payload type discriminator — the agreement between caller
and worker about the payload's schema"): the per-`payload_type` result
agreements, the closed vocabularies, the in-code proposal-only boundary,
and the validation hook. Implemented as ONE additive module,
`python/workers/worker_template/research.py` (stdlib-only — `json` is
the only new import; NO new external dependency; grpcio remains
UNPINNED-REVERIFY in `python/workers/requirements.txt`, which is
UNTOUCHED, and nothing was installed).

## 3. The research result contract per module (ledger step 1)

Every result carries the §9.2 provenance base — `ResultProvenance`
(worker/model/solver version, input snapshot id, termination status with
the proto's own "ok alone is not sufficient" rule, diagnostics) extended
with the P8.32 model-artifact reference — plus a CLOSED per-module
result-key set. `flexcore.research.gnn_warm_start.v1` carries the model
version in provenance and the training provenance in the result keys.

| Module | payload_type | Result keys (closed set) |
|---|---|---|
| ADMM/DMPC (P8.07–P8.09) | `flexcore.research.admm_schedule.v1` | `result_kind`, `primal_residual`, `dual_residual`, `iteration_count` |
| GNN warm-start/screening (P8.10–P8.12) | `flexcore.research.gnn_warm_start.v1` | `result_kind`, `training_data_ref`, `topology_scope`, `uncertainty` |
| MARL simulation/shadow (P8.13–P8.15) | `flexcore.research.marl_shadow.v1` | `result_kind`, `policy_version`, `training_seed`, `environment_version`, `safety_filter_outcome` |

The result-kind vocabulary is the closed §13.4 set (`proposed_schedule`,
`candidate_setpoint_vector`, `ranking`, `forecast_distribution`,
`uncertainty_estimate`, `diagnostics`) and is NAME-ALIGNED with
`crates/flexcore-research/src/validation.rs::ResearchResultKind` (the
recorded seam: the Rust enum is the definition of record; the two move
together, pinned by tests on both sides). The payload types are CLOSED:
an unknown type is refused (`UNSUPPORTED_PAYLOAD_TYPE`), never guessed.

## 4. The proposal-only boundary, enforced IN CODE (ledger step 2)

The ledger requires the boundary as code, "not only in docs". The
mechanism, in `research.py`:

1. **Whitelist construction** — `build_research_result` accepts only
   keys inside the module's closed set; any other key is a structured
   refusal (`INVALID_REQUEST`). There is no "extra fields tolerated"
   path, so an authorization token has no key it could ride on through
   the builder.
2. **Recursive forbidden-key scan** — `assert_proposal_only` walks the
   whole result (dicts and lists, any depth) and refuses any key in the
   closed `FORBIDDEN_RESULT_KEYS` set (`authorization`,
   `authorization_token`, `approval`, `command_id`, `dispatch`, `grant`,
   …) with `ProposalOnlyViolation`, naming the KEY and PATH, never a
   value. This is the defense in depth for results assembled outside
   the builder.
3. **Enforcement before serialization** — the boundary adapter inside
   `handle_research_execute` runs the schema check, the scan, and the
   provenance check on the STRUCTURED result and only then serializes
   it (deterministically: sorted keys, compact separators — the P7.42
   replay property). A boundary-violating result is never serialized,
   never stamped, never wired.

On the Rust side the same boundary is structural:
`flexcore-research::validation` has no field any researcher verdict
could occupy (diagnostics are prose the harness never reads), the §13.4
vocabulary is a closed enum, and `ValidatedProposal` is sealed (private
seal field, no `Deserialize`, the only constructor is the harness's
gate-passing path). Acceptance wording "the response type cannot express
an authorization token (verified by type inspection)" is satisfied by
these closed-set/type-level structures and their (written, unrun) tests
— not by prose.

## 5. The validation hook (ledger step 3) and the buf gates (step 4)

**Hook.** `proposal_candidate_record` wraps an accepted result as the
PROPOSED-stage candidate record (closed key set; `lifecycle_state:
"PROPOSED"`; `authority` is a fixed statement string —
`PROPOSAL_ONLY_STATEMENT` — never a token). The record enters the
protected validation → feasibility → authorization path like any other
candidate (§13.1, §14.1). Physical re-validation is NOT implemented
here: it is the independent harness of P8.05
(`crates/flexcore-research/src/validation.rs`), and the
deliberately-infeasible-proposal rejection (the ledger's verification
method) is pinned by that harness's written test suite
(`gate_limit_violation_refuses_with_the_named_constraint`,
`gate_fail_closed_refuses_never_passes`) — WRITTEN-UNRUN.

**buf (step 4).** `buf lint` / `buf breaking` were NOT run — no buf
toolchain was executed in this loop, matching the P7.17 honest status
("WRITTEN-UNLINTED"). Recorded as Phase 7 follow-ups, NOT fixed here:

| # | Phase 7 follow-up | Owner / activation point |
|---|---|---|
| F-1 | Register `python/workers/contracts/worker_api.proto` with the `proto/` buf module (or a documented include path) so `buf lint` / `buf breaking` actually run on it — the activation step already recorded in the P7.17 README §4 | P7.27 CI activation / first human build pass |
| F-2 | Run `buf lint` / `buf breaking` over the research `payload_type` agreements once they are documented into the proto's payload-schema registry (the per-type schemas currently live in `research.py::MODULE_RESULT_FIELDS` and the concrete workers' READMEs) | P8.28 CI gates |

**Contract-gap analysis result (honest):** inspection found NO gap that
requires a schema change — the opaque-bytes + `payload_type` mechanism
carries the module-specific result fields by design, the §9.2 base and
the determinism fields already exist, and the proposal-only boundary is
enforceable on the payload side. This analysis is by inspection of the
proto and template, not by buf validation, and is therefore itself
UNVERIFIED until F-1 runs.

## 6. Verification method and status

- Type inspection of the closed vocabularies and the sealed/boundary
  structures: implemented as `python/workers/tests/
  test_research_contracts.py` and the `#[cfg(test)]` suites in
  `crates/flexcore-research/src/validation.rs` — WRITTEN, NEVER RUN
  (pytest never installed; no cargo command executed; loop bans).
- "Submit a deliberately infeasible research proposal and confirm
  rejection by the feasibility gate": the harness-side tests named in §5
  pin the refusal; the live gate run is a human build/test item.
- Honest status: **WRITTEN-UNRUN**. Nothing in this record is evidence
  that any worker ever served a request or that any test ever passed.
