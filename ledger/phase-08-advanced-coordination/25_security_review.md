# P8.25 — Security review of the research workers: untrusted code, data, and models

Date: **2026-11-14** (the dated security-review record required by the
ledger). Subtask: P8.25 (`ledger_phase_8.md` §P8.25). Owner role:
security engineer. Report trace: §19.2(7); §17.4; §5.7; §9.3. Inputs:
P8.11 (`11_gnn_inference_worker.md`), P8.14 (`14_marl_worker.md`), P8.16
(`16_andes_integration.md`), P8.03 (`03_research_license_audit.md`),
P7.29 (`ledger/phase-07-plugin-sdk/29_security_test_report.md` — the
structure this record mirrors). Reviewed material (READ in full or in
the cited sections, on 2026-11-14): `apps/worker/python/
{marl_worker,gnn_worker,andes_worker,portfolio_analytics}/**` (worker
sources, NOT their pytest suites beyond spot-checks of the pinned
assertions), `crates/flexcore-research/src/{validation,admm,dmpc}.rs`,
`python/workers/worker_template/research.py`,
`plugins/profiles/{sim-lab,research-marl}.profile.json`.

## 0. Run-status honesty — READ FIRST

**This is a review BY READING, not by running.** No worker was executed,
no hostile input was fired, no framework was installed, no scan ran (the
loop bans; ENTRY_NOTE.md). Dynamic verification — the ledger's
"independent re-run of the hostile-input cases" — is **the human gate**:
§4's case table is WRITTEN-UNRUN with EMPTY findings cells.
**"Zero open high/critical findings" is NOT claimable today and is NOT
claimed**: one HIGH finding is recorded below (F-1, a code-as-written
unsafe-load call), which BLOCKS the ledger's exit check until it is
fixed and re-tested at the with-framework activation gate. Every finding
states file+line evidence from the code read, or is recorded as a
review-gap item with an owner — none is invented, none is patched
silently.

## 1. Method and the boundary map (what was verified by reading)

The three ledger input classes — model files, training data, worker
outputs — were reviewed against the actual worker code. The
**proposal-only boundary** was verified by reading each worker's
in-code scan (all four implement the same recursive whole-tree key scan
against a closed forbidden-key set; ADR-010 posture):

| Worker | Proposal-only scan (file:lines read) | Forbidden-key set | Result-envelope posture read |
|---|---|---|---|
| GNN inference | `apps/worker/python/gnn_worker/gnn_inference_worker.py:730–750` (`assert_proposal_only`) | `FORBIDDEN_RESULT_KEYS` (13 keys, :189–204) | `proposal_only: true`, `authority: "none"`, mode-lock constant `MODE_LOCKED = "simulation-shadow"` (:79), scan applied in `main` BEFORE emission (:1035) |
| MARL shadow | `apps/worker/python/marl_worker/marl_worker.py:456–478` | same 13-key set (:214–229) | same posture (:583–585); scan in `main` before emission (:881); the shadow log travels INSIDE the envelope and is scanned with everything else |
| ANDES dynamic | `apps/worker/python/andes_worker/andes_dynamic_worker.py:381–401` | same 13-key set (:147–162) | same posture (:429–431); scan in `main` before emission (:684) |
| Portfolio analytics | `apps/worker/python/portfolio_analytics/analytics.py:130–156` (`assert_reporting_only`) | `DISPATCH_SHAPED_KEYS` (schedule/setpoint/dispatch-shaped keys, ~:110–127) | reporting-only refusal `REPORTING_ONLY_VIOLATION` at ANY depth |

Additional structural facts verified by reading (the review's baseline
assumptions, each cited):

- **Mode lock is a constant, not a setting** in all three Class C
  workers (`MODE_LOCKED = "simulation-shadow"`: gnn :79, marl :120,
  andes :116) — no request field can select or raise a
  control-authority mode (§5.7; a plugin can never select a mode).
- **Fail-closed transport**: one JSON document in, one out; malformed
  input → NOTHING on stdout, reason on stderr, exit 2 (gnn `main`
  :1016–1051; marl :861–897; andes :665–696).
- **Closed result-key sets** enforced per payload type
  (`enforce_gnn_result_schema` gnn :753–783;
  `enforce_marl_result_schema` marl :481–524; the Phase 7 template's
  `MODULE_RESULT_FIELDS` whitelist `python/workers/worker_template/
  research.py` ~245–263) — an unknown key is a structured refusal, not
  tolerated.
- **The independent validation harness refuses anything the gate does
  not pass**: stage order fixed, physical re-validation LAST through an
  injected port, and ANY port `Err` is the named refusal
  `PHYSICAL_REVALIDATION_SEAM_FAILED` — fail-closed, never a pass
  (`crates/flexcore-research/src/validation.rs` ~43, 314–326, 761–795;
  pinned by its own test ~1374–1386). This is the answer to the
  ledger's named risk "a research output that bypasses contract
  validation because it arrives over an internal channel": there is no
  worker-internal channel into the feasibility gate — every proposal
  enters through the same stage sequence regardless of producer
  (MARL/GNN/ADMM `proposal_for_first_step` →
  `ResearchProposal` → `ValidationHarness::validate`,
  `crates/flexcore-research/src/dmpc.rs` ~1227).

## 2. Input class 1 — MODEL FILES (the ledger's step 1)

**Allowed model formats, named in writing (the ledger requires this in
writing; the honest current list is short and conditional):**

| Worker | Model artifact | Format allowed TODAY | Evidence |
|---|---|---|---|
| GNN | `model_manifest.model_artifact_ref` → `torch.load(..., map_location="cpu")` | **NONE safely loadable as written** — see finding F-1 | `gnn_inference_worker.py:924–927` |
| MARL | `policy_manifest.policy_artifact_ref` → load site NOT YET WRITTEN (the with-framework path raises `PolicyArtifactError` before any load) | **NONE named yet** — the safe-load requirement is a recorded precondition (finding F-2) | `marl_worker.py:832–844` |
| ANDES | `case_path` → `andes.load(case_path, setup=True)` (ANDES-native case files parsed by the third-party tool) | ANDES case format, UNREVIEWED (finding F-3) | `andes_dynamic_worker.py:535` |

**Findings (each with file+line evidence; severities are code-reading
judgements, to be re-tested at the human gate):**

| # | Severity | Finding | Evidence | Required action / owner |
|---|---|---|---|---|
| F-1 | **HIGH (open — blocks the exit check)** | The GNN artifact load is an **unguarded `torch.load`**: `torch.load(validated["model_manifest"]["model_artifact_ref"], map_location="cpu")` — as written, the first with-framework build would deserialize the model artifact through torch's default (pickle-capable) loading path. The ledger's rule is explicit: arbitrary-code deserialization is forbidden and "a format that cannot be loaded without executing code is not an allowed format". NOT exploitable in this repository TODAY — the call is UNREACHABLE (no torch is installed or pinned; the guarded import `import_torch()` raises the named DECISION-POINT requirement first, gnn :263–273, 909) and no artifact exists — but the finding is against the CODE AS WRITTEN, which is what the with-framework activation would build | `gnn_inference_worker.py:924–927`; unreachability: :907–917 | **Blocks the with-framework activation until fixed and re-tested**: the P8.10/P8.11 owner must pin the allowed format in writing (a non-executing serialization, e.g. a weights-only/tensor-only format, with the exact loading call recorded) BEFORE the torch/PyG pair is pinned (P8.03 §4 DECISION-POINT), and the P8.12 human gate re-tests hostile/corrupt/oversized artifacts against the fixed load. This record makes NO version or API claim about torch loading parameters — the safe-loading mechanism is chosen at pin time from the pinned framework's documented surface, verified from its primary source |
| F-2 | MEDIUM (review-gap, prospective) | The MARL policy-artifact load site does not exist yet (`run_shadow_inference` raises `PolicyArtifactError` "UNREACHABLE ... never fabricates an action", marl :838–844). The with-framework implementation is human-gate work — which means the safe-load decision is STILL OPEN. No defect exists in code-as-written (there is no load call to be unsafe); the risk is that the gate's implementation could repeat F-1's shape | `marl_worker.py:800–844` | The F-1 required action applies verbatim to the MARL artifact load when it is written: allowed format named in writing first, load reviewed, hostile-artifact cases re-run (owner: P8.13/P8.14 owner + human gate) |
| F-3 | MEDIUM (review-gap) | ANDES case files are parsed by the third-party tool with no named allowed-format list and no in-worker artifact signing; a malformed, oversized, or hostile case file's behaviour is expected to surface in-band (`termination_status: "failed"` — `andes.load` failure caught in-band, andes :533–535 + the in-band failure contract in `build_result`), but that behaviour is UNVERIFIED (ANDES was never installed/run) | `andes_dynamic_worker.py:523–560`; header honesty table :76–105 | The P8.16 human gate runs the hostile-case cases (§4 N-7..N-9) and records the outcome; the allowed case-format list is recorded at the P8.16 activation. The P8.32 signing seam (below) is the intended long-term control |
| F-4 | INFO (seam, not a defect) | The model/policy artifact **signing seam is P8.32's** and is NOT implemented: manifest fields are declared IN-REQUEST (`model_manifest` / `policy_manifest` validated as non-empty strings) because the P8.20 artifact store and P8.32 signing do not exist yet — a caller could declare a false manifest; the recorded mitigations are the in-band scope flag (gnn :460–489) and the recorded decision visibility (marl filter report) | `11_gnn_inference_worker.md` §8 item 4; `14_marl_worker.md` §8 item 4 | P8.31/P8.32 owner (separate wave; recorded here as the referenced seam, NOT implemented) |

**Oversized-model and unexpected-ops handling (ledger step 1):** no
in-worker size cap exists for model artifacts (the declared-timeout cap
and the payload caps are request/output-side, not artifact-side) —
recorded as review-gap item G-1 (owner: P8.10/P8.13 at the artifact-store
activation; the P8.20 store is the natural enforcement point). No
in-repo evidence exists either way for how a framework handles
unexpected ops; the §4 cases (N-4/N-5) are the human gate's to answer.
Nothing is claimed.

## 3. Input class 2 — TRAINING DATA (the ledger's step 2)

- **In the workers:** training data appears only as the declared
  reference strings `training_data_ref` (required non-empty: gnn
  :602–604, marl :423–425) carried verbatim into outputs — no
  deserialization of training data exists in any worker (verified by
  reading; the workers parse NO dataset files). Malformed references
  are refusals at the request gate.
- **The record-level data validation (poisoned/malformed records must
  not silently corrupt the experiment record) is P8.35's deliverable**
  (training-data provenance/quarantine, wave 8a) and does NOT exist yet
  — recorded as review-gap G-2 with owner P8.35; this review does NOT
  test or claim it. The ledger's dependency line (P8.11, P8.14, P8.16)
  does not include P8.35 — the review can only record the seam.
- **Where poisoning would land today:** the GNN/MARL evaluation
  harnesses validate every input record's shape/finiteness/vocabulary
  before use (`gnn_evaluation.py::EvaluationError` refusals ~161+;
  `marl_worker/evaluation_harness.py::EvaluationHarnessError` ~125+;
  `shadow_log.validate_shadow_log` refuses gapped/invalid logs) — so a
  malformed record is a named refusal, but a WELL-FORMED poisoned
  record is indistinguishable from honest data at these layers. That
  limit is stated, not fixed (the quarantine layer is P8.35's).

## 4. Input class 3 — WORKER OUTPUTS (the ledger's step 3)

Verified by reading (each behaviour code-derived; the hostile-input
cases themselves are UNRUN):

| # | Case (WRITTEN-UNRUN) | Required outcome (code-derived) | Status | Findings |
|---|---|---|---|---|
| O-1 | Oversized worker output (payload above the 1 MiB cap) | Stage-1 refusal `PROPOSAL_ENVELOPE_INVALID` — `MAX_PROPOSAL_PAYLOAD_BYTES = 1024*1024` checked at `validation.rs:565` (pinned by `envelope_refuses_oversized_payload` ~1169–1177); never reaches the feasibility gate | WRITTEN-UNRUN | (empty) |
| O-2 | Malformed output (unknown key smuggled into the result payload) | Closed-schema refusal: `enforce_gnn_result_schema` / `enforce_marl_result_schema` / `research.enforce_result_schema` refuse keys outside the closed set; forbidden keys at ANY depth refuse emission entirely (§1 scans) | WRITTEN-UNRUN | (empty) |
| O-3 | Out-of-range research output (setpoints outside declared capability) | The MARL filter clamps or refuses within declared limits (`safety_filter.py` closed outcomes `passed_within_limits` / `filtered_clamped_to_limits` / `refused_permission_ceiling` / `refused_nonfinite_action` / `infeasible_no_safe_projection`, ~126–146); the harness's Stage-6 physical re-validation refuses violations `PHYSICAL_REVALIDATION_LIMIT_VIOLATED` with the constraint named (`validation.rs` ~440–443) | WRITTEN-UNRUN | (empty) |
| O-4 | A permission outside the closed ceiling | `refused_permission_ceiling` — no clamp turns a forbidden permission into an allowed one (`safety_filter.py` ~409–437; the ceiling mirrors `plugins/profiles/sim-lab.profile.json`: `telemetry.read`, `asset.read`, `schedule.propose`, and the `research-marl` profile withholds even `schedule.propose`) | WRITTEN-UNRUN | (empty) |
| O-5 | A forged `safety_filter_outcome` (result without a named filter outcome) | Refused — `enforce_marl_result_schema` requires the outcome inside the closed `FILTER_OUTCOMES` vocabulary (marl :517–524, "a result without a named filter outcome is unrepresentable") | WRITTEN-UNRUN | (empty) |
| O-6 | A research output arriving "over an internal channel" (the ledger's named bypass risk) | No such channel exists in the reviewed code: the only worker output is one JSON document on stdout, scanned by `assert_proposal_only`; the only route into the feasibility gate is `ValidationHarness::validate` stages 1–6 (§1) — verified by reading the module import sets (the workers import json/math/os/sys and their sibling modules only; the MARL suite pins the import set by AST per `marl_worker.py` header item 2) | WRITTEN-UNRUN (static verification done by reading; dynamic re-run = human gate) | (empty) |
| O-7 | A wrong-protocol request (`protocol` ≠ the worker's tag) | Refused before any computation (`WorkerRequestError`, exit 2: gnn :574–577, marl :371–374, andes :257–260) | WRITTEN-UNRUN | (empty) |
| O-8 | A caller-injected action on the MARL policy path (bypass-the-policy attempt) | Refused by construction: a `shadow_inference` request carrying `action` is a `WorkerRequestError` ("a caller-injected action would bypass it", marl :401–408) | WRITTEN-UNRUN | (empty) |

**The AI-generated-code posture (recorded, per the review obligation):**
every reviewed worker was written by coding agents in this delivery
loop and NEVER EXECUTED; every framework-facing call is labelled
UNVERIFIED-API in the worker's honesty header and carries an
`unverified_note` verbatim in every result envelope (gnn :233–240,
marl :266–274, andes :182–188). The standing posture is therefore:
treat every framework call as untrusted until the human build/test
pass executes the worker and its suite; the pytest suites (including
the AST import-set pin and the introspection test asserting no
dispatch/issue/authorize-shaped function exists in `marl_worker.py` —
`14_marl_worker.md` §2) are the reviewable negatives. This review adds
no execution evidence and claims none.

## 5. Findings summary and the honest verdict

| # | Severity | One-line statement | Owner of the fix |
|---|---|---|---|
| F-1 | HIGH (open) | Unguarded `torch.load` in the GNN artifact path, code-as-written; unreachable today, blocks the with-framework activation | P8.10/P8.11 owner; re-tested by the P8.12 human gate |
| F-2 | MEDIUM (gap) | MARL policy load site unwritten — the safe-format precondition must precede the with-framework implementation | P8.13/P8.14 owner; human gate |
| F-3 | MEDIUM (gap) | ANDES case-format list unnamed; hostile-case behaviour UNVERIFIED | P8.16 owner; human gate |
| F-4 | INFO (seam) | Artifact signing is P8.32's (not implemented); manifests are in-request stopgaps | P8.31/P8.32 wave |
| G-1 | Gap | No artifact-size cap on model files (request/output caps exist; artifact-side does not) | P8.20/P8.10/P8.13 at artifact-store activation |
| G-2 | Gap | Record-level training-data validation/quarantine is P8.35's, not built | P8.35 owner |
| D-2 (from the P8.24 record) | Gap | No telemetry quality-state field in the worker request contracts | P8.04/Phase 7 follow-up owner |

**Verdict (honest):** the proposal-only boundary, the mode locks, the
fail-closed transport discipline, the closed result vocabularies, and
the harness's refuse-anything-the-gate-does-not-pass posture are
VERIFIED BY READING with file+line evidence above. The untrusted-model
surface is NOT safe as written for the first with-framework build (F-1)
— one HIGH finding is OPEN, so the ledger's acceptance item ("zero open
high/critical findings") is NOT met by this record and the exit gate
cannot take it as met. Dynamic verification (running the §4 cases and
the ledger's independent re-run) is the human gate; no clearance is
claimable from a read-only review.

## 6. Contract gaps → Phase 7 follow-ups (recorded, never invented fixes)

| # | Gap | Where recorded |
|---|---|---|
| S-1 | The Phase 7 worker template's gRPC binding is not wired into the research workers (JSON-document transport discipline instead; buf generation remains the activation step) — the trust-relevant consequence: the P7.34 transport-identity wave (mTLS, untrusted-host posture) does not cover the research workers until they bind the servicer | carried from `11_gnn_inference_worker.md` §8.1 / `14_marl_worker.md` §8.1; not fixed here |
| S-2 | The closed P8.04 result-key sets are too small for the role payloads (the values travel in `outputs.*` beside the checked `result_payload`) — the additive extension is the P8.04 owner's follow-up; until then the proposal-only scan (which covers the WHOLE envelope including `outputs`) is the enforcement, verified in §1 | carried from the P8.11/P8.14 records |
| S-3 | Proposal dedup/idempotency does not exist at the harness (the P8.24 record's O-3/D-1) — a security-adjacent availability/integrity question for the P8.04/Phase 7 owner | `24_failure_drill.md` §5.3 |

## 7. Human gate inputs (all with EMPTY results until executed)

| # | Gate input | How | Result |
|---|---|---|---|
| 1 | Re-run the hostile-input cases of §4 independently (the ledger verification method) | run the worker suites + the §4 cases against the actual workers | (EMPTY — UNRUN) |
| 2 | Fix F-1 at the DECISION-POINT resolution: name the allowed GNN model format in writing with its safe loading call verified from the pinned framework's primary source; re-test with corrupt/oversized/unexpected-ops artifacts | P8.10/P8.11 owner + P8.12 gate | (EMPTY — OPEN finding) |
| 3 | Record the ANDES allowed case-format list and run the hostile-case cases | P8.16 gate | (EMPTY) |
| 4 | The P8.32 signing seam and the P8.20 artifact store (F-4/G-1 enforcement points) | P8.31/P8.32 + artifact-store waves | (EMPTY — seams referenced, not implemented) |
| 5 | Re-assess "zero open high/critical findings" only after 1–4 | ledger acceptance check | (NOT ASSESSABLE — no run) |

## 8. Verification status (honest)

| Item | Status |
|---|---|
| Code review (by reading) | DONE — files and line ranges cited in §1–§4 (read 2026-11-14) |
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (must still report OK; no dependency edge added) |
| Hostile-input execution (any §4 case) | **NOT RUN — banned in this loop; human-gate input 1** |
| Findings | **F-1 HIGH OPEN (blocks exit check); F-2/F-3 gaps; F-4 seam; G-1/G-2 gaps — no "zero findings" claim** |
| P8.36 advisory checks | **NOT EXECUTED here — that is P8.36's recorded procedure (`36_cve_pin_check.md`), run at pin time** |
