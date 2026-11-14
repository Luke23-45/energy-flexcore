# P7.29 — Security-test report and decision record: supply chain, escalation, and host compromise at the SDK boundary

Date: **2026-11-13** (the dated security-test report required by ledger step 4
is §3 of this record; the report and the decision record live in this one
file, per the recorded file choice). Subtask: P7.29, Phase 7 ledger
(`ledger_phase_7.md` §P7.29). Owner role: security engineer. Report trace:
§19.2(7); §17.4; §9.3; §5.6; §5.7; §5.8; §21.4; ADR-010. Inputs: P7.13,
P7.14, P7.23 (the ledger's named prerequisites) plus the P7.12, P7.15,
P7.07, P7.36 seams the attack chains compose.

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-07-plugin-sdk/ENTRY_NOTE.md`):** every test in this wave is
WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was executed.
No attack was executed against a running host, no finding was produced, and
no clearance exists. **"Zero open high/critical findings" is NOT claimable
today and is NOT claimed** — the acceptance item stays open until the human
build/test pass executes §2's suites and the ledger's independent re-run by a
second engineer (P7.29 verification method) fills §3 from actual output.
Every findings cell below is deliberately EMPTY.

## 0. Files (complete list; nothing outside it was touched)

| File | Status |
|---|---|
| `apps/plugin-host/tests/security_p7_29.rs` | NEW (P7.29 host-side attack chains: escalation, containment, substitution traceability, audit trail, substituted-artifact last line) |
| `crates/flexcore-kernel/tests/security_p7_29_kernel.rs` | NEW (P7.29 kernel-side attack chains: supply chain, downgrade, unprovenanced, lifecycle abuse, forged profile) |
| `ledger/phase-07-plugin-sdk/29_security_test_report.md` | NEW (this record: dated report + reuse-by-citation map + trust-boundary table) |
| `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | EDITED (P7.29 tick ONLY) |

**Split discipline (recorded, not hidden):** the task named one test file
under `apps/plugin-host/tests/`; the mandated supply-chain cases must drive
the KERNEL signing/profile/lifecycle types read-only, and
`flexcore-plugin-host` depends on no `flexcore-*` crate by design (§8.2
dependency rule, enforced by `tools/check_dependency_rule.py`) — adding a
dev-dependency would require a `Cargo.toml` edit, which the loop bans. The
kernel-reachable attack chains therefore live in the companion file under
`crates/flexcore-kernel/tests/` (a permitted new-test location), each case
cross-referencing the host file. No crate `src/**`, no `Cargo.toml`, and no
existing test file was touched.

## 1. Reuse-by-citation map: every §19.2(7) category → exactly one owning suite

No suite re-runs another's cases (no drift); P7.29 composes owned unit cases
into attack CHAINS and adds the cases no existing suite had.

| §19.2(7) category | Owning suite (citation — NOT re-run) | What P7.29 adds (the chain) |
|---|---|---|
| Tampered package / artifact substitution | `crates/flexcore-kernel/tests/signing_p7_14.rs` (unit: digest mismatch, manifest binding, verifier port) | kernel file case 1: tamper AFTER a successful verification, re-presented at the next boundary → `DIGEST_MISMATCH` → `GATE_REFUSED` → never advances (§5.6 default) |
| Manifest/version substitution (downgrade) | `crates/flexcore-kernel/tests/profile_p7_15.rs` (unit: unsatisfied constraint) + `signing_p7_14.rs` (unit: manifest mismatch) | kernel file case 2: the two-layer downgrade chain — profile constraint refusal AND provenance-binding refusal on the same substituted pair |
| Unprovenanced plugin (no SBOM) | `crates/flexcore-kernel/tests/conformance_p7_21.rs` (unit: SBOM reference required) | kernel file case 3: the three unprovenanced shapes (missing field / empty reference / no record) through the gate to the never-advances containment |
| Privilege escalation (capability expansion) | `apps/plugin-host/tests/enforcement_p7_12.rs` (unit: frozen verdict) | host file case 1: the expansion-attempt chain — new verdict vs untouched issued verdict, both decisions audited, every freeze record naming drain-before-replace |
| Escalation containment / removal | `enforcement_p7_12.rs` (unit: single-grant revocation) + `lifecycle_p7_07.rs` (`Failed → Draining` cleanup edge) | host file case 2: whole-frozen-set revocation — every capability denied in every scope, three audited denials, frozen evidence retained |
| Lifecycle-state abuse (restart, drain-deadline) | `crates/flexcore-kernel/tests/lifecycle_p7_07.rs` (unit: explicit-restart-only; deadline restores Ready) | kernel file cases 4–5: the abuse chains — restart to shed the drain obligation (refused), clock-advance never restarts, drain-deadline expiry cannot force a replacement (refused + audited + still blocked) |
| Profile-ceiling escalation (forged profile) | `crates/flexcore-kernel/tests/profile_p7_15.rs` (unit: ceiling never-granted, mode lock, unknown fields) | kernel file case 6: the four forgery shapes as one escalation attempt set (never-granted entry, mode raise, smuggled knob, out-of-vocabulary id) with the honest-profile positive control |
| Ambient access / exhaustion / API escapes | `apps/plugin-host/tests/sandbox_p7_13.rs` + `enforcement_p7_12.rs` (units) + `untrusted_plugin_p7_23.rs` (layered chain) | none — fully owned by P7.13/P7.23; P7.29 re-refuses nothing they already prove (no drift) |
| Tenant leakage / cross-tenant reads | `untrusted_plugin_p7_23.rs` case 3 (three attempts, each audited); data-level filtering = P7.39 (later wave) | none here — cross-tenant refusal is P7.23's; the data-level tenant-filtering layer is P7.39's deliverable and is NOT claimed |
| Secrets exposure | `crates/flexcore-kernel/tests/plugin_config_p7_19.rs` (unit: value/collision refusals, handle-only resolution) + P7.19 §3 seam | none here — cited |
| Dispatch bypass (ADR-010) | `untrusted_plugin_p7_23.rs` case 4 + `sandbox_p7_13.rs` §8 (unbuildable proof) | none here — cited |
| Wasmtime-internal sandbox escape (plugin-escape surface) | NOT testable in-repo — the R7-C2 CVE floor (`12_13_enforcement_negatives.md` §9; `40_toolchain_pinning.md` §3) + human gate | none — see §3 and §5 |
| Host compromise (compromised HOST vs compromised COMPONENT) | RECORDED, not tested (§4) — no in-repo seam can show it; the mTLS/service-identity enforcement is P7.34's wave | the trust-boundary table §4 |

## 2. The attack tables — every case, status WRITTEN-UNRUN, findings EMPTY

### 2.1 Kernel-side chains (`crates/flexcore-kernel/tests/security_p7_29_kernel.rs`, 7 cases)

| # | Case (test name) | Attack attempted | Required outcome | Status | Findings |
|---|---|---|---|---|---|
| K-1 | `a_tampered_artifact_is_refused_at_reverification_and_never_advances` | artifact bytes swapped after a successful verification, re-presented at the next verification boundary | `DIGEST_MISMATCH` at the gate; `GATE_REFUSED` at the lifecycle; state stays `Discovered` (never advances, §5.6 default); both refusals audited with the attempted edge | WRITTEN-UNRUN | (empty) |
| K-2 | `a_version_downgrade_substitution_is_refused_by_the_profile_constraint_and_the_provenance_binding` | same plugin id substituted at 1.9.0 where 2.0.0 was pinned and provenanced | `PROFILE_MANIFEST_VERSION_UNSATISFIED` at plan validation AND `MANIFEST_MISMATCH` at the provenance binding (positive control proves the honest pair composes) | WRITTEN-UNRUN | (empty) |
| K-3 | `an_unprovenanced_plugin_is_refused_before_any_compatibility_check` | record without `sbom-reference`; record with empty `sbom-reference`; no record at all | `PROVENANCE_MISSING_FIELD` / `PROVENANCE_SBOM_REFERENCE_INVALID` at parse; `UNSIGNED_ARTIFACT` at the gate before any compatibility check; `GATE_REFUSED`; never advances | WRITTEN-UNRUN | (empty) |
| K-4 | `a_restart_without_explicit_policy_is_refused_and_only_failed_plugins_restart` | restart an Active (Ready) plugin to shed the drain obligation; walk forward out of `Failed`; let time pass | `ILLEGAL_TRANSITION` from Ready; gated forward out of `Failed` refused; state stays `Failed` across a clock day (no auto-restart); only the audited `EXPLICIT_RESTART` from `Failed` yields a fresh lifecycle | WRITTEN-UNRUN | (empty) |
| K-5 | `a_drain_deadline_abuse_cannot_force_a_replacement` | force the drain deadline to expire with work outstanding so the incumbent looks removable | `DRAIN_DEADLINE_EXCEEDED`; stop refused; incumbent RESTORED to `Ready` (audited); `REPLACEMENT_BLOCKED_UNDRAINED` on the replacement attempt (audited); only a genuinely quiesced drain opens the gate | WRITTEN-UNRUN | (empty) |
| K-6 | `a_forged_profile_cannot_escalate_the_ceiling_or_the_control_authority_mode` | forged profiles: ceiling with `dispatch.authorize`; `control-authority: operator-approved`; smuggled `enable-dispatch` field; out-of-vocabulary ceiling entry | `PROFILE_PERMISSION_CEILING_NEVER_GRANTED` / `CONTROL_AUTHORITY_MODE_FORBIDDEN` / `PROFILE_UNKNOWN_FIELD` / `PROFILE_PERMISSION_CEILING_UNKNOWN` — strict parse refusals; honest-profile positive control composes | WRITTEN-UNRUN | (empty) |
| K-7 | `every_refusal_name_asserted_by_this_suite_is_distinct` | (meta) the refusal-name table is pairwise distinct; names read back from the code | distinctness holds | WRITTEN-UNRUN | (empty) |

### 2.2 Host-side chains (`apps/plugin-host/tests/security_p7_29.rs`, 5 cases)

| # | Case (test name) | Attack attempted | Required outcome | Status | Findings |
|---|---|---|---|---|---|
| H-1 | `an_expanded_capability_set_is_a_new_verdict_and_never_a_mutation_of_the_issued_one` | mutated manifest + widened ceiling re-evaluation against a running plugin's issued verdict | a NEW verdict; the issued verdict byte-identical to its issue-time clone (frozen); both allows and both freezes audited; every freeze record names drain-before-replace | WRITTEN-UNRUN | (empty) |
| H-2 | `detected_escalation_is_contained_by_revoking_the_whole_frozen_set` | (containment drill) revoke every grant of the frozen verdict after a detected expansion | every formerly granted capability denies in every scope (`CAPABILITY_NOT_GRANTED` ×3); three individual broker audit denials; frozen evidence retained | WRITTEN-UNRUN | (empty) |
| H-3 | `a_downgraded_manifest_identity_is_recorded_verbatim_and_an_unnamed_one_is_refused` | manifest-version downgrade presented at the door; empty manifest id/version | the verdict and every derived grant record the identity VERBATIM (kernel refuses the downgrade — cited); unnamed identities refused `INSTANTIATION_REQUEST_INVALID` | WRITTEN-UNRUN | (empty) |
| H-4 | `the_escalation_attempt_leaves_a_complete_ordered_audited_trail` | the full chain: forged ceiling → honest instantiation → revocation → use attempt | door-audit order refused < allowed ≤ frozen with injected timestamps; post-revocation denial audited broker-side with the reason class | WRITTEN-UNRUN | (empty) |
| H-5 | `a_substituted_artifact_reaches_no_execution_even_under_a_fully_allowed_verdict` | substituted artifact bytes under a fully allowed verdict | the fail-closed `NoBindings` seam refuses every use (`BINDINGS_NOT_GENERATED`); nothing parses, nothing executes in this build | WRITTEN-UNRUN | (empty) |

## 3. THE DATED SECURITY-TEST REPORT (2026-11-13) — summary (results UNRUN)

| §19.2(7) category | Cases driven by | Executed? | Findings (severities) | Open high/critical |
|---|---|---|---|---|
| Supply-chain compromise (tamper / substitution / unprovenanced) | K-1, K-2, K-3 (+P7.14/P7.21 units cited) | **NO — WRITTEN-UNRUN** | (EMPTY — no run, no findings) | NOT ASSESSABLE — no run |
| Privilege escalation (capability / lifecycle / profile) | H-1, H-3, H-4, K-4, K-5, K-6 (+P7.12/P7.23 cited) | **NO — WRITTEN-UNRUN** | (EMPTY) | NOT ASSESSABLE — no run |
| Escalation containment & removal | H-2 (+P7.07 cleanup edge cited) | **NO — WRITTEN-UNRUN** | (EMPTY) | NOT ASSESSABLE — no run |
| Tenant leakage | P7.23 case 3 (cited); P7.39 later wave | NO — cited, not re-run | (EMPTY — owner: P7.39) | NOT ASSESSABLE |
| Secrets exposure | P7.19 suite (cited) | NO — cited, not re-run | (EMPTY — owner: P7.19 verifier) | NOT ASSESSABLE |
| Exhaustion / ambient access / API escapes | P7.13 suite (cited) | NO — cited, not re-run | (EMPTY — owner: P7.13 verifier) | NOT ASSESSABLE |
| Plugin-escape-by-wasmtime surface (CVE class) | NOT RUNNABLE in-repo — see below | **NO — human gate** | (EMPTY) | **OPEN by record**: the R7-C2 floor is a PIN, not a test result |
| Host compromise (trust boundary) | RECORDED (§4) — P7.34's wave | NO — seam not built | (EMPTY — owner: P7.34) | NOT ASSESSABLE |

**Statements that may NOT be made from this report (and are not made):**

1. **"Zero open high/critical findings" is NOT claimable.** The ledger's
   acceptance item ("zero open high/critical findings") requires the human
   build/test pass to execute every case above, the independent re-run by a
   second engineer (the ledger's verification method), and findings tracked
   to closure. No run exists; no clearance exists; the CHECKLIST.md item
   "Security tests pass with zero open high/critical findings" remains
   UNTICKED by this wave.
2. **No hostile-component execution exists.** Every case above drives the
   kernel/host API — refusal happens BEFORE any component code runs. A live
   WASM component attempting these attacks from inside an instance is the
   P7.13 §7 human-gate list (unchanged, still open).
3. **The plugin-escape-by-wasmtime surface is bounded by the R7-C2 CVE floor
   and a HUMAN GATE, not by this report.** The pin `wasmtime = "=50.0.1"`
   carries the three October 2026 fixes (CVE-2026-115889 /
   GHSA-897f-g5gm-fmxm; CVE-2026-115893 / GHSA-mghf-wjfp-p5xm;
   CVE-2026-115384) per `40_toolchain_pinning.md` §3 — but the advisory-page
   re-check, the check date, and a real hostile-component run remain the
   P7.13 §9 human gate (`03_sdk_license_audit.md` §4 flags wasmtime
   RE-VERIFY). This report adds no sandbox-escape coverage and claims none.

## 4. Host-compromise posture — the trust-boundary table (RECORDED, not tested)

The ledger names "host compromise via a malicious plugin". The honest
position: **no in-repo test can demonstrate what a compromised HOST could or
could not do** — the host IS the enforcement point in v0.5.0, and the
host↔control-plane channel carries no service identity yet. That enforcement
is P7.34's wave (mTLS, service identity, untrusted-host posture). This
section records the boundary so P7.34 starts from it; nothing here is tested
and no claim is made.

| Compromised element | What it could plausibly do in the v0.5.0 build | What still bounds it | Wave that enforces more |
|---|---|---|---|
| A malicious PLUGIN (component) | attempt every refusal path of P7.13/P7.23/P7.29; exhaust resources; produce oversized/wrong outputs | the door (P7.12), the broker per-call checks (P7.11), the limits + teardown contract (P7.10), the lifecycle (no auto-restart; drain-before-replace), the profile ceiling (P7.15); no never-granted id is grantable | P7.33 (audit tamper-evidence), P7.38 (revocation/removal), P7.39 (isolation) |
| A malicious WORKER (Class C, out-of-process) | return malformed/oversized results; hang; crash | process isolation (ADR-004: no import path into the control plane), supervision (P7.18: timeouts, bounded retries, backpressure, crash → FAILED), proposal-only contract (ADR-010) | P7.34 (worker transport identity), P7.42 (determinism) |
| A malicious HOST process (the trust boundary above the SDK) | read everything the host process can read (plugin artifacts, config references, in-process state); emit forged audit records into its own sink; refuse to enforce (it IS the enforcement code); impersonate a plugin toward the control plane over the (currently unauthenticated) transport seam | ONLY out-of-band facts: the protected kernel is a DIFFERENT process (the command path never runs inside the host), the health/telemetry snapshot is display-only (P7.25), and the operator sees plugin identity/versions in the console. In-process, a compromised host is NOT containable by the SDK itself | **P7.34** — mTLS with service identity, short-lived certificates, revocation, and the explicit "the host is an untrusted process" posture; P7.33 (tamper-evident audit records that would expose a forging host) |
| A malicious CONTROL-PLANE client | attempt API-level attacks | the control-plane authn/authz layers (P2/P6 suites) | out of P7.29's scope (existing platform suites) |

**The recorded seam:** the health/telemetry transport (`HealthSnapshot`
carrier) and the host↔control-plane gRPC channel are UNAUTHENTICATED seams
today (`19_20_36_config_health_path.md` §5; `10_11_plugin_host_broker.md` §1)
— exactly the surface a compromised host could impersonate. P7.34 owns
closing it; until then the §21.4 posture stands: the entire 0.x line is
simulation/shadow and the host never touches field control.

## 5. Honest observations recorded while writing (not silently patched)

1. **`ILLEGAL_TRANSITION` refusals carry no audit record.** The P7.07
   decision record states "every transition — committed OR refused — emits a
   `LifecycleAuditRecord`"; in `crates/flexcore-kernel/src/lifecycle.rs` the
   gate refusals, drain-deadline refusals, and `require_fully_drained`
   refusals ARE audited, but the `illegal()` paths (including a refused
   `restart_explicitly` from a non-`Failed` state) return the named error
   WITHOUT emitting a record. This suite asserts audit records ONLY on the
   paths that emit them (verified against the source read 2026-11-13) — the
   discrepancy is flagged for the human build pass and the P7.07 record's
   owner to reconcile (extend the audit or correct the record's wording).
2. **Broker grants are keyed by plugin id, not by instance.** A
   re-evaluation (H-1) derives NEW grants for the SAME plugin id; the broker
   cannot distinguish "the drained instance" from "the replacement" within
   one plugin id. The recorded containment model is therefore: re-evaluation
   is a composition-level act (the door), and the running instance is
   replaced only through drain-before-replace — the per-call re-checks bound
   what the id can use RIGHT NOW. A per-instance grant binding (verdict-
   scoped checks) is a possible P7.34/P7.38 hardening item, recorded as a
   recommendation, not implemented here.
3. **The H-3 traceability case is deliberately NOT a downgrade refusal.**
   The host door accepts any non-empty identity and records it verbatim;
   version-order enforcement lives kernel-side (profile constraint +
   provenance binding, K-2). Testing a host-side version refusal would test
   an enforcement that does not exist at that layer.

## 6. Decisions (DEFAULT-labelled where recommended, not decided)

| # | Decision | Label |
|---|---|---|
| D-1 | The P7.29 attack layer is split across two new test files (host-side + kernel-side) because `flexcore-plugin-host` depends on no flexcore crate (§8.2) and no `Cargo.toml` edit is permitted; the dated report's §1 map allocates every category to exactly one owning suite | RECORDED constraint (followed from the dependency rule) |
| D-2 | The dated report and the decision record live in this single file (`29_security_test_report.md`); the ledger tick cites this file | DEFAULT |
| D-3 | The forged-profile forgeries are built by string-editing an in-file honest fixture profile (the P7.15 `.replace()` pattern) rather than shipping new profile JSON files — no new files under `plugins/profiles/`, which belong to the P7.15 owner | DEFAULT |
| D-4 | H-5 (substituted artifact → `NoBindings`) is kept despite P7.23's adjacent case, because the P7.29 chain must END at the execution seam for the supply-chain scenario; the assertion set is narrower (no world/entry assertions beyond the seam refusal) and cross-referenced, not duplicated | DEFAULT |
| D-5 | Tenant-leakage and secrets-exposure categories are CITED (P7.23/P7.19), not re-tested — no re-runs, no drift (the ledger's own step-1/step-3 wording is satisfied by the citation map in §1) | DEFAULT (the P6.28 D-6 reuse-by-citation precedent) |

## 7. Human gate inputs (all with EMPTY results until executed)

| # | Gate input | How | Result |
|---|---|---|---|
| 1 | Build + run both new suites | `cargo test -p flexcore-kernel --test security_p7_29_kernel`; `cargo test -p flexcore-plugin-host --test security_p7_29` | (EMPTY — UNRUN) |
| 2 | Fill §2's findings columns and §3's summary from actual output | run 1's output | (EMPTY — no findings claimed) |
| 3 | Independent re-run of the attack cases by a second engineer; then, and only then, assess "zero open high/critical findings" | the ledger P7.29 verification method | (EMPTY — no clearance claimable) |
| 4 | The R7-C2 advisory re-check (three GHSA pages, IDs + date recorded) and, once a signed component exists, the hostile-component run list of P7.13 §7 | P7.13 §9 human gate | (EMPTY — still open at its owner) |
| 5 | Reconcile observation §5.1 (the `ILLEGAL_TRANSITION` audit gap) with the P7.07 record's owner | human build pass | (EMPTY — flagged) |

## 8. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (output quoted in the completion report — it must still report OK for all members; no dependency edge was added) |
| `cargo build` / `cargo test` / `cargo clippy` (any target) | **NOT RUN — banned in this loop; human gate input 1** |
| Security findings | **NONE RECORDED — §2/§3 tables deliberately EMPTY; no "zero findings" claim is made** |
| Host-compromise testing | **RECORDED ONLY (§4) — no in-repo seam can test it; P7.34's wave** |

## 9. Related records

- `12_13_enforcement_negatives.md` — the P7.12/P7.13 unit negatives and the
  R7-C2 human gate this wave composes and cites.
- `21_22_23_conformance_swap_untrusted.md` — the layered untrusted-plugin
  proof (P7.23) whose categories P7.29 extends at chain level.
- `14_15_16_signing_profiles_recomposition.md` — the P7.14 provenance gate
  and P7.15 profile ceiling/mode lock the supply-chain and forgery chains
  drive.
- `07_08_lifecycle_registry.md` — the P7.07 lifecycle abuse surface
  (restart policy, drain-before-replace) and the recorded audit-wording
  nuance (§5.1).
- `19_20_36_config_health_path.md` §5 — the unauthenticated transport seams
  the §4 trust-boundary table names as P7.34's surface.
