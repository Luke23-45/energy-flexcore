# P8.31 — Research-worker tenant isolation and scoped data access

Date: 2026-11-14. Subtask: P8.31 (`ledger_phase_8.md` §P8.31). Owner
role: Security engineer + research lead. Report trace: §17.4
(cross-tenant leakage); §16.2 (multi-tenancy); §5.7. Inputs: P8.04
contracts (`04_worker_contracts.md`); P7.11 broker; P7.12 matrix; the
Phase 7 pattern extended BY CITATION:
`crates/flexcore-kernel/src/tenant_scope.rs` (P7.39 — the host-stamped
`TenantScope`, echo-mismatch refusal, audited enforcement);
`crates/flexcore-kernel/src/{plugin_path_protection.rs, revocation.rs}`
(the digest/checkpoint disciplines mirrored); the research-side
implementation `crates/flexcore-research/src/tenant_isolation.rs` and
its integration tests
`crates/flexcore-research/tests/proposal_windows_and_tenant_isolation.rs`.

## 1. Status of this record — WRITTEN-UNCOMPILED, nothing run

The implementation and its test suite exist and were written under the
loop bans: nothing installed, nothing built, no cargo command executed,
no test run. Every expected value in the tests is hand-computed from
the recorded rules (the covers matrix, catalog keys, refusal names)
and stated in the test comments. NOTHING here is evidence that the
crate compiles or that any test passes; the human build/test gate
(AGENTS.md rule 7) owns that evidence. The ledger tick means
*implemented, awaiting human build/test*.

## 2. The scoping rule, per worker (ledger step 1)

The rule (recorded verbatim in code as
`tenant_isolation::RESEARCH_WORKER_SCOPING_RULE` and enforced
structurally): **each worker request carries a host-stamped
tenant/site scope; training data, model artifacts, and inference
inputs are all scoped.** The scope is derived from the platform-
authenticated context by the HOST (`ResearchWorkerScope::
from_authenticated_context` — the only public constructor; fields
private), stamped onto the worker identity (`WorkerScopeStamp::
stamp_by_host`, FNV-1a 64 binding digest — the P7.36/P7.14 test-grade
digest honesty applies: NOT a security primitive; a production digest
is a DEPLOYMENT REQUIREMENT), and cross-checked at every enforcement
point — not only at the call edge.

| Worker family (closed set) | Scope consumed at | Scoped surfaces (ALL of them, per the ledger) |
|---|---|---|
| `admm_dmpc` (P8.07–P8.09) | every request | telemetry, topology, scenario inputs; training data (none today — the coordination baseline is not ML); model artifacts (none today) |
| `gnn` (P8.10–P8.12) | every request | training data, model artifacts, telemetry/topology inference inputs |
| `marl` (P8.13–P8.15) | every request | training data (episode sets), policy artifacts, telemetry/topology observation inputs |
| `andes` (P8.16–P8.17) | every request | scenario artifacts, model/case references, telemetry inputs |
| `helics` (P8.18) | every request | federation scenario artifacts, telemetry inputs |
| `portfolio_analytics` (P8.22) | every request | telemetry/topology reads behind the reporting views |

The data kinds are a CLOSED vocabulary
(`tenant_isolation::ResearchDataKind`: telemetry, topology,
training_data, model_artifact, scenario) — an unknown kind is the
named refusal `RESEARCH_DATA_KIND_UNKNOWN`, never guessed. The worker
families are a CLOSED vocabulary (`ResearchWorkerFamily`) — there is
no "generic" worker that could bypass the per-family table above.

## 3. Enforcement shape (what the code guarantees, and what it does not)

| Guarantee | Mechanism | Named refusal |
|---|---|---|
| Only the host can stamp a scope | Private fields; `from_authenticated_context` / `stamp_by_host` are the only constructors; the stamp binding digest recomputes on every enforcement step | `RESEARCH_WORKER_STAMP_INVALID`, `RESEARCH_WORKER_STAMP_BINDING_MISMATCH` |
| A worker cannot widen its own scope | `verify_worker_echo` requires the echoed scope to match EXACTLY (tenant AND site) | `TENANT_SCOPE_ECHO_MISMATCH` (name-aligned with the kernel's `TenantScopeError::ScopeEchoMismatch`) |
| Cross-tenant reads denied | `enforce_scoped_read`: stamp binding → record existence → covers check (the kernel narrowing rule mirrored: tenant exact; tenant-wide covers any site in the tenant; site-narrowed covers only the same site) | `TENANT_SCOPE_MISMATCH` (aligned with the kernel's `ScopeOutsideEnvelope` name so audit queries span both paths); unknown records fail closed as `RESEARCH_DATA_RECORD_UNKNOWN` |
| Every denial (and admission) audited | `scoped_read_audited` / `model_serving_audited` emit `TenantIsolationAuditRecord`s through the append-only `TenantIsolationAuditSink` port, timestamps from the injected clock (`validation::HarnessClock` — the crate's one clock vocabulary; `SystemTime::now()` nowhere) | audit kinds `RESEARCH_SCOPE_READ_REFUSED` / `RESEARCH_MODEL_SERVING_REFUSED` |
| Unapproved cross-tenant model serving refused | `ModelServingRegister`: artifacts registered with OWNER scope; serving outside the owner scope requires an append-only RECORDED approval (reason + actor required; duplicate refused; no revoke/un-approve — the kernel `UNREVOCATION_SEAM` honesty mirrored) | `RESEARCH_MODEL_SERVING_NOT_APPROVED`, `RESEARCH_MODEL_ARTIFACT_UNKNOWN`, `RESEARCH_APPROVAL_*` |
| Determinism | Catalog/register iteration `BTreeMap`-ordered; canonical forms fixed-format; closed key sets | pinned by tests |

**The recorded residual risk (honest, never hidden):** a recorded
approval admits cross-tenant SERVING; it does **not** treat WEIGHT
LEAKAGE — a model trained on tenant A's data can leak tenant A's
information through its weights even when serving to tenant B is
approved. That treatment belongs to the training-data/leakage controls
(P8.35) and the evaluation leakage rules (P8.02 §4); it is NOT solved
by this module, and no statement here claims otherwise (the ledger's
recorded risk: "a shared model artifact that leaks tenant A's data
through its weights with no treatment recorded"). The risk is recorded
in the module header and pinned by a test asserting the statement does
NOT claim leakage prevention.

**The RLS relationship (unchanged, mirrored from the kernel):**
research workers never touch database tables directly (ADR-007); all
platform data access flows through the platform's tenant-scoped
services where application authorization and PostgreSQL Row-Level
Security (§16.2, [R25]) remain the platform's defence in depth. This
module adds the WORKER-level guarantee only; RLS is not re-implemented,
weakened, or replaced.

## 4. The cross-tenant negative-test record (ledger step 2)

The suite lives in
`crates/flexcore-research/tests/proposal_windows_and_tenant_isolation.rs`
(WRITTEN-UNRUN; the ledger's verification method — "independent
re-run of the cross-tenant suite" — is the human build/test gate plus
a reviewer re-run). The suite, with its hand-computed expectations:

| Test | Hostile input | Expected outcome (hand-computed) |
|---|---|---|
| `cross_tenant_reads_are_denied_and_audited_for_every_data_kind` | worker stamped tenant-a/s1 reads tenant-b topology; reads a tenant-b key of the wrong kind; reads tenant-a tenant-wide training data under a narrowed stamp | `TENANT_SCOPE_MISMATCH`, `RESEARCH_DATA_RECORD_UNKNOWN`, `TENANT_SCOPE_MISMATCH`; 3 audit records, all `RESEARCH_SCOPE_READ_REFUSED` |
| `in_scope_reads_are_admitted_and_audited` | tenant-wide worker reads tenant-wide training data and tenant-a/s1 telemetry | admitted; 2 audit records, both `RESEARCH_SCOPE_READ_ADMITTED`-class |
| `cross_tenant_model_transfer_is_refused_without_a_recorded_approval` | tenant-b requests a model trained on tenant-a data | `RESEARCH_MODEL_SERVING_NOT_APPROVED`; after the host records the approval (reason + actor), the same request is admitted; audit order refusal → approval-recorded → admission |
| `the_worker_stamp_cannot_be_forged_or_widened` | widened echo; cross-tenant echo | `TENANT_SCOPE_ECHO_MISMATCH` both |
| In-module suite (`#[cfg(test)]` in `tenant_isolation.rs`, 14 tests) | covers matrix (7 hand-computed pairs), stamp determinism, catalog duplicate keys, approval reason/actor requirements, unknown-artifact fail-closed, the closed error-name table vs the enum | per-test named refusals |

The scoping rule is enforced at TRAINING-DATA SELECTION too, not only
at inference: training datasets are ordinary catalog records
(`ResearchDataKind::TrainingData`) read through the same
`enforce_scoped_read` path (the ledger's recorded failure mode —
"scoping enforced at inference but not at training-data selection" —
is closed by construction: there is one read path).

## 5. Contract recording and the recorded Phase 7 follow-ups (ledger step 4; gaps, never invented fixes)

The ledger's step 4 asks the scoping rule to be recorded "in each
worker's contract (P8.04)". This record and the code constants are the
recorded rule; the worker-contract ARTIFACTS themselves are NOT edited
in this subtask (the P8.04 contract files are outside this subtask's
file ownership, and ENTRY_NOTE boundary 3 forbids inventing parallel
contracts). Recorded honestly as follow-ups:

| # | Phase 7 follow-up | Owner / activation point |
|---|---|---|
| TF-1 | `python/workers/contracts/worker_api.proto` carries no tenant-scope field on the request envelope. The host-stamped scope today travels OUTSIDE the wire envelope (bound at the broker/host layer, the P7.39 shape). If a future deployment needs the scope ON the wire, that is a Phase 7 contract extension (recorded decision + buf governance per §10.5), never a parallel contract | Phase 7 contract owners; only on a recorded deployment need |
| TF-2 | The P8.04 worker contracts (`research.py` module tables / worker READMEs) do not yet cite the per-worker scoping rule of §2; adding the citation to each worker's contract documentation is a P8.04-record update, to be made by the P8.04 owner (or the P8.33 identity work, which touches the same files) | P8.04 owner / P8.33 |
| TF-3 | The kernel `TenantScope` mirror (`tenant_isolation.rs`) must be re-diffed against `crates/flexcore-kernel/src/tenant_scope.rs` at the first human build pass (the recorded alignment seam: if the kernel shape changed since 2026-11-14, the mirror update is deliberate and fail-closed in both directions until then) | First human build/test gate |

## 6. Reviewer verification procedure

1. Re-run the cross-tenant suite (the ledger's verification method):
   `cargo test -p flexcore-research` after the human build gate; the
   suite names are §4's table — any missing test name is a finding.
2. Confirm the closed vocabularies (worker families, data kinds, error
   names) round-trip and that the error-name table equals the enum
   (`error_names_from_enum` test) — a variant added without updating
   the table must fail the build.
3. Confirm by inspection that no scope constructor is reachable from
   request data: grep `from_authenticated_context` / `stamp_by_host`
   call sites — every call site must be host-side composition.
4. Confirm the weight-leakage residual is PRESENT (the test
   `the_residual_weight_leakage_risk_is_recorded_not_hidden`), i.e. no
   prose or constant claims leakage prevention.
5. Confirm every audit path emits on BOTH outcomes (admission and
   refusal) and that timestamps flow only from the injected clock
   (no `SystemTime` in `tenant_isolation.rs`).

## 7. Honest status

**WRITTEN-UNCOMPILED.** No cargo command was executed in this loop.
The cross-tenant negative-test record above is the WRITTEN suite with
hand-computed expectations, not a run record — there is no pass/fail
evidence in this file, and no run output cell is filled. The kernel
pattern was READ (tenant_scope.rs, plugin_path_protection.rs,
revocation.rs); the research module mirrors it by citation with the
alignment seam recorded in code
(`KERNEL_TENANT_SCOPE_ALIGNMENT_SEAM`); flexcore-kernel is untouched.
