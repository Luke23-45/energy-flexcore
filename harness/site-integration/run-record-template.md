<!-- P10.22 — Harness RUN RECORD template. EVERY result cell EMPTY (UNRUN). Filled
     ONLY from actual run output, the same day as the run. Filling a cell from
     anything other than actual output is fabrication (AGENTS.md rule 1). -->
# Site Integration Test Harness — Run Record (EMPTY template)

Harness id: `flexcore-site-integration-harness-v1` (`site-harness-v1`)
Release-candidate identifier: `<EMPTY — must equal the P10.18 candidate>`
Manifest: `harness-manifest-v1.json` (version-mismatch rule: README §4)

## 1. Configuration record (fill BEFORE the first end-to-end case) — ALL EMPTY

| Field | Value (EMPTY — fill at execution) |
|---|---|
| Release-candidate identifier (P10.18) | |
| Control-plane components + versions (PostgreSQL, NATS, API, console) | |
| Edge-agent build (git SHA) | |
| Site-EMS instance + INSTALLED version (re-pinned, never assumed) | |
| Mapping profile (file + version) | |
| Grid-model version | |
| mTLS material reference (CA, edge credential issuer) | |
| Staging environment description + its pilot-mirroring statement | |
| Version-mismatch gate outcome (step 0) | |
| Run dates / executor / witness (negative command cases) | **UNSIGNED** |
| Clean-environment statement (what was clean, verified by whom) | |
| Non-author statement (executor is not a harness author — name + role) | |

## 2. Per-leg, per-check result table — ALL EMPTY

One row per check of each leg (leg contents per `harness-manifest-v1.json`
and its cited source records). Result ∈ {PASS, FAIL, NOT-APPLICABLE-with-
reason, BLOCKED-`<reason>`, REFUSED-VERSION-MISMATCH}.

| Leg | Check | Result | Evidence artifact (log/capture/audit reference) | Detail (from the actual output) |
|---|---|---|---|---|
| B | `contract_capability_honesty` | (EMPTY — UNRUN) | | |
| B | `contract_named_error_refusals` | (EMPTY — UNRUN) | | |
| B | `contract_quality_passthrough_no_upgrade` | (EMPTY — UNRUN) | | |
| B | `contract_units_and_acl_conformance` | (EMPTY — UNRUN) | | |
| B | `contract_identity_source_id_discipline` | (EMPTY — UNRUN) | | |
| B | `contract_unmapped_channel_register` | (EMPTY — UNRUN) | | |
| B | `suite_full_p6_13_fault_injection_set` | (EMPTY — UNRUN) | | |
| B | `suite_full_p6_13_fault_injection_set_openems` | (EMPTY — UNRUN) | | |
| B | `flexcore_only_read_only_structural_guarantee` | (EMPTY — UNRUN) | | |
| B | `flexcore_only_registry_allowlist_conformance` | (EMPTY — UNRUN) | | |
| B | `suite_record_renders_suite_and_claim_boundary` | (EMPTY — UNRUN) | | |
| A | L1 … L10 (one row each; `14_lab_integration_test.md` §3) | (EMPTY — UNRUN) | | |
| A | F11 … F18 (one row each; §3 part C) | (EMPTY — UNRUN) | | |
| C | L-1 exit-gate scenario + fault/shadow suites (per pre-registered expectation) | (EMPTY — UNRUN) | | |
| C | L-2 scenario-a/b/c (per pre-registered expectation) | (EMPTY — UNRUN) | | |
| C | L-3 lib-01 … lib-08 (per record tolerance; lib-04/05 worker env) | (EMPTY — UNRUN) | | |
| C | L-4 edge register files marked IN for this run (§6.5-style applicability record required) | (EMPTY — UNRUN) | | |
| C | L-5 sdk-01 … sdk-04 (exact verdicts) | (EMPTY — UNRUN) | | |
| D | quality-mapping rows / stale+missing injections / clock hold-on-drift | (EMPTY — UNRUN) | | |
| E | dry-run gate steps (propose/validate/feasibility/approve/hold) + refusal paths | (EMPTY — UNRUN) | | |
| F | drills per `20_fallback_drills.md` §5/§6 (register precondition) | (EMPTY — UNRUN) | | |
| G | BLOCKED-NOT-A-LAB-LEG (staging run) — or the site FAT record reference | (EMPTY — UNRUN) | | |

Applicability record (a check omitted from the run without a row above is a
run FAIL on completeness):

| Check | IN / EXCLUDED | Reason (written, environment-tied) |
|---|---|---|
| (one row per excluded check — ALL EMPTY) | | |

## 3. Failures-and-dispositions table

| Failed check | Disposition (fixed-and-re-run / accepted limitation filed to the limitations register / blocks the release via P10.27) | Evidence | Status |
|---|---|---|---|
| (none — exists only when a failure is recorded from actual output) | | | (EMPTY — UNRUN) |

## 4. Suite ids printed by the run (quote verbatim — the claim wording anchor)

| Suite id AS PRINTED | Leg |
|---|---|
| (EMPTY — fill from run output only) | |

## 5. Automatic-FAIL audit (verify each, explicitly)

| Rule | Held? | Evidence |
|---|---|---|
| No command reached an adapter or device in any leg | (EMPTY) | |
| No fault produced a silent wrong value | (EMPTY) | |
| Every check has a result or a reasoned not-applicable | (EMPTY) | |
| Version-mismatch gate executed before every leg | (EMPTY) | |
