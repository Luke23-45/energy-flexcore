# FIM Final Analysis — Necessary Features and Findings Before the Release Gate

**Program:** Final analysis pass of the Final Implementation Milestone (FIM)
program — the static, read-only review of the FIM output for the Neryva
FlexCore initial public release gate.
**Created:** 2026-11-21. **Review ID scheme:** `FIM2-R1-NNN` (this pass;
findings vocabulary reused from
`docs/reviews/final_review_for_v1/README.md` §4).
**Method:** STATIC READ ONLY — no build, no test execution, no installation.
Every finding cites `file:line` evidence; nothing below is "verified by
running". All findings are **OPEN** as written; the fix dispositions follow
the FRV1 status rule (`OPEN` → `FIXED (written, awaiting human build/test)`),
and the first human `cargo build --workspace` remains the real gate.
**Status: COMPLETE (written) — 11 findings (3 P0 / 4 P1 / 4 P2), 1 proposed
necessary feature (written-unrun), 6 considered-and-rejected. E-decisions stay
OPEN; no capability below needs a counterparty, jurisdiction, or field pilot.**

---

## 1. Program summary — what now exists that did not at b8012e7

Between `b8012e7` and the current working tree the FIM program implemented,
written-unrun, all 15 proposed pre-publication features of
`docs/releases/final-implementation-before-publication.md` §5 (F-01..F-15)
plus the three §3 implementation items (V-8 race-regression test, V-9
Postgres audit-chain upgrade to the audit-crate HMAC-SHA-256 construction,
V-10 removal of the signing-path `.expect()`), the control-plane composition
wiring (`fim_compose` in `apps/control-plane/src/main.rs` — six
`FLEXCORE_*_ENABLED` configuration gates over the OIDC resolver, SCIM
service + bearer guard, step-up verifier, machine-identity and API-key
services, webhook HTTP transport, and the OTA rollout coordinator), the
CGMES class-spelling remediation (record 44a addendum + alias resolution in
`crates/adapters/protocol-cim/src/spelling_remediation.rs`), and fixes for
four pre-existing Python self-test defects. The hand-written wire codecs the
dependency policy mandated all exist and are pinned by hand-computed vector
tests: RFC 6455 WebSocket + a pinned FIPS 180-4 SHA-1 (OCPP), MQTT 5 +
Sparkplug B protobuf encoders, an RDF/XML pull parser (CIM), Modbus TCP
MBAP/PDU framing, and a minimal DER reader for OCSP. New identity surfaces
(machine-identity client-credentials JWTs, per-tenant API keys with rotation
grace, SCIM 2.0 endpoints with a deprovisioning-enforcing resolver wrapper)
and new OTA surfaces (TUF-shaped bundle producer, Ed25519 signing with
domain separation, device-side A/B slot bookkeeping, cohort rollout
coordinator over the existing P18.06 orchestrator) carry honesty headers
throughout. The frozen invariants are untouched: `git status` shows no
modification to the command-lifecycle, the authz `Action` surface, the
telemetry quality states, the canonical units, `deny.toml`, the kernel
policy, or the audit-crate chain; the BESS and `source_auth` edits are the
recorded F-11/V-10 targets. The release-notes draft and the foundations
documents are unmodified; their NOT-QUALIFIED verdicts stand.

## 2. What this pass reviewed and how

The review followed the FRV1 methodology (`docs/reviews/final_review_for_v1/README.md`
§§1-7): static file-by-file reading of the highest-risk seams named by the
task — the hand-written wire codecs (`crates/adapters/protocol-ocpp/src/
websocket.rs`, `sha1.rs`, `pnc_ocsp.rs`, `dispatch.rs`, `transport.rs`;
`crates/adapters/protocol-mqtt/src/mqtt_wire.rs`, `sparkplug_wire.rs`,
`live.rs`; `crates/adapters/protocol-cim/src/xml_pull.rs`, `xml_profile.rs`;
`crates/adapters/protocol-modbus/src/codec.rs`; `crates/adapters/
protocol-20305/src/transport.rs`), the identity services
(`apps/control-plane/src/machine_identity.rs`, `api_keys.rs`, `scim.rs`,
`scim_store.rs`, the two route modules, `routes.rs::fim_router`), the OTA
modules (`crates/edge-agent/src/ota_bundle_produce.rs`, `ota_signing.rs`,
`ota_slots.rs`, `zero_touch_claim.rs`,
`apps/control-plane/src/ota_rollout.rs`), and the composition wiring
(`apps/control-plane/src/main.rs` `fim_compose`) — plus cross-checks of
signature vs call sites, configuration-gate coverage, error-vocabulary
reuse, frozen-invariant adjacency (via `git status`/`git diff` scoping), and
honesty-header accuracy. The remaining FIM surfaces (20305 serving, OpenADR
serving/transport, CIM import, worker vendor ingestion and forecast
evaluation, DOE wiring, BESS rainflow/cost-curve/SoH/warranty, Python OTEL
instrumentation, the VXX items) were read at header + surface level. Cross
-module mismatches are recorded as static findings, never as "compiles".

## 3. Findings

Ordered P0 > P1 > P2. Severity classes are FRV1's: P0 = release blocker
(would-not-compile class, broken frozen invariant, honesty violation,
security defect, data-loss path); P1 = important (missing implementation,
unwired seam, API/client mismatch, missing error path, stale reference); P2
= polish (docs, naming, consistency, comment drift).

| ID | Sev | Area | One-line summary |
|---|---|---|---|
| FIM2-R1-001 | P0 | api_keys | `issue_key` moves `scopes` twice — the crate does not compile |
| FIM2-R1-002 | P0 | machine_identity | `revoke_client` can never succeed — the store trait has no update path |
| FIM2-R1-003 | P0 | ocpp dispatch | malformed-payload honesty claim is false; the pinned test fails; the reply builders are dead code |
| FIM2-R1-004 | P1 | scim composition | resolver and service use two different `InMemoryScimStore` instances — the composed identity path denies everything |
| FIM2-R1-005 | P1 | policy composition | the apikey.* admin surfaces have no grant path in the shipped binary; the main.rs FRV1-R2-009 claim is now false |
| FIM2-R1-006 | P1 | api_keys | revoked key's superseded secret refuses as `UnknownKey`, contradicting the doc and the pinned test |
| FIM2-R1-007 | P1 | sparkplug wire | the declared field-mapping table very likely disagrees with the published Sparkplug B proto; fixtures pin the wrong numbers |
| FIM2-R1-008 | P2 | ledger docs | ledger status lines contradict their own ticked tables |
| FIM2-R1-009 | P2 | machine_identity | `secret_prefix` doc describes an `fkint_` prefix the code does not use |
| FIM2-R1-010 | P2 | scim | module doc still shows the `nest("/scim")` example the composition explicitly does not use |
| FIM2-R1-011 | P2 | doe wiring | the absent optimizer-assembly call site is recorded for F-11 but not for F-14 |

### FIM2-R1-001 (P0) — `issue_key` moves `scopes` twice; use-after-move

- **Evidence:** `apps/control-plane/src/api_keys.rs:666` — `scopes,` inside
  the `ApiKeyRecord` literal moves the parameter `Vec<String>`;
  `apps/control-plane/src/api_keys.rs:683` — `scopes: scopes,` inside the
  returned `IssuedApiKey` uses it again. This is E0382 (`use of moved
  value`): `issue_key` cannot compile, so `-p flexcore-control-plane` fails
  at the human gate. The sibling `rotate_key` does it correctly
  (`api_keys.rs:709` — `let scopes = record.scopes.clone();` before the
  `..record` update), which confirms the intent.
- **Class:** would-not-compile (P0 under FRV1 §4).
- **Minimal fix:** in the record literal use `scopes: scopes.clone()`
  (leaving line 683 as-is), or clone for the response — one line, mirrors
  `rotate_key`.

### FIM2-R1-002 (P0) — machine-client revocation is a dead path

- **Evidence:** `MachineClientStore` has exactly two methods — `put`
  (insert-only) and `get` (`apps/control-plane/src/machine_identity.rs:361-374`);
  the shipped implementation refuses an existing `client_id` with
  `E_CLIENT_EXISTS` (`machine_identity.rs:397-405`, "registration is
  explicit, never an upsert"). `revoke_client` performs its update THROUGH
  `put` (`machine_identity.rs:739-744`), so every revocation — the record 07
  §4 emergency kill-switch — returns `E_CLIENT_EXISTS` and the epoch never
  bumps; a pre-revocation token therefore stays valid until `exp`. Both the
  unit test (`machine_identity.rs:854` — `.expect("revoked")`) and the
  integration test (`apps/control-plane/tests/phase13_fim_machine_identity.rs:164`)
  would fail at the human gate. The API-key store solved the same shape with
  a separate `update` method (`api_keys.rs:371-372`, `434-448`); the
  machine-identity trait did not get one.
- **Class:** broken security control + would-fail-at-gate (P0).
- **Minimal fix:** add `fn update(&self, record: MachineClientRecord) ->
  Result<(), MachineIdentityError>;` to `MachineClientStore` (refuse when
  absent, insert over the key when present — mirror `MemoryApiKeyStore::update`),
  implement it on `MemoryMachineClientStore`, and call it from
  `revoke_client`. `put` stays insert-only.

### FIM2-R1-003 (P0) — OCPP dispatch: malformed-payload honesty claim is false and its pinned test fails

- **Evidence:** the `dispatch.rs` module header item 2 records that "a
  malformed payload for a KNOWN action is answered with the recorded
  CALLERROR code `FormationViolation` (decode failure) or
  `PropertyConstraintViolation` (validation failure) rather than tearing the
  connection" (`crates/adapters/protocol-ocpp/src/dispatch.rs:54-59`). The
  code does the opposite: every known-action decode failure propagates —
  `answer_boot` (`dispatch.rs:374-379`), and `decode` for
  StatusNotification/TransactionEvent/SecurityEventNotification
  (`dispatch.rs:322-337`, `492-501`) — and the transport converts any
  `DispatchError` into a connection CLOSE with 1008
  (`crates/adapters/protocol-ocpp/src/transport.rs:336-351`). The reply
  builders that would implement the recorded behavior,
  `formation_violation_reply` / `property_constraint_reply`
  (`dispatch.rs:506-538`), have no call site anywhere (grep-verified — the
  definitions are their only occurrences). The pinned test
  `a_malformed_boot_payload_is_answered_with_the_recorded_formation_violation`
  (`dispatch.rs:687-714`) asserts a `FormationViolation` CALLERROR reply and
  would panic at `.expect("answered")` because the decode error surfaces as
  `Err`. The module's honesty header therefore states a behavior the module
  does not have — an honesty violation by the FRV1 definition, and a test
  that fails at the human gate.
- **Minimal fix:** in `handle_inbound_text`, catch the
  `DispatchError::Payload` case for a decoded CALL and return the
  `formation_violation_reply(unique_id, &action)` frame instead of the error
  (the helpers already exist; the OCPP-J recorded semantics are preserved;
  the header and the test become true). The alternative — rewriting the
  header and test to record the close behavior — is weaker: it abandons the
  recorded OCPP-J answer-with-CALLERROR shape and keeps the close path as
  the malformed-payload answer.

### FIM2-R1-004 (P1) — two different SCIM stores in the composed identity path

- **Evidence:** `fim_compose::build_identity_resolver` constructs its own
  store for the deprovisioning-enforcement wrapper —
  `apps/control-plane/src/main.rs:1147`: `let store: Arc<dyn ScimStore> =
  Arc::new(InMemoryScimStore::default());` — while `build_scim_service`
  constructs a DIFFERENT store for the SCIM HTTP surface
  (`main.rs:1171-1176`). `ScimAwareIdentityResolver::resolve` denies unless
  the subject exists in ITS store (`apps/control-plane/src/scim.rs:799-806`).
  With `FLEXCORE_OIDC_ENABLED=true` and `FLEXCORE_SCIM_ENABLED=true`, the
  composed binary therefore denies EVERY token (the resolver's store is
  empty forever — nothing writes to it), and a user deactivated through the
  SCIM API cannot propagate to the resolver either. Fail-closed, so not a
  security hole — but the recorded F-07 composition semantics (a provisioned
  user resolves with the SCIM group→role mapping; deactivation rejects at
  the resolver) cannot work in the shipped composition. The pinned
  composition tests do not catch it: `tests/fim_composition.rs:349-351`
  builds its own store for the resolver test, and the wrap test
  (`fim_composition.rs`, `scim_wrap_denies_when_the_inner_resolver_denies`)
  only exercises the deny path.
- **Class:** unwired seam inside the composition root (P1).
- **Minimal fix:** compose ONE store: build the `Arc<dyn ScimStore>` once
  (in `main.rs` or in a shared `fim_compose` helper), pass it to BOTH
  `ScimAwareIdentityResolver::new` and `ScimService::new`, and pin the share
  with a test that provisions through the service and resolves through the
  wrapper.

### FIM2-R1-005 (P1) — the apikey.* admin surfaces have no grant path in the shipped binary; the main.rs claim is stale

- **Evidence:** the FIM Wave 8B routes evaluate
  `Action::ApiKeyCreate` / `ApiKeyRotate` / `ApiKeyRevoke` against
  `state.policies` — `apps/control-plane/src/machine_identity_routes.rs:359`
  and `:489`, `apps/control-plane/src/api_key_routes.rs:328`, `:357`, `:384`
  — but the composed policy set is `phase5_default_policies()`
  (`apps/control-plane/src/auth.rs:500-520` = phase4 + `alarm.read`; zero
  apikey grants — grep-verified: no `ApiKey` occurrence anywhere in
  `auth.rs`), and `flexcore_authz::phase13_policies` (which carries the
  apikey.* grants, `crates/flexcore-authz/src/phase13_policies.rs:69-105`)
  is composed by NO configuration — only by the route tests
  (`apps/control-plane/tests/fim_identity_routes.rs:247,281,428,596`). The
  deny-under-shipped-defaults posture is itself recorded and pinned
  (`machine_identity_routes.rs:35-46` guard note;
  `fim_identity_routes.rs:670-724` pins the 403). Two things are still wrong:
  (a) the recorded operation path — "a deployment that composes the recorded
  phase-13 grant set operates them"
  (`machine_identity_routes.rs:45-46`) — does not exist: no environment
  variable or composition branch composes those grants, so enabling
  `FLEXCORE_API_KEYS_ENABLED` or `FLEXCORE_MACHINE_IDENTITY_ENABLED` yields
  permanently-403 admin surfaces in the shipped binary; and (b) the
  composition-root comment `main.rs:224-230` still says "apikey.*/market.*
  have no routes", which is now factually false — a REST surface in this
  binary DOES evaluate apikey.*. The FIM ledgers tick FIM-F08.01/FIM-F08.02
  as implemented without recording the unoperable-admin-surface posture.
- **Class:** unwired seam + stale reference (P1). The deny is deliberate;
  the missing grant path and the stale claim are not acceptable to leave
  silent at a release gate.
- **Minimal fix (pick one, record either way):** (a) compose the apikey.*
  grants behind the same configuration gates — e.g. an additive
  `phase13_credential_policies()` accessor (the apikey.* slice only, NOT the
  SiteApprover command.approve grants, which stay unrouted per FRV1-R2-009)
  unioned into the policy set when the machine-identity/api-keys sections
  are enabled — and update the pinned test and the main.rs comment; or
  (b) keep the shipped deny, and record the unoperable posture in the FIM
  ledger README (F-08 rows) plus fix the `main.rs:224-230` comment to state
  that the routes now exist and are deny-pinned. Option (a) is the stronger
  release outcome; (b) is the honesty minimum.

### FIM2-R1-006 (P1) — revocation of a rotated key refuses the old secret as `UnknownKey`, contradicting doc + test

- **Evidence:** `ApiKeyService::revoke_key` clears the superseded verifier
  (`apps/control-plane/src/api_keys.rs:808-815`). After a rotate-then-revoke,
  authenticating with the OLD (superseded) secret finds neither the current
  verifier nor the superseded one and returns `ApiKeyError::UnknownKey`
  (`api_keys.rs:750-757`) — but the doc comment promises `KeyRevoked` for
  "either secret" (`api_keys.rs:800-802`) and the pinned test
  `revocation_is_immediate_for_both_secrets` asserts `KeyRevoked` for the
  old secret (`api_keys.rs:944-955`) — the test fails at the human gate.
  Both refusals fail closed (no security hole), but the recorded semantics
  and the pin disagree with the code.
- **Minimal fix:** retain the superseded fields on revoke (do not clear
  them): the state check `record.state == "REVOKED"` runs before the grace
  check for BOTH lookup paths (`api_keys.rs:760-771`), so the old secret
  then refuses `KeyRevoked` exactly as documented, and `validate()` stays
  satisfied (Some/None agreement kept). One-line deletion of the two field
  overrides plus a test comment.

### FIM2-R1-007 (P1) — the Sparkplug B declared field table very likely disagrees with the published proto

- **Evidence and provenance:** `crates/adapters/protocol-mqtt/src/
  sparkplug_wire.rs` declares, as an explicitly UNVERIFIED design pin
  (module header lines 19-25): metric value fields boolean=6 / long=7 /
  double=8 / string=9 (`sparkplug_wire.rs:66-73`), type codes Double=7 /
  String=8 (`sparkplug_wire.rs:79-80`), and `bdSeq` as payload field 5
  (`sparkplug_wire.rs:57`). A secondary-source reading of the commonly
  published Eclipse Tahu `sparkplug_b.proto` gives DIFFERENT numbers:
  Metric value fields `double_value=10`, `long_value=12`,
  `boolean_value=14`, `string_value=15` (fields 6-9 are `isTransient`,
  `isNull`, `metadata`, `properties`); MetricDataType `Double=11`,
  `String=12` (7=UInt16, 8=UInt32); and `bdSeq` travelling as a METRIC
  NAMED `bdSeq` (UInt64) inside the payload, while payload field 5 is
  `bytes body`. I cannot verify the primary text inside this repository
  (no network, no spec text in-tree — the same boundary the module header
  records), so the corrected numbers are themselves UNVERIFIED until the
  recorded implementation-time check against the .proto text. What is
  verifiable statically is the COST of being wrong: the byte fixtures in
  `sparkplug_wire.rs:575-645`, the fixture suite
  `crates/adapters/protocol-mqtt/tests/sparkplug_wire_fixtures.rs`, and the
  NDEATH LWT bytes all PIN the declared table, and `live.rs` builds birth/
  data/death payloads through it — if the published proto is confirmed, a
  real Sparkplug B consumer misreads every metric (the "type" code 7 reads
  as UInt16; the double value rides field 8, which the real proto reads as
  `metadata`), and the death certificate's bdSeq is unreadable. The module
  honestly labels the table UNVERIFIED, so this is not an honesty violation
  — but the lab gate would discover a table-and-fixture rewrite that is far
  cheaper to do now, from the primary text, as one recorded edit.
- **Minimal fix:** perform the module's own recorded implementation-time
  check NOW (fetch the Tahu `sparkplug_b.proto` at the human gate or attach
  the verified table to record 44a's discipline), then update the constants
  and the pinned fixtures in one recorded edit before the interop lab gate.
  If the published numbers above are confirmed, the bdSeq mechanism moves
  from a payload field to a named metric and the DEATH fixture changes
  shape.

### FIM2-R1-008 (P2) — ledger status lines contradict their own tables

- **Evidence:** `docs/releases/implementation_ledger/README.md:4-5` says
  "all 15 features implemented, 92/96 subtasks ticked; V-8/V-9/V-10 in
  flight" while all four VXX subtask rows are ticked "fixed (written,
  awaiting human build/test) — wave 8"
  (`docs/releases/implementation_ledger/ledger_VXX.md:9-12`); and the
  trailing status line of `ledger_VXX.md:16` still reads "Status: NOT
  STARTED".
- **Minimal fix:** README → "96/96 ticked"; `ledger_VXX.md` trailing status
  → the same implemented-written wording the other ledgers carry.

### FIM2-R1-009 (P2) — `secret_prefix` doc drift

- **Evidence:** `MachineClientRecord.secret_prefix` is documented as "the
  first 4 characters after the `fkint_` prefix — the fingerprint form
  record 08 §4 prescribes" (`apps/control-plane/src/machine_identity.rs:273-275`),
  but machine-client secrets are caller-supplied with no `fkint_` prefix
  (that grammar belongs to the API-key tier) and the code takes the first 4
  characters verbatim (`machine_identity.rs:566`).
- **Minimal fix:** reword the comment to "the first 4 characters of the
  caller-supplied secret" (the struct field comment at
  `machine_identity.rs:291-293` already says this).

### FIM2-R1-010 (P2) — SCIM module doc shows a nest example the composition does not use

- **Evidence:** `apps/control-plane/src/scim.rs:152-156` still documents
  "e.g. `Router::new().nest("/scim", scim_router(...))`", while the actual
  composition merges at the root precisely because the path constants carry
  the `/scim/v2` prefix (`apps/control-plane/src/routes.rs:126-128` —
  "nesting it under `/scim` would double the prefix"). A deployment author
  following the module doc would mount a doubled prefix and break the meta
  `location` values.
- **Minimal fix:** update the `scim.rs` doc comment to the merge-at-root
  rule (one sentence), citing `routes.rs::fim_router`.

### FIM2-R1-011 (P2) — record the absent optimizer-assembly call site for the F-14 wiring

- **Evidence:** the F-14 wiring functions
  (`crates/flexcore-application/src/doe_constraint_wiring.rs:533`, `:691`,
  `:744`, `:815`, `:900`) are exported (`crates/flexcore-application/src/lib.rs:723-725`)
  and pinned by tests, but no scheduler/optimizer assembly calls them
  (grep-verified: the only consumers are the module itself and its tests;
  no `ObjectiveInputs` assembly exists in any binary — the type's only
  non-test references are its definition `crates/flexcore-application/src/
  scheduling_objective.rs:1057` and the control-plane mapping). For F-11 the
  equivalent posture IS recorded at the composition root
  (`apps/control-plane/src/main.rs:1358-1380`, "the future optimization
  assembly"); for F-14 the module header states the override rule but never
  states that no assembly site consumes it yet, so the ledger's
  "Optimizer constraint wiring ... fixed" reads as if the dispatch path now
  respects DOE feeds.
- **Minimal fix:** add the F-14 posture statement (one paragraph, either in
  `doe_constraint_wiring.rs`'s header or in the F-14 ledger row) mirroring
  the F-11 wording: the wiring is composed now and pinned by tests; the
  assembly site that applies it to a runnable optimization is the recorded
  remaining step (see the proposed N-2 below).

## 4. New features necessary

Only capabilities genuinely missing for the release gate, implementable
written-unrun without new dependencies (workspace-internal additions only;
no new crate enters the resolution graph), are listed. Each names the owning
file/crate and acceptance criteria; the considered-and-REJECTED list follows
in §5.6's style.

### N-1 — An operator surface that drives the composed OTA rollout coordinator

- **Why necessary:** `build_ota_rollout` composes the coordinator into
  `AppState.ota_rollout` behind `FLEXCORE_OTA_ROLLOUT_ENABLED`
  (`apps/control-plane/src/main.rs:1303-1341`, `state.rs:283-292`), and
  `state.rs:288-291` honestly records that "no route serves device reports
  in this binary — that transport is the named remaining wiring step". The
  consequence is that the composed F-12 control-plane half is unreachable:
  no operator can start, advance, authorize, or report into the rollout,
  and the canary/abort semantics (F-12 acceptance criteria 3-4) cannot be
  exercised outside a test. For a release that lists staged OTA rollout as
  a capability, the composed coordinator needs its driver surface.
- **Owning code:** NEW `apps/control-plane/src/ota_rollout_routes.rs`
  (mirroring `machine_identity_routes.rs`'s structure), mounted by the
  existing `routes.rs::fim_router` match on `state.ota_rollout`; a credential
  section in `fim_compose` (a SHA-256 credential digest carried the way the
  SCIM bearer digest is, `main.rs:924-931` — NO new `Action` variant: the
  closed `flexcore-authz` surface has no OTA action, and adding one is out
  of scope); handlers call the coordinator through its mutex —
  `start`, `authorize_device`, `record_outcome` (the `OtaReportReceiver`
  port, `apps/control-plane/src/ota_rollout.rs:654+`), `advance_cohort`,
  and a read-only snapshot — with every refusal mapped onto the existing
  `ApiError` vocabulary.
- **Acceptance criteria (written-unrun, pinned by socket-free tests):**
  (1) absent `FLEXCORE_OTA_ROLLOUT_ENABLED` ⇒ the routes do not exist (the
  `fim_router` pattern; pinned like `tests/fim_composition.rs`); (2) a
  request carrying the composed credential drives
  start → authorize → report → advance through the composed coordinator and
  every other caller is refused 401 with no detail (the `scim_bearer_guard`
  shape); (3) every attempt lands in the composed audit sinks
  (`MemoryRolloutAuditSink` / `MemoryOtaRolloutAuditSink` — the FIM-F12.05
  records); (4) the coordinator's outputs remain rollout decisions and
  audit records, never device commands (the P18.06 posture; ADR-010 and the
  14-state lifecycle untouched); (5) a threshold tripwire freezes the
  rollout through the route exactly as the coordinator records
  (`OTA_ROLLOUT_FROZEN`).
- **Posture:** WRITTEN-UNRUN like every FIM surface; the durable Postgres
  audit-sink wiring stays the recorded remaining step.

### N-2 — An optimization-assembly consumption step for the F-11 cost curve and the F-14 bounds

- **Why necessary:** F-11's `C_degradation` mapping
  (`main.rs:1430-1441`) and F-14's connection-point bounds
  (`doe_constraint_wiring.rs:691,900`) exist as tested library/composition
  functions, but no runnable composition assembles `ObjectiveInputs` at all
  (grep-verified: only the definition, the domain tests, and the
  control-plane mapping reference it). Until one exists, neither "the cost
  curve changes the optimizer's `C_degradation` total vs the zero
  placeholder" (F-11 criterion 2) nor "a live envelope constrains a
  dispatch that the static limit would have allowed" (F-14 criterion 1) is
  exercisable outside unit tests — the features remain data halves.
- **Owning code:** the first runnable assembly site — recommended:
  `apps/sim-runner` (it already depends on `flexcore-application`
  (`apps/sim-runner/Cargo.toml:45`) and drives the command-gate scenarios;
  adding the workspace-internal `flexcore-optimization-api` dependency puts
  no new crate into the resolution graph). The step is exactly the one
  recorded in `main.rs:1358-1380`: convert
  `MappedDegradationCostDeclaration` fields into
  `DegradationCostDeclaration::new(...)` and fill
  `ObjectiveInputs.degradation_cost_curves`; call
  `resolve_connection_point_constraints` +
  `apply_connection_point_bounds` for the per-connection-point MW pair.
- **Acceptance criteria (written-unrun, pinned by a sim-runner scenario
  test):** (1) a scenario with a declared measured curve produces a
  `C_degradation` total strictly greater than the same scenario's zero
  placeholder; (2) a scenario with an in-force envelope produces a dispatch
  the static limit alone would have allowed, clipped to the envelope pair,
  with the reversion telemetered after expiry; (3) the mapping is
  per-asset/per-connection-point exactly as the recorded granularities
  state; (4) solver classes untouched (HiGHS LP/QP/MIP, Clarabel conic —
  AGENTS.md rule 4).

## 5. Considered and REJECTED — with reasons

Following `docs/releases/final-implementation-before-publication.md` §5.6's
style; each stays rejected unless its recorded revisit condition fires.

- **Postgres-backed SCIM / machine-identity / API-key stores — not a
  release-gate feature.** All three services record the Postgres-backed
  store as the `Unwired*` fail-closed deployment seam
  (`scim_store.rs` header; `machine_identity.rs:83-88`; `api_keys.rs:77-83`).
  Building them needs schema migrations + RLS design (migration-surface
  work the FIM explicitly did not open), and the in-memory defaults are
  fail-closed (an empty store authenticates nothing) for the single-process
  posture. Revisit at the first real IdP/partner connection, when a restart
  losing registrations becomes an operational risk instead of a lab one.
- **Serving binaries / composition entries for the five live protocol
  transports (20305, OCPP CSMS, OpenADR VTN, MQTT broker client, Modbus
  client) — rejected.** The transports are library seams exercised
  in-process by the gated suites (each crate's `tests/live_*_gated.rs`),
  and their construction already fail-closes on an unexplained plaintext
  listener (`transport.rs:96-118` OCPP; the 20305/MQTT recorded-reason
  pattern). A shipped listener binary needs a deployment context (the
  recorded lab-posture reason, D5/D6) and adds attack surface to a
  simulation-only release. Revisit when a named lab or pilot deployment
  exists.
- **Machine-token / API-key authentication of the ordinary API surface —
  rejected.** The authz decision path is strictly role-driven
  (`auth.rs:144-177`, `policies.evaluate(caller.role, ...)`); a machine
  principal carries parsed `Action` scopes, not a role. Wiring the
  credential tiers into the `IdentityResolver` requires either a scope-based
  evaluation path or a synthetic role on the frozen policy surface — a
  recorded-design change (record 07 §6.5 owns the mapping), not a mechanical
  feature. The dedicated `/machine-identity/validate` endpoint is the
  composed surface for now.
- **The OCPP CSMS relay composition in the control plane (an ISSUED command
  → `RelayRequest`) — rejected.** It depends on the command-core
  composition, which is recorded as a later-release step at the composition
  root (`main.rs:232-264`, FRV1-R1-004); the relay path is ISSUED-only by
  typing and is exercised at the seam by the pinned tests. Revisit with the
  command-core composition.
- **Kubernetes / Kafka migrations, market participation, V2G discharge,
  DNP3 master, IEC 104/61850 clients, OCPI roaming — unchanged.** All carry
  recorded §5.6 rejections in the source document; the FIM program changed
  none of their preconditions (D1 still OPEN; still no counterparty; the
  dispatch shape still fits JetStream).
- **Operator-approved control qualification — not a feature.** It is the
  P10 human gate (§6 item 8 of the source document); no code in this pass
  or the FIM program enables it. The release-notes draft's NOT-QUALIFIED
  verdict stands.

## 6. Wiring status of every new FIM seam (static cross-check result)

| Seam | Status | Evidence |
|---|---|---|
| fim_compose config gates (6 sections) | WIRED | `main.rs:64-70,156-212,328-347`; pinned by `tests/fim_composition.rs` |
| OIDC resolver composition | WIRED (config-gated) | `main.rs:1107-1141`; lazy JWKS fetch recorded |
| Step-up service composition | WIRED (config-gated) | `main.rs:1190-1235` |
| SCIM HTTP surface + bearer guard | WIRED (config-gated) | `routes.rs:143-152,188-210` |
| SCIM deprovisioning resolver wrap | WIRED-BROKEN (split store) | FIM2-R1-004; `main.rs:1147` vs `main.rs:1171-1176` |
| Machine-identity service + routes | WIRED (config-gated) | `routes.rs:157-160`; FIM2-R1-002 (revoke) and FIM2-R1-005 (grants) open |
| API-key service + routes | WIRED (config-gated) | `routes.rs:165-168`; FIM2-R1-001 (compile), FIM2-R1-005, FIM2-R1-006 open |
| Webhook HTTP transport | WIRED (config-gated; unwired default intact) | `main.rs:1071-1093`; `UnwiredOutboundHttpPost` default pinned |
| OTA rollout coordinator | COMPOSED, UNWIRED-RECORDED | `state.rs:288-291`; driver surface proposed as N-1 |
| OTA bundle producer / signer / verifier / slots | WRITTEN (library; no key material; default verifier unchanged) | `ota_signing.rs` header; `ota_bundle_produce.rs:1037` |
| OCPP CSMS WebSocket transport | WRITTEN (library; no composition binds it) | `transport.rs` header; FIM2-R1-003 open |
| 20305 / OpenADR / Modbus / MQTT live transports | WRITTEN (library; opt-in by construction) | each module header; edge default `UnwiredUplinkTransport` recorded (`edge_to_cloud.rs:17-19`) |
| CIM/CGMES import + spelling remediation | WRITTEN (library) | `xml_profile.rs` header; record 44a addendum |
| Vendor weather/price ingestion | WRITTEN (config-driven; fail-closed without a token) | `apps/worker/src/vendor_ingestion.rs` header |
| Python OTEL instrumentation | WRITTEN (stdlib-only core; SDK optional) | `apps/worker/python/otel_instrumentation.py` header |
| V-8 race test / V-9 chain upgrade / V-10 expect removal | WRITTEN | `tests/source_auth_replay.rs:588+`; `audit.rs:26-61`; `source_auth.rs:74-89,162-190` |
| F-11/F-14 optimizer consumption | UNWIRED-RECORDED (F-11) / UNWIRED (F-14, statement missing) | `main.rs:1358-1380`; FIM2-R1-011; N-2 proposed |

## 7. What remains human-gated

Everything the FIM and this pass may not do, restated so the release gate is
one list:

1. **The first build and test run:** `cargo build --workspace`, then the
   workspace test gates — including the three P0 fixes above and every
   "fixed (written, awaiting human build/test)" tick in
   `docs/releases/implementation_ledger/`. `Cargo.lock` regeneration
   (promoting `reqwest` to a direct edge) happens at this gate.
2. **The gated re-runs (V-1/V-2):** the Postgres 18 re-provision (approval
   was the recorded blocker), the 21 integration targets, the worker gated
   tests, the E_STORE_UNAVAILABLE trio (V-6) and fix M (V-7).
3. **The finish step (V-4/V-5):** `cargo fmt --check`, `cargo clippy
   --workspace -- -D warnings`, `check_dependency_rule.py`, the warning
   triage — plus §4.3's fmt-drift decision, still OPEN for its owner.
4. **Every HUMAN LAB GATE the honesty headers name:** real-peer interop for
   each wire codec (WebSocket, MQTT 5 + Sparkplug B, CGMES profiles, Modbus
   devices, IEEE 2030.5/CSIP, OpenADR VENs, OCSP), including the Sparkplug
   .proto verification pass (FIM2-R1-007); a real IdP for the OIDC/SCIM/step-up
   path (P13-E-3 stays OPEN); live webhook receiver interop; live vendor
   feeds (P15-E-1/E-2 and the registered token); the OTA key ceremonies
   (P18.03) and a rendezvous service + voucher-issuance process (F-15); a
   running Collector and firing SLO alerts (F-09); a named PKI operator and
   the five record-41 §3 conditions (F-13 — code surface only, NOT claimed
   met); live DSO DOE feeds (F-14); real DSO profile interop (F-06).
5. **The decisions:** D1-D6 stay OPEN for their recorded owners; §4.2's
   cursor decision and §4.3's fmt decision; E-decisions (P13-E-3,
   P15-E-1/E-2/E-3, P17-E-1/E-2, P18-E-*) stay OPEN — agents never close
   them.
6. **The release mechanics (§6 of the source document):** the release-notes
   draft revision — including statements now stale relative to this tree
   ("signed update DESIGN ONLY": a written-unrun producer/verifier now
   exists while `NoUpdateVerifier` remains the shipped default; "no IdP
   integration": written-unrun OIDC/SCIM/step-up now exists while no IdP is
   deployed) — the disposition of every F-01..F-15 item, the P10 human gates
   (security review, operator-mode qualification NOT QUALIFIED, benchmark
   baseline), the artifact set (SBOM from the committed `Cargo.lock`,
   reproducible-build note, signing keys), and the simulation-only restatement.

---

*FIM final analysis pass — written 2026-11-21, static read-only, HEAD
`d2761e2` plus the uncommitted working tree. Every finding cites file:line
evidence; nothing is verified by running; the frozen invariants are
byte-identical per the git-status scoping check; nothing here is a
certification, a safety case, or permission for live grid control.*
