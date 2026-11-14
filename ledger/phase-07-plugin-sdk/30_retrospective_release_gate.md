# Phase 7 retrospective, open-items register, and v0.5.0 release-gate record

Date: 2026-11-13. Ledger subtask: **P7.32** ("Phase retrospective, open
items, and v0.5.0 release-gate record"); this record's FILE follows the
Phase 6 naming pattern (`30_retrospective_release_gate.md`, per the P6.30
template) while carrying P7.32's content — the P7.32 ledger line itself is
NOT ticked here (the coordinator ticks it at phase close, after completing
`CHECKLIST.md` and `HANDOVER.md`).

**Status at the time of writing (2026-11-13, from the ledger as it stands
after the concurrent waves landed mid-write): 35 of 43 subtasks are ticked
(P7.01–P7.34, P7.35, P7.36, P7.40); P7.30, P7.32 (this record), P7.37–P7.39,
and P7.41–P7.43 are unticked.** Every
ticked subtask is "implemented, awaiting human build/test" — nothing
installed, nothing built, no tests run, no CI run — so the §23 Stage 7 exit
gate is **NOT HELD** (prepared, not demonstrated). Every lab/witnessed run,
test execution, CI green run, scan, and sign-off named below is a HUMAN GATE
INPUT recorded as UNRUN / NOT SIGNED / OPEN — never asserted, never
fabricated. Empty measured cells stay empty.

## 1. What the phase delivered (traceable, per ticked subtask)

| Area | Subtasks (all ticked) | Delivered (decision record) |
|---|---|---|
| Decisions & readiness | P7.01–P7.04 | Phase 1–6 readiness check with the recorded Stage 1–6 gate exceptions (G-1 build pass UNRUN); SDK scope freeze — the never-granted permission set, the six protected-kernel contract ids, the deferred/OUT categories bound to the open D-items; static license audit for the SDK stack (wasmtime 50.0.1 UNVERIFIED-until-first-scan; the whole SDK dependency table UNPINNED-REVERIFY/ASSUMPTION where RESEARCH.md verified nothing); the capability-seam inventory (contract → provider → consumer, solver-class declarations, the protected non-seams) (`01`, `02`, `03`, `04`) |
| Manifest, resolution, registry, lifecycle | P7.05–P7.08 | The strict `flexcore-plugin-manifest-v1` validator (closed key sets, named errors, never-granted refusal at the manifest gate); the resolver (platform-API gate, range intersection, protected-kernel guard, deterministic plans); the P7.07 lifecycle state machine (frozen §5.6 table + crash edges, gates handed in, drain-before-replace fail-closed, EXPLICIT-only restart, injected clock, audit on every gate/refusal); the P7.08 Class A provider registry (plan-bound registration, protected-kernel refusal first, lifecycle-aware `swap_provider`, seed ids) (`05_06`, `07_08`) |
| WIT worlds + toolchain pins | P7.09, P7.40 | Three versioned WIT worlds with zero `wasi:` imports (validation PREPARED-UNRUN); the toolchain pin record (Rust 1.99.1, wasmtime `=50.0.1` with the three October 2026 CVE fixes, `wasm-tools` 1.239.0 ASSUMPTION, `wit-bindgen`/`cargo-component`/`wasip2` UNPINNED-REVERIFY) (`09`, `40`) |
| Host, broker, enforcement, sandbox | P7.10–P7.13 | The fail-closed component host (loader caps, compile-gated bindings seam with `NoBindings` default, mandatory fuel, derived limits); the capability broker (per-plugin scoped grants, never-granted re-refusal, per-call checks, revocation/expiry, audit-or-refuse); the enforcement door (8-step fail-closed pipeline, frozen verdicts, vocabulary-drift refusals, no raw instantiation path); the sandbox negative suite (ambient access, exhaustion teardown contracts, grant forgery, ADR-010 unbuildable proof, the R7-C2 human-gate split) (`10_11`, `12_13`) |
| Signing, profiles, re-composition | P7.14–P7.16 | The digest-and-record supply-chain layer (provenance records with REQUIRED SBOM reference, key-REFERENCE-only structure, `NoVerifier` fail-closed default, `Discovered → Verified` gate with audit); five `flexcore-profile-v1` profiles as reviewable data (mode lock to `simulation-shadow`, permission ceilings incl. the research no-dispatch ceilings, explicit selections, strict parse); the audited profile switch (confirmation gate, mode-change refusal, protected-contract guard, drain-before-replace ordering, restart fallback) (`14_15_16`) |
| Workers + supervision | P7.17, P7.18 | The Python gRPC worker template (ADR-004 isolation rules, fail-closed config, closed health states, correlation echo, the `flexcore.worker.v1` proto with §9.2 provenance + determinism metadata); worker supervision (per-contract timeouts, bounded deterministic-backoff retries, non-idempotent-never-retried, backpressure refusal, crash → FAILED, probe-success recovery gate) — grpcio paths UNVERIFIED, buf registration recorded (`17_18`) |
| Config, health, path protection | P7.19, P7.20, P7.36 | The plugin config resolver (schema validation, layered constraints document, explicit defaults, secret-REFERENCE-only with value refusals echoing nothing, config drift); the host health/readiness monitor (closed 10-name vocabulary, never inferred from silence, exhaustion→degradation mapping, snapshot for the console); the plugin-path protection (envelope → digest → window → sequence, distinct named refusals, restart-surviving file checkpoint, proposal-envelope type-level ADR-010) (`19_20_36`) |
| Conformance, swap, untrusted | P7.21–P7.23 | `flexcore-plugin-sdk-conformance-suite-v1` (44 cases, class-agnostic with per-class records, claim boundary pinned); the P7.22 swap seam proof (doubles) with the recorded seam gaps 1–3; the P7.23 layered untrusted-plugin proof (refusal at the earliest layer, bypass-then-next-layer-refuses, every denial audited) (`21_22_23`) |
| SDK docs + console | P7.24, P7.25 | The `docs/plugin-sdk/` set incl. the deliberately-bad example with recorded named refusals (actual rejection output PREPARED-UNRUN); the Plugins & Integrations console workspace (display-only, no secret values, confirmation on high-impact actions) (`24_25`) |
| Performance, CI, no-dylib | P7.26, P7.27, P7.35 | The std-only perf harness over the four host-side overheads (measured tables EMPTY, no invented numbers); the CI gate matrix extended to the SDK (five additive jobs; Python worker gates deliberately NOT wired — un-installable pins); the ADR-006 no-dylib lexical gate with calibrated tokens and the test twin (`26_27_35`) |
| Exit gate + security testing | P7.28, P7.29 | The witnessed exit-gate run book + EMPTY record (PREPARED/UNRUN; gate NOT HELD; the seam-gap exposure listed for the coordinator/human); the P7.29 security-test wave (two new attack-chain suites, the dated report with the reuse-by-citation map, the trust-boundary table, findings EMPTY and no clearance claimed) (`28`, `29` — this wave) |
| Phase 8 handover | P7.31 | The research handover note: no research provider of any kind exists in v0.5.0 (a generic `Optimizer` world is not a GNN contract); the proposal-only boundary restated; non-deliverables explicit; Phase 8 lead's confirmation is the open verification step (`31_phase8_handover.md`) |
| Audit integrity + trust boundary | P7.33, P7.34 | The per-plugin/per-session audit hash chain (`audit_chain.rs`, `flexcore-audit-chain-v1`, TEST-GRADE FNV-1a digest, external anchors, first-broken-link verifiers, tamper-evidence suite); the transport trust-boundary wave (the parts implementable without a TLS stack/PKI; the compromised-host posture carried from `29` §4) — both WRITTEN-UNRUN (`33_34_audit_integrity_trust_boundary.md`) |

**Later waves of this phase — referenced, NOT claimed delivered** (unticked
at the time of writing; the retrospective covers each by reference to its
record as it lands):

| Subtask | Title | Status at writing |
|---|---|---|
| P7.30 | Failure-mode drill (plugin host, workers, registry) | unticked — the drill's per-fault skeleton is carried EMPTY in `28_exit_gate_run_record.md` §3.5; the drill record owns the results |
| P7.37 | §17.5 safety-invariant test map | unticked — later wave |
| P7.38 | Revocation, staged rollout, failed-plugin removal | unticked — later wave; the P7.14 §2.4 seams are its recorded plug-in points |
| P7.39 | Cross-component isolation + data-level tenant filtering | unticked — later wave; the tenant-leakage category in the P7.29 map points at it |
| P7.41 | Manifest-validator fuzz/property tests | unticked — later wave; the duplicate-key-collapse case is the recorded target |
| P7.42 | Plugin-path determinism + seeded replay | unticked — later wave; the P7.36 identity and P7.18 determinism fields are its inputs |
| P7.43 | Scenario-regression suite for the SDK | unticked — later wave |

## 2. Open-items register (owner + venue; carried from this phase's records)

| # | Item | Status | Owner / venue |
|---|---|---|---|
| O-1 | Workspace build + full test-suite run (P7.01 G-1; every tick reads "awaiting human build/test") | UNRUN — the single verification gate for the whole phase | Human build/test pass; first `cargo test --workspace` |
| O-2 | D1–D6 (`.agent/DECISIONS.md`) still OPEN (D1 jurisdiction/market; D2 asset mix; D3 grid fidelity; D4 licenses; D5 protocol/asset mix; D6 OT boundary) | OPEN — the SDK deferrals in `02_scope_freeze.md` are bounded by them | Product + architecture review; `.agent/DECISIONS.md` |
| O-3 | License verifications: rustls/rustls-pemfile RE-VERIFY; OpenEMS license UNVERIFIED (Apache-2.0 must NOT be assumed); wasmtime + the whole SDK dependency table (wasmtime-wasi, wit-bindgen, wasm-tools, cargo-component, wasip2, tonic, prost, buf, grpcio, protobuf) UNVERIFIED until the first lockfile-backed `cargo deny check` | UNPINNED-REVERIFY / ASSUMPTION rows in `03_sdk_license_audit.md` §2 | Legal + engineering (first human `cargo deny` run; P7.03 §5) |
| O-4 | `wasm-tools` 1.239.0 is an ASSUMPTION (single secondary survey, 26 Aug 2026); `wit-bindgen` / `cargo-component` / `wasip2` versions UNPINNED-REVERIFY | Re-check release pages at implementation (RESEARCH.md §7 tasks 2/4) | P7.27 CI owner / first-bindings human run |
| O-5 | The Rust-pin conflict note: wasmtime 50.0.0 requires Rust ≥ 1.97; the workspace pin is 1.99.1 (satisfied today) — the first-build `cargo check` outcome must be recorded, and a future wasmtime release may raise the minimum above the exact pin (bump policy applies) | RECORDED, unresolved-by-design (`40_toolchain_pinning.md` §4) | P7.10 owner at first build |
| O-6 | Bindings generation: the wit-bindgen modules are not materialised; `generated-bindings` is OFF (fail-closed `NoBindings`); the gated-build human-gate list (real hostile components, teardown behaviour, epoch/fuel/output limits under load) is open | RECORDED seam (`10_11` §4; `12_13` §7) | Bindings-build owner (P7.24 examples) + the P7.13 verifier |
| O-7 | Buf registration of `worker_api.proto` with the `proto/` module + first `buf lint`/`buf breaking` verdicts (the P7.27 job is config-only) | RECORDED seam (`17_18` §9 item 1) | Coordinator / activation wave |
| O-8 | grpcio/protobuf pins deliberately un-installable (`==UNPINNED-REVERIFY`); the Python worker CI gates (Ruff/type/pytest) not wired by decision | OPEN (`17_18` §6; `26_27_35` D-5) | Activation wave after version verification |
| O-9 | The P7.22 seam-gap exposure: two path deps in `apps/sim-runner/Cargo.toml` (`flexcore-kernel`, `flexcore-optimization-api`) + `apps/sim-runner/tests/swap_p7_22_integration.rs` — to be applied at the witnessed run, then `check_dependency_rule.py` re-run | NOT APPLIED (`21_22_23` §3.3 gap 2; `28` P-3) | Coordinator/human at the P7.28 witnessed run |
| O-10 | The P7.28 exit-gate run + witness reproduction (the §23 Stage 7 gate itself) | NOT HELD — run book PREPARED/UNRUN (`28`) | Architect + the named witness |
| O-11 | The P7.29 security clearance: findings tables EMPTY, "zero open high/critical findings" NOT claimable until the human run + the independent second-engineer re-run; the R7-C2 advisory re-check (three GHSAs) open | UNRUN (`29` §3, §7) | Security engineer (P7.29 verification method) |
| O-12 | The `ILLEGAL_TRANSITION` audit nuance: illegal-transition paths return named errors without emitting audit records, while the P7.07 record's wording says every refusal is audited — reconcile (extend the audit or correct the record) | FLAGGED (`29` §5.1) | P7.07 owner at the human build pass |
| O-13 | Per-instance grant binding: broker grants are keyed by plugin id, so a re-evaluation for the same id widens the id's live authority; the recorded model relies on drain-before-replace at the composition level — a verdict-scoped check is a possible hardening | RECORDED recommendation (`29` §5.2) | P7.34/P7.38 owners |
| O-14 | The `builtin-ess-provider` example manifest api-version inconsistency (`>=0.5,<1.0` fails the 3-component grammar) | RECORDED, not patched (`14_15_16` §3.6) | P7.05 owner |
| O-15 | The compromised-HOST posture: no in-repo seam can test it; the host↔control-plane transports are UNAUTHENTICATED seams today | RECORDED (`29` §4); P7.34's record + test wave has since landed WRITTEN-UNRUN (`33_34_audit_integrity_trust_boundary.md`) — the real-TLS/PKI parts and the behavioural proof remain open | P7.34 verifier (real mTLS stack, revoked-identity run) |
| O-16 | Carried from Phase 6: uplink wire format (`NoBatchPayloadEncoder` REFUSES), TLS on the OpenEMS REST hop, OpenEMS A1–A8 assumption register | OPEN (P6.30 O-4/O-5; `01` §4) | Phase 6 owners / wire-format owner |
| O-17 | Perf measurements (P7.26 tables), the pilot/research load profile, target-hardware runs on x86-64 AND ARM64 | EMPTY — PLACEHOLDER pending pilot/research input | P7.26 verifier at the lab run |
| O-18 | WASI 0.3 migration re-evaluation (after Rust 1.100 stable + host compatibility, per RESEARCH.md) | DEFERRED, tracked (`CHECKLIST.md` records it as NOT claimed) | Phase planning |

## 3. §22.3 v0.5.0 release-gate position — NOT MET (prepared)

v0.5.0 = "Plugin SDK, WASM host — simulation / shadow".

**Satisfied (implemented, awaiting human verification — §1):** the plugin
manifest schema and validator; the resolver and protected-kernel guard; the
lifecycle machine with drain-before-replace; the Class A registry with the
lifecycle-aware swap; the WIT worlds and toolchain pins; the fail-closed
WASM host with the capability broker, enforcement door, limits, health, and
sandbox negatives; the supply-chain layer (provenance/SBOM-reference/gate)
and composition profiles with the mode lock; the audited profile switch;
the Python worker template and supervision; plugin config and secret
reference discipline; the conformance suite, swap proof, and untrusted-plugin
proof; the plugin-path protection; the SDK docs and console workspace; the
perf harness, CI gate matrix, and no-dylib gate; the exit-gate run book and
the security-test wave.

**Still open (human gate inputs — all UNRUN; the §22.3 "prepared" half):**

| Gate input | Where |
|---|---|
| The workspace build/test pass (fmt, clippy, tests) | O-1 |
| The conformance suite run in CI (run ID empty) | `28` §3.4; P7.27 jobs UNRUN |
| The witnessed swap run (real providers, real validation gate, empty diff) | `28` §2 steps 1–3; O-9, O-10 |
| Security clearance (findings filled, second-engineer re-run, R7-C2 re-check) | `29` §3; O-11 |
| CI green on the phase branch (all jobs incl. the five SDK jobs) | `26_27_35` §2.2 |
| Buf lint/breaking first verdicts + WIT validation (`wasm-tools component wit wit/`) | O-7; `09`/`40` prepared commands |
| Bindings materialisation + the real-component hostile run list | O-6 |
| The failure-mode drill (P7.30) and the later-wave subtasks (P7.37–P7.39, P7.41–P7.43) | §1 later-waves table |
| D1–D6 dispositions and the license re-verifications | O-2, O-3 |

**Belongs to later phases/releases:** operator-approved control (v1.0.0,
post-Stage 9); research providers and benchmark suites (Phase 8 — nothing
handed over is a GNN/MARL/ADMM provider, per P7.31's non-deliverables);
HIL (Stage 9); any field write authority (the entire 0.x line is
simulation-only).

## 4. What v0.5.0 is NOT, in plain language

v0.5.0 is a **simulation and shadow-mode release only**. No plugin can issue
or authorize a dispatch: the permission vocabulary contains no authorization
id, the never-granted set is refused at the manifest gate, the enforcement
door, and the broker, and a dispatch-bypassing plugin is **unbuildable, not
merely discouraged** (ADR-010). Plugins and workers produce proposals only;
every proposal enters the protected validation → feasibility → authorization
path and gains no authority from having been proposed. v0.5.0 is **not
certified** — no protocol, security, or safety certification of any plugin is
claimed, and the conformance suites entitle exactly their own bounded
statements. It is **not safety-qualified** and **not field-control-approved**;
operator-approved control is the v1.0.0 default after Stage 9 qualification.
No claim is made that the WASM sandbox is escape-proof — the wasmtime CVE
floor is a pin plus a recorded human gate, not a tested guarantee.

## 5. Process retrospective — notes

- **Concurrent agents with disjoint file ownership worked, with one rule
  worth keeping:** each wave recorded "who owns which ledger tick lines"
  explicitly, and concurrent ticks were never touched (P7.12/P7.13 vs
  P7.14–P7.16; P7.19/P7.20/P7.36 vs P7.17/P7.18; P7.26/P7.27/P7.35 vs
  P7.24/P7.25; P7.28/P7.29 vs P7.31/P7.33/P7.34 — the latter landed their
  ticks while this retrospective was being written, and the write-time
  state in the header reflects them honestly rather than a frozen snapshot).
- **The dependency rule caught a design error before the compiler could:**
  P7.29's mandated supply-chain cases need kernel types, but
  `flexcore-plugin-host` depends on no flexcore crate by design — reading
  the rule (and `check_dependency_rule.py`) before writing forced the honest
  two-file split instead of a forbidden `Cargo.toml` edit (`29` §0).
- **Compile-catch by reading types remains load-bearing:** every type,
  signature, named error, and JSON key used in the new test files was read
  from source (e.g. the provenance-record field order, the profile-id
  grammar, the `restart_explicitly(self)` consumption, the lifecycle audit
  emissions) — including two real defects caught and fixed before any human
  build (a trailing-comma JSON fixture; a non-emitting audit path that must
  not be asserted). It is still NOT a compiler: O-1 stands.
- **Honesty discipline held:** 21 decision records (including this one;
  the run book `28` and the security report `29` carry the gate/security
  records among them); every run record prepared-not-executed; no fabricated
  run ID, figure, finding, or clearance anywhere.
- **Record-vs-code discrepancies were recorded, not silently patched**
  (O-12, O-14) — the minimal-edit rule over cosmetic consistency.

## 6. Verification tools (permitted only; recorded output, 2026-11-13)

The three permitted repo tools were run after all files in this wave were
written; their ACTUAL output is quoted in the agent completion report and
summarized below. These check documentation and structure only — they do NOT
compile or test Rust; O-1 (the human build/test pass) is unaffected.

| Tool | Result |
|---|---|
| `python tools/verify_docs.py` | OK (output below, exit 0) |
| `python tools/validate_agent.py` | OK (output below, exit 0) |
| `python -I tools/check_dependency_rule.py` | OK — dependency rule holds for 19 workspace members (exit 0; no dependency edge was added by this wave, so the count is unchanged) |
| Any cargo command | **NOT RUN — banned in this loop** |

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
[exit 0]

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
[exit 0]

$ python -I tools/check_dependency_rule.py
OK — dependency rule holds for 19 workspace members.
[exit 0]
```

(The dependency tool's em-dash renders as a mojibake byte on this terminal;
the file and the tool output are UTF-8 — the rendering note recorded in
`19_20_36_config_health_path.md` §11.)

## 7. Handover to the coordinator (phase close)

1. Complete `CHECKLIST.md`: tick each exit-gate item ONLY when the named
   evidence exists (the mapping in `28_exit_gate_run_record.md` §4); items
   whose evidence is the later waves stay unticked with the reason.
2. Complete `HANDOVER.md`; tick the P7.32 ledger line after this record and
   the CHECKLIST are in place (this record deliberately does not tick it).
3. The exit gate itself remains NOT HELD until the P7.28 witnessed run and
   its witness reproduction are recorded — a completed CHECKLIST with
   "prepared" evidence is the honest v0.5.0 position, not a passed gate.
