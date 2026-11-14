# P8.33 — Research-worker identity, mTLS, and secret handling

Date: 2026-11-14. Subtask: P8.33 (`ledger_phase_8.md` §P8.33). Owner role:
security engineer. Report trace: §17.3 (mTLS, short-lived credentials,
secrets manager; no secrets in plugin config or source control); §5.4
Class C. Inputs: P8.04 contracts (`04_worker_contracts.md`); P7.17
template; P7.34 AS REFERENCE (`apps/plugin-host/src/transport_trust.rs` —
read in full, mirrored by citation); the kernel secret-reference rule AS
REFERENCE (`crates/flexcore-kernel/src/manifest.rs` — read in the
secret-reference sections, mirrored by citation); the P8.31 worker-family
vocabulary (`crates/flexcore-research/src/tenant_isolation.rs`);
`25_security_review.md` finding S-1 (the gRPC-binding follow-up this
record carries).

## 1. Status of this record — WRITTEN-UNCOMPILED, nothing run

The implementation and its test suites exist and were written under the
loop bans: nothing installed, nothing built, no cargo command executed, no
test run (ENTRY_NOTE.md). Every expected value in the tests is
hand-computed from the recorded rules (the config-validation order, the
closed key sets, the scheme rules, the refusal names) or is a literal
copied from the module's own constants. NOTHING here is evidence that the
crate compiles or that any test passes; the human build/test gate
(AGENTS.md rule 7) owns that evidence. The ledger tick means
*implemented, awaiting human build/test*.

## 2. The identity model (ledger step 1) — closed vocabulary, deployment-bound

| Property | Recorded shape |
|---|---|
| Worker families | the CLOSED P8.31 vocabulary (`ResearchWorkerFamily`: admm_dmpc, gnn, marl, andes, helics, portfolio_analytics) — there is no "generic" worker identity that could bypass the per-family table of P8.31 §2 |
| Identity record | `ResearchWorkerIdentity`: worker instance id + family + deployment id + certificate/key REFERENCES (`Option<String>`, both REQUIRED at validation) — the P7.34 `HostServiceIdentity` pattern mirrored rule-for-rule; identity MATERIAL is a DEPLOYMENT REQUIREMENT, no PKI exists here, no key material ever enters the crate |
| Issuance/rotation owner | recorded CONFIGURATION (`rotation_owner`, validated non-empty) — never invented by code (the P7.34 posture) |
| Short-lived credentials | `session_credential_ttl` (positive TTL enforced; zero refuses) — the §17.3 short-lived posture; expiry monitoring MUST be enabled (disabled refuses) |
| Revocation | the append-only `WorkerIdentityRevocationRegister`: reason + actor REQUIRED, exactly one record per worker id, NO un-revoke (the safe direction only, the kernel `UNREVOCATION_SEAM` honesty); re-issue of a rotated identity is a NEW worker instance id, never an edit |

## 3. The mTLS transport-trust posture (config-shaped; the P7.34 mirror)

`WorkerMtlsTransportConfig::validate` mirrors
`apps/plugin-host/src/transport_trust.rs::MtlsTrustConfig::validate` RULE
FOR RULE, in the mirror's deterministic order (the FIRST refusal of a
multiply-broken config is pinned by tests): plaintext endpoint refused →
pinned server name required → PINNED platform CA reference required (never
the ambient system store) → positive session TTL → rotation owner recorded
→ expiry monitoring enabled → revocation checking configured. The worker
DIALS the platform; the platform never dials into the worker (Class C
posture, no inbound listener).

**What is a RECORDED SEAM (not implemented):** the gRPC-with-mTLS
transport itself. rustls is NOT added to this crate in this wave
(`RESEARCH_TRANSPORT_WIRING_SEAM`, the plugin-host wording restated: the
composition owner adds rustls (ring provider, exactly one provider
feature) + rustls-pemfile + the chosen gRPC stack at composition time, D4
note required, pins UNPINNED-REVERIFY). Nothing is installed; this module
ships configuration VALIDATION and the fail-closed material assembly
(`assemble_worker_trust_material`) only — no TLS handshake, no socket, no
PKI.

**The Phase 7 follow-up (recorded, never invented fixes):** the gRPC
binding of the research workers is NOT wired (`PHASE7_GRPC_BINDING_FOLLOWUP`,
carried from `25_security_review.md` finding S-1 verbatim in substance):
the research workers use the JSON-document transport discipline today, and
the P7.34 transport-identity wave covers them only when they bind the
servicer at their activation wave. This module is the config-shaped
posture they adopt then — never a parallel contract.

**Revocation enforcement honesty:** enforced here at ADMISSION time
(`verify_worker_admission`: material assembly fail-closed FIRST, then the
revocation register — a revoked worker is refused even with a perfect
config, audited on both outcomes). The SESSION-time enforcement (a revoked
identity gets no new session; in-flight work drained) is the platform-side
session gate over the recorded transport seam — no statement here claims
session-time coverage.

## 4. Secret handling — REFERENCES ONLY (ledger step 2)

The kernel manifest rule mirrored (`KERNEL_SECRET_SCHEME_ALIGNMENT_SEAM`):
every credential need (dataset access, model-store access, simulator
access, artifact-store access, telemetry access — the CLOSED
`WORKER_CREDENTIAL_NEEDS` vocabulary; there is no "other" bucket a secret
could hide in) is declared as a `WorkerSecretReference` — closed
`{name, reference}` object keys, the reference scheme-prefixed from the
closed `env:` / `file:` / `secret-store:` vocabulary, resolved BY THE HOST
at composition time. A scheme-less reference is refused as LIKELY INLINE
SECRET MATERIAL and the rejected VALUE is never echoed back. There is no
field anywhere in these types that can carry a secret VALUE.

**The literal-secret config refusal (ledger step 3):**
`scan_worker_config` walks the worker config JSON whole-tree (the worker
suites' own scan discipline, mirrored) and refuses any key whose
normalized form (lowercased, `-` folded to `_`) contains a
`FORBIDDEN_CONFIG_SECRET_KEYS` entry (closed 15-entry vocabulary), at ANY
depth — all violations collected, the FIRST reported sorted by
(location, key) so the refusal is deterministic. The sanctioned declaration
containers (`secrets`, `secret_references`) are EXEMPT from the key check
(the kernel manifest declares the same key legitimately) but their
children are still scanned and the declarations themselves are validated
by `parse_secret_declarations` — the exemption opens no value channel.
Both the refusal and the acceptance are audited
(`scan_worker_config_audited`), and the audit detail carries the refused
KEY NAME — never any config VALUE.

**The honest residual (recorded in the module header, pinned by a test):**
the scan refuses the forbidden KEY vocabulary; it cannot detect a secret
hidden under an innocuous key name (the test pins that the scan accepts
the shape it cannot judge, rather than claiming detection). The recorded
mitigation is structural: the only sanctioned way to name a credential
need is the scheme-prefixed reference path, the closed config schema the
workers ship with declares those needs, and every refusal is audited. No
statement here claims the scan proves absence of secrets.

## 5. The negative-test record (ledger step 3)

The suite lives in `crates/flexcore-research/src/worker_identity.rs`
(in-module, 13 tests) and
`crates/flexcore-research/tests/artifact_signing_and_worker_identity.rs`
(4 P8.33 tests) — WRITTEN-UNRUN; the ledger's verification method
("inspect worker configs and logs for secret values; attempt connection
with a revoked identity") is the human build/test gate plus a reviewer
re-run. Hand-computed expectations:

| Test | Hostile input | Expected outcome (hand-computed) |
|---|---|---|
| `a_revoked_worker_identity_is_refused_at_admission` (integration) | worker id in the revocation register | `WORKER_IDENTITY_REVOKED`; audit order revocation-recorded → admission-refused |
| `a_worker_config_with_a_literal_secret_is_rejected_and_audited_without_echoing_the_value` (integration) | config with `"db_password": "hunter2-..."` | `WORKER_CONFIG_SECRET_KEY_REFUSED`; audit detail contains the KEY, never the VALUE; the error text likewise |
| `a_schemeless_credential_reference_is_refused_as_likely_inline_secret` (integration) | declaration reference without a scheme prefix | `WORKER_SECRET_REFERENCE_INVALID`; the VALUE never echoed; the sanctioned form parses |
| `an_incomplete_identity_names_the_missing_piece` (in-module) | missing/blank certificate or key reference | `WORKER_IDENTITY_INCOMPLETE` with the field named |
| `the_channel_config_refuses_each_broken_rule_in_order` (in-module) | each of the 7 broken rules, one at a time + a multiply-broken config | the seven `WORKER_IDENTITY_*` names; the FIRST rule wins deterministically |
| `a_missing_identity_is_fail_closed_never_anonymous` (in-module) | empty identity store | `WORKER_IDENTITY_MISSING` |
| `a_revoked_worker_identity_is_refused_at_admission_and_audited` (in-module) | revoked id, valid config | `WORKER_IDENTITY_REVOKED`; audit order pinned; timestamps from the injected clock |
| `revocation_requires_reason_and_actor_and_refuses_duplicates` (in-module) | empty reason/actor/worker id, duplicate | `WORKER_IDENTITY_REVOCATION_INVALID` / `..._ALREADY_RECORDED` |
| `secret_declarations_accept_scheme_references_and_refuse_inline_material` (in-module) | scheme-less ref, unknown need name, duplicate name, smuggled `value` key | `WORKER_SECRET_REFERENCE_INVALID` / `WORKER_SECRET_NAME_DUPLICATE` / `WORKER_SECRET_DECLARATION_INVALID`; the VALUE never echoed |
| `a_literal_secret_config_is_refused_at_any_depth_and_audited` (in-module) | forbidden keys at depth, hyphen/case-folded variants | `WORKER_CONFIG_SECRET_KEY_REFUSED`; the VALUE never in the audit detail or error text |
| `the_error_name_table_equals_the_enum_names` (in-module) | the closed 19-name table vs the enum | table == enum names, pairwise distinct |

## 6. Contract recording and the recorded Phase 7 follow-ups (gaps, never invented fixes)

The ledger's inputs cite P8.04 contracts; the worker-contract ARTIFACTS
themselves are NOT edited in this subtask (outside this subtask's file
ownership; ENTRY_NOTE boundary 3 forbids inventing parallel contracts).
Recorded honestly as follow-ups:

| # | Phase 7 follow-up | Owner / activation point |
|---|---|---|
| F-1 | The research workers' gRPC binding is not wired (JSON-document transport today; buf generation is the activation step) — until the workers bind the servicer, this module's mTLS posture is CONFIG-SHAPED ONLY and the workers are not transport-identity-covered | the with-framework activation wave (carried from `25_security_review.md` S-1; recorded in code as `PHASE7_GRPC_BINDING_FOLLOWUP`) |
| F-2 | `python/workers/contracts/worker_api.proto` carries no identity/credential-declaration fields on the request envelope; the identity material and secret references live in DEPLOYMENT configuration (the P7.34 shape). If a future deployment needs them ON the wire, that is a Phase 7 contract extension (recorded decision + buf governance per §10.5), never a parallel contract | Phase 7 contract owners; only on a recorded deployment need |
| F-3 | The P8.31 record's TF-2 (worker contracts citing the scoping rule) extends to this module: the worker contract documentation should also cite the identity/secret rule of §4 — a P8.04-record update | P8.04 owner |
| F-4 | The rustls/gRPC wiring seam (`RESEARCH_TRANSPORT_WIRING_SEAM`) carries UNPINNED-REVERIFY pins only; the P7.03/P8.03 license audit re-verifies at the wiring wave | composition owner + D4 audit note |

## 7. Reviewer verification procedure

1. Re-run the suites after the human build gate:
   `cargo test -p flexcore-research` — every §5 test name must be present;
   a missing name is a finding.
2. Confirm the closed vocabularies: the 3-scheme reference set, the 15-entry
   forbidden-key set, the 5-entry credential-need set, the 2-token TLS
   minimum, the sanctioned container exemptions, and the 19-name error
   table (`the_error_name_table_equals_the_enum_names`).
3. Confirm the no-secret-in-logs property by inspection: grep the audit
   emission sites and error `Display` impls — a rejected secret-reference
   VALUE and any config VALUE must never be formatted (the tests pin the
   specific cases; the reviewer confirms no other emission site exists).
4. Confirm the admission door order by inspection: material assembly
   fail-closed FIRST, revocation SECOND; every outcome audited; timestamps
   only from the injected clock (no `SystemTime` in `worker_identity.rs`).
5. Confirm the honesty seams are present in code:
   `RESEARCH_TRANSPORT_WIRING_SEAM`, `PHASE7_GRPC_BINDING_FOLLOWUP`,
   `WORKER_IDENTITY_STATEMENT`, `PLUGIN_HOST_TRUST_ALIGNMENT_SEAM`,
   `KERNEL_SECRET_SCHEME_ALIGNMENT_SEAM` — and that nothing was installed
   (no rustls/gRPC dependency in any `Cargo.toml`; the crate stays
   serde/serde_json only).
6. Confirm the residual is pinned: the test asserting the scan accepts the
   shape it cannot judge (`the_scan_residual...` branch of the innocuous-
   config test) is present — no prose claims the scan proves absence of
   secrets.

## 8. Honest status

**WRITTEN-UNCOMPILED.** No cargo command was executed in this loop. The
negative-test record above is the WRITTEN suite with hand-computed
expectations, not a run record — there is no pass/fail evidence in this
file, and every run cell is EMPTY. The plugin-host P7.34 pattern and the
kernel manifest secret-reference rule were READ; the research module
mirrors them by citation with the alignment seams recorded in code.
apps/plugin-host, crates/flexcore-kernel, apps/worker/python, and the root
`Cargo.toml` are untouched. mTLS is a CONFIG-SHAPED posture — no TLS
handshake, no socket, no PKI, nothing installed — and the research workers
are NOT transport-identity-covered until they bind the gRPC servicer at
their activation wave (the recorded Phase 7 follow-up).
