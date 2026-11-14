# P8.32 — Model-artifact signing, provenance, and verification at load

Date: 2026-11-14. Subtask: P8.32 (`ledger_phase_8.md` §P8.32). Owner role:
security engineer + research engineer (ML). Report trace: §9.3 (supply
chain, by analogy); §17.4 (supply-chain compromise); §6.5 (artifact
storage). Inputs: P8.11 GNN worker (`11_gnn_inference_worker.md`); P8.14
MARL worker (`14_marl_worker.md`); P8.20 artifact store
(`20_reproducibility.md`); P7.14 signing scheme AS REFERENCE
(`crates/flexcore-kernel/src/signing.rs` — read in full, mirrored by
citation); the P8.31 integration surface
(`crates/flexcore-research/src/tenant_isolation.rs`); the security-review
findings this subtask addresses at the RECORD level: F-4 (signing seam
referenced-not-implemented), with F-1/F-2/F-3 as the worker load sites the
verification gets wired into at their activation wave
(`25_security_review.md`).

## 1. Status of this record — WRITTEN-UNCOMPILED, nothing run

The implementation and its test suites exist and were written under the
loop bans: nothing installed, nothing built, no cargo command executed, no
test run (ENTRY_NOTE.md). Every expected value in the tests is
hand-computed from the recorded rules (closed key sets, scheme rules,
refusal tags, the DEFINITIONAL FNV offset-basis vector) or is a literal
copied from the module's own constants. NOTHING here is evidence that the
crate compiles or that any test passes; the human build/test gate
(AGENTS.md rule 7) owns that evidence. The ledger tick means
*implemented, awaiting human build/test*.

## 2. The signing scheme (ledger step 1) — the digest-and-record structure

The scheme MIRRORS the kernel's P7.14 provenance-v1 pattern by citation
(`KERNEL_SIGNING_ALIGNMENT_SEAM` is recorded in code; flexcore-kernel is
untouched; this crate depends on no flexcore crate):

| Question (ledger step 1) | Recorded answer |
|---|---|
| Which keys sign | The signer key appears in the record as a scheme-prefixed REFERENCE only (`key-store:` / `env:` / `file:` — `MODEL_SIGNER_KEY_REFERENCE_SCHEMES`). The closed top-level key set (10 keys) makes key MATERIAL structurally impossible — there is no field a key could hide in; a reference without a scheme prefix is refused as likely inline key material and the value is never echoed. No release keys, no PKI: the production signing identity is a DEPLOYMENT REQUIREMENT (§3 decision point below) |
| How signatures are verified at load | The composed load gate `verify_model_artifact_at_load` (§4): P8.31 serving register FIRST → revocation register → provenance presence → ref binding → algorithm match → digest verification through the injected [`ModelArtifactDigestVerifier`] port. The DEFAULT verifier is `NoArtifactVerifier`, which refuses EVERYTHING (fail-closed — the absence of signing infrastructure can never look like success). `UnsafeTestOnlyArtifactDigest` (FNV-1a 64) is the loudly-labelled test implementation, never production-acceptable |
| How revocation is handled | The append-only `ModelArtifactRevocationRegister`: reason + actor REQUIRED (an unattributed revocation is refused, never recorded); exactly one record per artifact ref; NO un-revoke — the only direction implemented is the safe one (the kernel `UNREVOCATION_SEAM` honesty, mirrored). A revoked artifact is refused at load EVEN with valid provenance, and the refusal is audited |

The versioned record envelope is `flexcore-model-artifact-provenance-v1`
(`MODEL_PROVENANCE_SCHEMA_ID`; a different envelope id is refused, never
parsed permissively). Digest algorithm vocabulary is CLOSED
(`sha-256` / `fnv1a64-test`; anything else is
`MODEL_PROVENANCE_DIGEST_ALGORITHM_UNKNOWN`).

## 3. Provenance linked to the signature (ledger step 3)

The ledger requires provenance "linked to the signature, not stored
separately where it can drift". The record therefore carries the three
provenance references as REQUIRED fields — a record without them does not
parse, so the provenance cannot silently detach from the signature:

| Field | Linked provenance | Owner of the full record |
|---|---|---|
| `training-data-ref` | the training dataset the artifact was trained on | P8.35 (training-data provenance; this record carries the REFERENCE only) |
| `training-config-ref` | the training config + seed (the reproducibility inputs) | P8.20 (reproducibility standard) |
| `framework-version-ref` | the pinned framework versions (the P8.03 verified pair) | P8.03/P8.10 |

A successful load returns a `ModelArtifactLoadAcceptance` carrying all
three references plus the digest/signer identity — "every loaded artifact
carries linked provenance" is enforced by construction (there is no
accepted load without them).

**DECISION-POINT (recorded, not decided here): the production digest
verifier.** No crypto crate is added to this workspace in this wave (the
crate stays serde/serde_json only; root `Cargo.toml` untouched). DEFAULT:
defer the crate selection to the deployment wiring wave that wires the
verifier through the `ModelArtifactDigestVerifier` port at the composition
root, with the D4 license audit re-run at that wave (P7.03/P8.03
discipline). RECOMMENDATION: a maintained SHA-256 implementation from the
Rust Crypto project (e.g. the `sha2` crate, Apache-2.0/MIT dual-licensed)
— recorded as a RECOMMENDATION only, no version pin invented here, no
dependency added. Until a production verifier is wired, `NoArtifactVerifier`
refuses everything and NOTHING loads — that is the recorded safe default,
not a defect.

## 4. The composed load gate (ledger step 2) — and the honest F-4 answer

Order is deterministic; each step is a DISTINCT named refusal; EVERY
outcome (acceptance and refusal) is audited with a timestamp from the
injected clock:

| Step | Gate | Named refusal |
|---|---|---|
| 1 | P8.31 stamp binding re-verified | `RESEARCH_WORKER_STAMP_*` (the P8.31 names, delegated — audit queries span both gates) |
| 2 | P8.31 serving register: registration, scope, recorded cross-tenant approval | `RESEARCH_MODEL_ARTIFACT_UNKNOWN`, `TENANT_SCOPE_MISMATCH`, `RESEARCH_MODEL_SERVING_NOT_APPROVED` |
| 3 | Revocation register consulted | `MODEL_ARTIFACT_REVOKED` |
| 4 | Provenance record must EXIST | `MODEL_ARTIFACT_UNSIGNED` |
| 5 | Record binds the artifact being loaded | `MODEL_PROVENANCE_ARTIFACT_REF_MISMATCH` |
| 6 | Algorithm match + digest verification | `MODEL_ARTIFACT_VERIFICATION_REFUSED` (reason carries `MODEL_DIGEST_MISMATCH` / `MODEL_DIGEST_ALGORITHM_MISMATCH` / `MODEL_VERIFIER_REFUSED`) |

**F-4, addressed honestly at the RECORD level:** the security review's
INFO finding F-4 ("the model/policy artifact signing seam is P8.32's and
is NOT implemented; manifests are in-request stopgaps") is now answered by
THIS module — the flexcore-research-side record/verify types exist and the
seam has an implementation to reference. What F-4 does NOT become:
**the Python worker load sites are not touched and not wired** (they are
outside this subtask's file ownership and read-only here). The recorded
worker-load-site seam is `WORKER_LOAD_SITE_SEAM` (in code, pinned by the
record): the GNN/MARL with-framework activation waves wire these types
into the `apps/worker/python` load sites (findings F-1/F-2) and the ANDES
case path (F-3) records its allowed-format list then — and the hostile-
artifact cases are re-run at those gates. **The end-to-end load path is
NOT live in this delivery loop**, and no statement in this record or the
module header claims it. F-1 (the HIGH finding) remains OPEN — fixing it
is the P8.10/P8.11 owner's obligation at the DECISION-POINT resolution
(the allowed-format naming that must precede the torch pin); this subtask
neither fixes nor re-severities it.

## 5. The negative-test record (ledger step 4)

The suite lives in `crates/flexcore-research/src/artifact_signing.rs`
(in-module, 14 tests) and
`crates/flexcore-research/tests/artifact_signing_and_worker_identity.rs`
(6 P8.32 tests) — WRITTEN-UNRUN; the ledger's verification method
("tamper with a signed artifact and attempt load; confirm refusal") is the
human build/test gate plus a reviewer re-run. Hand-computed expectations:

| Test | Hostile input | Expected outcome (hand-computed) |
|---|---|---|
| `tampered_signed_artifact_is_refused_at_load_and_audited` (integration) | one changed artifact byte against the record's digest | `MODEL_ARTIFACT_VERIFICATION_REFUSED` with the `MODEL_DIGEST_MISMATCH` tag; audit order acceptance → refusal |
| `unsigned_artifact_is_refused_at_load_and_audited` (integration) | no provenance record | `MODEL_ARTIFACT_UNSIGNED`; one audited `MODEL_ARTIFACT_LOAD_REFUSED` |
| `revoked_artifact_is_refused_even_with_valid_provenance` (integration) | artifact in the revocation register | `MODEL_ARTIFACT_REVOKED`; audit order revocation-recorded → load-refused |
| `cross_tenant_load_is_refused_by_the_p831_gate_before_signing` (integration) | tenant-b worker, tenant-a-owned artifact | `RESEARCH_MODEL_SERVING_NOT_APPROVED` (P8.31 name delegated) — the signing gate never reached |
| `the_default_verifier_refuses_everything_fail_closed` (integration) | valid record + matching bytes under `NoArtifactVerifier` | `MODEL_ARTIFACT_VERIFICATION_REFUSED` with `MODEL_VERIFIER_REFUSED` |
| `the_test_grade_digest_of_empty_input_is_the_fnv_offset_basis` (in-module) | FNV-1a 64 over ZERO bytes | DEFINITIONAL: equals the offset basis `0xcbf29ce484222325` (no bytes processed); any other 16-digit hex is a Mismatch |
| in-module parse refusals | wrong envelope id / unknown field / missing linked-provenance field / inline signer key / unknown algorithm / unknown family | the six `MODEL_PROVENANCE_*` names; the inline key's VALUE never echoed |
| in-module verify refusals | tampered bytes / foreign artifact ref / algorithm mismatch | the three refusal tags |
| in-module revocation | empty reason/actor, duplicate | `MODEL_ARTIFACT_REVOCATION_INVALID` / `..._ALREADY_RECORDED`; audit order pinned |
| `the_error_name_table_equals_the_enum_names` (in-module) | the closed 17-name table vs the two enums | table == enum names, pairwise distinct |

The one numeric digest expectation is DEFINITIONAL (FNV-1a 64 over zero
bytes is the offset basis — no bytes are processed); every other digest
expectation is self-consistent (computed by the module's own test-grade
primitive over the same bytes the test verifies). No number is invented.

## 6. Reviewer verification procedure

1. Re-run the suites after the human build gate:
   `cargo test -p flexcore-research` — every §5 test name must be present;
   a missing name is a finding.
2. Confirm the closed vocabularies: the 10-key record key set, the
   2-algorithm digest set, the 3-scheme signer-reference set, and the
   17-name error table (`the_error_name_table_equals_the_enum_names`).
3. Confirm the fail-closed default by inspection: `NoArtifactVerifier` is
   the default and its `verify` returns `Refused` unconditionally; grep
   for any other verifier implementation in the crate (only the labelled
   `UnsafeTestOnlyArtifactDigest` may exist).
4. Confirm the F-4 honesty: `WORKER_LOAD_SITE_SEAM` is present in
   `artifact_signing.rs`; `apps/worker/python/**` shows NO diff in this
   wave (the worker load sites are untouched); no prose in this record or
   the module claims the end-to-end load path is live.
5. Confirm the linked-provenance construction: drop any of the three
   `*-ref` fields from a record — parsing must refuse
   (`MODEL_PROVENANCE_MISSING_FIELD`).
6. Confirm no crypto dependency was added: root `Cargo.toml` and
   `crates/flexcore-research/Cargo.toml` unchanged (serde/serde_json
   only); the decision point of §3 is recorded, not silently decided.
7. Confirm audit-on-both-outcomes and the injected clock only (no
   `SystemTime` in `artifact_signing.rs`).

## 7. Honest status

**WRITTEN-UNCOMPILED.** No cargo command was executed in this loop. The
negative-test record above is the WRITTEN suite with hand-computed
expectations, not a run record — there is no pass/fail evidence in this
file, and every run cell is EMPTY. The kernel P7.14 pattern, the plugin-host
P7.34 record posture, and the P8.31 integration surface were READ; the
research module mirrors them by citation with the alignment seams recorded
in code. flexcore-kernel, apps/plugin-host, apps/worker/python, and the
root `Cargo.toml` are untouched. The end-to-end model-artifact load path is
**NOT live**: signing/verification infrastructure (PKI, release keys, a
production SHA-256 verifier) is a DEPLOYMENT REQUIREMENT recorded as the
§3 decision point, and the worker load sites get the verification wired at
their with-framework activation wave.
