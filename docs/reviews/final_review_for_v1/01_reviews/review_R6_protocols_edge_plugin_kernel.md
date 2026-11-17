# Final Review R6 — Protocol Adapters, Edge Agent, Kernel, Plugin Host (Iteration 1)

**Reviewer:** Agent B (wave 3)
**Date:** 2026-11-17
**Status:** REVIEWED — static read only (no build/test executed)
**Finding totals:** P0: 0 · P1: 3 · P2: 4

## 1. Scope reviewed

Census (Rust source; LOC excludes nothing — inline `#[cfg(test)]` modules are a large
share of several files, which is the recorded written-unrun pattern):

| Crate / app | Files | LOC | Depth |
|---|---|---|---|
| `crates/flexcore-protocol-ports` | 5 | 1,785 | Deep-read in full (scope, manifest, connection, observability) |
| `crates/adapters/protocol-20305` | 8 | 3,663 | Deep-read `lib.rs`, `server.rs`, `proposal.rs`; skim `model.rs`, `addressing.rs`, `scheduling.rs`; tests structural |
| `crates/adapters/protocol-openadr` | 8 | 2,580 | Deep-read `scope.rs`, `event_mapping.rs`, `backpressure.rs`, `credentials`-adjacent model validation; skim `vtn.rs`, `endpoints.rs` |
| `crates/adapters/protocol-modbus` | 9 | 2,231 | Deep-read `backpressure.rs`, `units_map.rs` header, `registry.rs` header; skim `model.rs`, `polling.rs`, `transport.rs` |
| `crates/adapters/protocol-dnp3` | 8 | 1,737 | Deep-read `master.rs`, `scope.rs`, `outstation.rs` (non-test), `model.rs::validate`; skim `transport.rs`, `secure_transport.rs`, `backpressure.rs` |
| `crates/adapters/protocol-mqtt` | 11 | 2,980 | Deep-read `credentials.rs`; skim `scope.rs`, `backpressure.rs`, `birth_death.rs`, `uplink.rs`, `topic.rs`, `transport.rs` |
| `crates/adapters/protocol-cim` | 8 | 2,688 | Structural skim (`lib.rs`, `unmapped.rs` header, `class_mapping.rs` header); scope constructor verified |
| `crates/adapters/protocol-entsoe` | 7 | 2,043 | Deep-read `token.rs`; skim `document.rs`, `xml_seam.rs`, `transport.rs`, `scope.rs` |
| `crates/adapters/protocol-ocpp` | 14 | 7,276 | Deep-read `scope.rs`, `charging.rs`, `csms.rs` (non-test); skim `session.rs`, `framing.rs`, `security.rs`, `provisioning.rs`, `model.rs`, `hosting.rs`, `conformance.rs` |
| `crates/edge-agent` (update/OTA subset) | — | ~5,300 of 55,098 | Deep-read `update_command_interlock.rs`, `update_bundle.rs`, `update_session.rs` (non-test); skim `attestation.rs`, `update_verification.rs` outline; headers of `identity_lifecycle.rs`, `zero_touch.rs`, `esim_lifecycle.rs`, `fleet_registry.rs` |
| `crates/flexcore-kernel` | 33 | 23,806 | Deep-read `adapter_permissions.rs` (full), `revocation.rs` (rollout + removal sections + header), `manifest.rs` never-granted/semver sections; rest structural |
| `apps/plugin-host` | 25 | 13,935 | Deep-read `bindings.rs` header, `broker.rs` never-granted/allowlist sections, `host.rs` WorldId, `main.rs` manifest/env surface, `Cargo.toml` feature/pin block |
| `docs/protocols/*.md` | 9 | — | All 9 read / outlined for doc-claims-vs-code |

Out of scope per assignment: control-plane routes (R2), console (R3). Grep-verified
wiring claims into those areas only.

## 2. Method

Static checks actually run (Read/Grep/ls only; no cargo/rustc/python-executed-repo-code):

1. **Census.** `find` + `wc -l` per in-scope crate; `ls` per source dir.
2. **Trait-vs-impl sweep.** Grep `ProtocolConnection for` / `impl ProtocolConnection`
   across `crates/` and `apps/` → **zero implementations found anywhere** (see
   FRV1-R6-001). Grep scope/manifest constructors per adapter (`ProtocolScopeDeclaration::new`,
   `ProtocolAdapterManifest::new`) → present in non-test `scope.rs` of openadr, dnp3,
   modbus, mqtt, cim, ocpp; **absent from non-test code of protocol-20305** (FRV1-R6-004).
3. **Module-vs-file census.** Compared every crate's declared `mod` items in
   `lib.rs`/`mod.rs` against actual `.rs` files (including `edge-agent/src/adapters/mod.rs`)
   → all match; no phantom or missing module.
4. **PROPOSED-entry sweep per adapter.** Grep `CommandState|Proposed|Issued|try_transition`
   across all 8 adapter sources. Results: 20305 `ExternalControlProposal` entry is the
   const `CommandState::Proposed` (proposal.rs:115); OpenADR `ExternalCommandProposal::
   entry_state()` is a const fn returning `Proposed` (event_mapping.rs:147); DNP3
   `Dnp3ControlProposal::entry_state()` likewise (outstation.rs:338); OCPP inbound has
   no lifecycle construction at all; the only `CommandState::Issued` construction-adjacent
   code in any adapter is OCPP `charging.rs:107` which **refuses** every non-ISSUED state
   (relay gate). No adapter constructs APPROVED/ISSUED/FEASIBLE from external input. **Clean.**
5. **Never-granted permission sweep.** Grep the four ids across `crates/` + `apps/`:
   byte-identical set in `flexcore-kernel/src/manifest.rs:76`, mirrored verbatim in
   `apps/plugin-host/src/broker.rs:92`, refused by name in
   `flexcore-authz/src/action.rs:375-378` (parse returns `None`), re-checked at the
   adapter-binding boundary (`flexcore-kernel/src/adapter_permissions.rs:329-337`) and in
   the profile-ceiling parse. No adapter, plugin, or host surface grants any of the four. **Clean.**
6. **Revocation/rollout invariant read.** `StagedRollout` (revocation.rs:1221-1407): no
   cohort, percentage, or bulk field exists on the type (grep confirms the only
   "percentage" occurrences are the prohibition statements); `plan` forces
   `promoted_count == 0` (first stage is one profile); `promote` succeeds only for the
   exact next un-promoted profile; completed rollouts refuse further promotion. **Invariant
   holds by construction. Clean.**
7. **OCPP relay-gate + stored-not-applied check.** `into_relay_request` refuses all
   non-ISSUED states with the state named (charging.rs:107-111); `record_accepted_status`
   returns the STORED-NOT-APPLIED statement verbatim (charging.rs:176-183); application
   evidence comes only from `parse_composite_schedule_response` + the domain
   reconciliation; no adapter-side retry path exists. **Clean.**
8. **ENTSO-E token gate check.** `token.rs`: closed two-state gate, `Unsigned` default,
   scheme-prefix validation with the three P7.05 schemes, inline material refused and
   never echoed (tests pin both). No real token handling ships. **Clean.**
9. **DNP3 role check.** `MasterRoleVerdict` is a one-variant DEFER enum;
   `assert_role_available` refuses unconditionally (master.rs:98-100); the scope declares
   SERVER only and the CLIENT role is refused at the scope gate (scope.rs test pins it). **Clean.**
10. **Budget-vocabulary application sweep.** `MessageRateBudget`/`BoundedQueuePolicy`/
    `FleetConnectionRegistry` consumed by modbus, mqtt, dnp3, openadr `backpressure.rs`
    modules and by `edge-agent/src/adapters/edge_to_cloud.rs:458`; record 83 records
    explicit no-module decisions for 20305 (ServerLimits IS the cap) and CIM (batch
    parse). OCPP and ENTSO-E have no row and no later recorded decision → FRV1-R6-002.
11. **Doc-claims-vs-code table.** All 9 `docs/protocols/*.md` checked against the crates:
    20305/openadr/modbus/dnp3/mqtt/cim/openems guides match implemented surfaces (seams
    correctly labelled UNRUN/UNWIRED, no conformance claims). Two mismatches/gaps:
    the connection-state claims (FRV1-R6-001) and the missing OCPP/ENTSO-E guides
    (FRV1-R6-003). The 104/61850 design-status docs correctly describe crates that
    do not exist (verified: no such crate directory).
12. **WIT-vs-host check.** `wit/` holds exactly the three packages
    (`flexcore:grid@1.0.0`, `flexcore:optimizer@1.0.0`, `flexcore:telemetry@1.0.0`) with
    worlds `grid-model-provider-plugin`, `optimizer-plugin`, `telemetry-source-plugin`;
    `WorldId` (host.rs:55-95) names and package-ids match one-to-one; no `wasi:` imports
    in any world, matching the host's no-WASI decision (plugin-host Cargo.toml feature
    block); wasmtime pin `=50.0.1` is crate-local with an explicit first-human-build
    promotion step. **Clean.**
13. **Manifest-schema-vs-kernel check.** The plugin-host binary takes plugin identity,
    permissions, and resource bounds as required env inputs and re-validates every id
    (main.rs:117-243) against the kernel vocabulary; `parse_manifest` (kernel manifest.rs:729)
    refuses the never-granted set at parse (manifest.rs:1145). No schema field drift found
    between the two surfaces. **Clean.**
14. **unwrap/expect/panic sweep** on adapter non-test paths → no `unwrap()` on
    parse/I-O paths in shipped (non-test) adapter code; the three `expect`s found in
    non-test adapter code are all construction-validated invariants (dnp3 outstation.rs:239
    reactive-convention, validated at `DeclaredScaling::validate`; edge-agent mutex
    poisons). **Clean.**
15. **Frozen-state-name citation check.** Edge-agent `FROZEN_COMMAND_STATE_NAMES`
    (command_path.rs:849-864) lists exactly the 14 states of conventions §4; the
    terminal five (interlock) match the frozen terminal set. **Clean.**

## 3. Checks performed, all consistent

Numbered cross-boundary checks that PASSED (diligence evidence; no findings attached):

1. `mod`/`pub use` declarations vs actual files, all 12 in-scope crates and
   `edge-agent/src/adapters/mod.rs` — no mismatch.
2. `ProtocolScopeDeclaration::new` field order/types at every adapter call site vs the
   8-argument definition (scope.rs:274) — all consistent; authority statements at all
   sites name both PROPOSED entry and not-a-dispatch-authority.
3. `ProtocolAdapterManifest::new` call sites vs the 7-argument definition
   (manifest.rs:175) — all consistent; `authority` is set only by the constructor to
   the one-variant value.
4. Kernel `AdapterBinding::check_command_relay_only` vs the manifests the adapters
   actually declare: OCPP declares `CommandRelay` **with** `ControlScope::CommandRelay`
   and the `Control` resource class (ocpp scope.rs:102-160) — passes the kernel check;
   all other adapters declare MonitorOnly and do not declare CommandRelay — also pass.
5. OCPP `CsmsServer::new` manifest-mismatch guard ids (`ADAPTER_ID`/`PROTOCOL_ID`) vs
   the `csms_manifest`/`csms_scope` constructor ids — identical spellings.
6. 20305 `AggregatorServer::new` manifest-mismatch guard vs `crate::ADAPTER_ID` /
   `PROTOCOL_ID` constants — identical spellings.
7. `ExtendedUpdateGate::verify_inner` refusal order (9 steps) vs the recorded order in
   the type doc and vs `AntiRollbackCounter::accept` semantics (strictly-ahead re-check
   under the lock, durable write before the in-memory move) — consistent.
8. `UpdateCommandInterlock` two-step composition contract (`authorize_update_attempt`
   always returns `in_flight_count == 0` on `Ok`, fed verbatim to
   `UpdateSessionStateMachine::begin_download`) — matches the wave-1 session rule's
   consumed signature.
9. Kernel `bind_profile_adapters` posture check vs `ControlAuthorityPosture::
   SimulationShadow` — the only posture the adapter-binding path accepts; no bypass.
10. `FleetConnectionRegistry.admit` fail-closed semantics (undeclared adapter id →
    refusal, zero-maximum → BudgetNotDeclared) — consistent at all three consumption
    sites (ports tests, openadr backpressure, modbus record-83 note).
11. Broker `ALLOWED_PERMISSIONS` vocabulary vs kernel §5.7 permission ids — the broker
    classes are a strict, parseable subset; `dispatch.validate.request` is target-locked
    to `PROTECTED_VALIDATOR_TARGET`.
12. `edge_to_cloud.rs` consumes `flexcore_protocol_ports::observability::MessageRateBudget`
    (the record-82 edge seam) — dependency line present in `crates/edge-agent/Cargo.toml:76`.
13. Anti-rollback replay: corrupt-tail truncate-and-count, last-seen = MAX over good
    records, wrong magic/version = hard open error — consistent with the recorded
    fail-closed statement.
14. `SecurityProfile` vocabulary (OCPP) vs `CsmsSecurityRequirements::onboarding_profile
    () == 3` structural posture — consistent; no Profile 1/2 downgrade field exists.
15. Never-granted set byte-identity across kernel manifest, plugin-host broker mirror,
    authz parse refusals, and adapter-permission ceiling re-check — four independent
    sites, identical strings.

## 4. Findings

### FRV1-R6-001 [P1] [phase-14] `ProtocolConnection` port has zero implementations, and five adapter guides claim connection states ride it
- Status: OPEN
- Location: crates/flexcore-protocol-ports/src/connection.rs:131 (trait definition);
  docs/protocols/ieee-2030.5-adapter.md:51, docs/protocols/openadr-adapter.md:43,
  docs/protocols/sunspec-modbus-adapter.md:46, docs/protocols/dnp3-adapter.md:43,
  docs/protocols/mqtt-sparkplug-adapter.md:45 (doc claims)
- Issue: Workspace-wide grep finds no `impl ProtocolConnection for …` anywhere
  (crates/ or apps/). Only OCPP consumes the connection vocabulary types
  (`crates/adapters/protocol-ocpp/src/csms.rs:40` imports `ConnectionReport`,
  `ConnectionState`, `ProtocolPortError` and maps errors/state), and it too does not
  implement the trait. None of the six phase-14 adapter crates references
  `connection::ConnectionState` at all. Yet five adapter guides state the connection
  states travel "via the common `ProtocolConnection` port" — a claim the code does not
  back. The ports crate's own lib.rs (line 4) says "Every Phase 14 protocol adapter …
  implements THIS layer". The trait is a defined-but-never-implemented seam, and unlike
  the other unwired seams (`AggregatorTransport`, `CsmsTransport`, `RdfProfileSource`,
  `CsmsTransport`) no decision record records the deferral or a refusing gate for it —
  the other seams ship a named `Unwired*`/`No*` default that refuses; this one ships
  nothing.
- Recommendation: Minimal, either direction is acceptable but it must be one of them:
  (a) record the deferral (one paragraph in a phase-14 decision record) and reword the
  five doc lines to "vocabulary defined by the common port; per-adapter connection
  reporting lands with the first adapter composition", or (b) add the honest
  state-returning impl per adapter (each adapter already has enough state to report
  honestly; OCPP's `CsmsTransport::state` shows the pattern). Do not leave the docs
  claiming a port the adapters do not exercise.

### FRV1-R6-002 [P1] [phase-17] OCPP (and ENTSO-E) have no recorded backpressure decision — the P14.20 shared budget vocabulary stops before them
- Status: OPEN
- Location: crates/adapters/protocol-ocpp/src/csms.rs:90-114 (`CsmsLimits` — a
  crate-local bounded queue, outside the shared refusal/metric vocabulary);
  crates/adapters/protocol-entsoe/src/transport.rs (no budget surface at all);
  docs/phases-11-20/phase-14-protocol-device-connectivity/records/83_p14_20_backpressure_rate_limiting.md §5 (the per-adapter table with no OCPP/ENTSO-E row)
- Issue: Record 83 binds the shared `MessageRateBudget`/`BoundedQueuePolicy`/
  `FleetConnectionRegistry` vocabulary onto modbus, mqtt, dnp3, openadr, and records
  explicit no-module decisions for 20305 (ServerLimits IS the cap) and CIM (batch
  parse). OCPP — the largest adapter (7.3k LOC, per-connection sessions, an outbound
  CALL queue, a connection registry with a cap) and ENTSO-E (poll-based ingest) landed
  later and have **no row and no later recorded decision** (grep of all phase-15/16/17
  records for backpressure/rate-budget returns nothing for these two adapters). OCPP's
  `CsmsLimits` enforces real caps with named refusals, so the safety posture is not
  absent — but its refusal reasons are crate-local (`CsmsError::OutboundQueueFull`,
  `ConnectionCapReached`) rather than the closed `RateRefusal` vocabulary, so the
  `flexcore_protocol_adapter_messages_refused_total` label semantics from record 83 do
  not cover the largest protocol surface, and the no-module rationale is unrecorded.
- Recommendation: Add one recorded decision (phase-17 close record §consistency or a
  small additive record) covering both adapters: either bind the shared vocabulary
  (`FleetConnectionRegistry.admit` at `CsmsServer::accept_connection`; a rate budget
  derived from the declared per-connection cadence) or record the no-module rationale
  the way record 83 does for 20305/CIM. For ENTSO-E the unwired-transport status makes
  a one-line "no recurring message rate exists until the transport seam is wired"
  decision sufficient.

### FRV1-R6-003 [P1] [phase-17] No operator-facing adapter guide for OCPP or ENTSO-E in docs/protocols/
- Status: OPEN
- Location: docs/protocols/ (directory census — 9 files; grep for "ocpp"/"entsoe"
  returns zero hits)
- Issue: Every other shipped protocol adapter has a `docs/protocols/*.md` guide in the
  same table format (20305, openadr, sunspec-modbus, dnp3, mqtt-sparkplug, cim, openems)
  plus two honest design-status docs for the not-built adapters (104, 61850). OCPP
  (P17.01–P17.05, the largest adapter) and ENTSO-E (P15.04) have phase decision records
  but no operator/protocol guide: a release consumer reading docs/protocols/ cannot
  learn that an OCPP 2.0.1/2.1 CSMS adapter and an ENTSO-E A44 ingest adapter exist,
  what their seams are (`CsmsTransport` UNRUN, token gate UNSIGNED), or their honest
  boundaries. This is exactly the release-readiness gap class the other guides were
  written to close.
- Recommendation: Add `docs/protocols/ocpp-csms-adapter.md` and
  `docs/protocols/entsoe-price-adapter.md` mirroring the existing guide format (scope /
  implementation status / control status / security / operations / honest boundary),
  sourced from records 01_p17_01…, 03_p17_03…, 04_p15_04… and the crate lib.rs honesty
  headers. No new claims — carry the existing UNVERIFIED/UNRUN labels verbatim.

### FRV1-R6-004 [P2] [phase-14] protocol-20305 is the only adapter without a non-test scope/manifest constructor
- Status: OPEN
- Location: crates/adapters/protocol-20305/src/server.rs:675-714 (manifest built only
  inside `#[cfg(test)] mod tests`); crates/adapters/protocol-20305/src/proposal.rs:242-260
  (same for the scope); crates/adapters/protocol-20305/src/lib.rs (no `scope.rs` module)
- Issue: openadr (`scope.rs:98/127`), dnp3 (`scope.rs:70/98`), modbus (`scope.rs:71/102`),
  mqtt (`scope.rs:93/128`), cim (`scope.rs:86/121`), and ocpp (`scope.rs:102/148`) each
  expose public, reviewed `*_scope`/`*_manifest` constructors in non-test code — the
  composition root's reviewed entry point. protocol-20305 does not: its scope and
  manifest exist only as test-helper functions inside `#[cfg(test)]` blocks, so a
  composition root must hand-assemble the 8-argument `ProtocolScopeDeclaration::new`
  and the capability list, with nothing but tests to keep the spellings from drifting
  from the reviewed record.
- Recommendation: Add `src/scope.rs` to protocol-20305 with
  `aggregator_scope(adapter_id)` / `aggregator_manifest(adapter_id)` copied verbatim
  from the test helpers, and have the test helpers delegate to them (one reviewed
  spelling, no drift).

### FRV1-R6-005 [P2] [phase-18] The entire OTA/update audit surface is classified `EdgeAuditKind::Startup`
- Status: OPEN
- Location: crates/edge-agent/src/ports.rs:361-393 (the closed `EdgeAuditKind`
  vocabulary — no update/OTA variant); crates/edge-agent/src/update_command_interlock.rs:906,
  crates/edge-agent/src/update_session.rs:851, crates/edge-agent/src/update_verification.rs:715,1293
- Issue: The P18 update/interlock/gate/ledger modules emit their audit events through
  `EdgeAuditEvent { kind: EdgeAuditKind::Startup, detail: "EDGE_UPDATE_*: …" }`. The
  dedicated event name rides the detail text (the recorded name-leads-detail pattern),
  but the structured `kind` field misclassifies every update-verification, interlock,
  drain, and re-check event as a startup event. The vocabulary gained additive
  identity kinds in P6.06 but nothing for the P18 surface. Any consumer filtering or
  counting audit events by `kind` sees the whole OTA lifecycle as "startup".
- Recommendation: One additive `EdgeAuditKind` variant (e.g. `UpdateLifecycle` →
  `"EDGE_UPDATE_LIFECYCLE"`) in ports.rs, adopted by the four emission sites. Additive
  enum change on a closed vocabulary is the recorded pattern (P6.06 did the same).

### FRV1-R6-006 [P2] [phase-14] MQTT `CredentialBindingSet` doc says BTreeSet; the implementation is a sorted Vec
- Status: OPEN
- Location: crates/adapters/protocol-mqtt/src/credentials.rs:127-131
- Issue: The doc comment reads "the set is ordered (BTreeSet over client id)" while the
  struct is `bindings: Vec<DeviceCredentialBinding>` sorted by client id at
  construction (`sort_by`, credentials.rs:148-150). Determinism is actually preserved,
  so this is comment drift only — but on a credential surface, drifted comments are
  the kind a security reviewer must re-derive by hand.
- Recommendation: Reword the comment to "a Vec sorted by client id at construction
  (deterministic presentation); uniqueness enforced against a BTreeSet at assembly."

### FRV1-R6-007 [P2] [phase-18] `AntiRollbackCounter::accept` durably appends a sequence before the strictly-ahead re-check under the lock
- Status: OPEN
- Location: crates/edge-agent/src/update_bundle.rs:741-785 (append at 756-768; lock
  re-check at 769-780)
- Issue: When two gates share one counter (the comment's own scenario), the second
  gate's `accept` writes its record to the append-only file first and only then
  discovers `sequence <= last_seen` and returns `SequenceNotAhead`. The refused
  sequence stays on disk forever. Safety is preserved — replay derives last-seen as
  the MAX over good records, so the persisted out-of-order value can never lower the
  counter — but the register grows with refused entries and the file contents diverge
  from the "accepted" set the name implies; `records_accepted` and the file record
  count can also disagree.
- Recommendation: Move the `sequence <= *last_seen` re-check before the append (take
  the `last_seen` mutex first, check, then append and update). One-line reorder, no
  semantics change for the single-gate path.

## 5. Release-readiness verdict for this area

This area is unusually strong for a written-unrun baseline: every frozen invariant the
mission flagged is enforced by types rather than convention (PROPOSED-only entry via
const entry states in 20305/OpenADR/DNP3 and zero inbound lifecycle construction
elsewhere; the OCPP SetChargingProfile relay refuses every non-ISSUED state and carries
the STORED-NOT-APPLIED statement; the ENTSO-E token gate ships UNSIGNED with inline
material structurally impossible; DNP3 master is a one-variant DEFER enum; the kernel
rollout is cohort/percentage-free with order-locked promotion; the never-granted set is
byte-identical across four independent enforcement sites), the OTA surface
(extended gate, anti-rollback counter, command interlock, drain procedure,
conformance re-check with override) is fail-closed at every seam including
counter-unavailable and device-state-unknown cases, and the plugin-host keeps
wasmtime behind a compile-gated, fail-closed `NoBindings` seam with the WIT worlds
matching the host's `WorldId` exactly. Before release, three items must be fixed: the
five adapter guides' `ProtocolConnection` claim (record the deferral or soften the
docs — as written, the docs describe a port no adapter exercises, FRV1-R6-001); a
recorded backpressure decision for OCPP and ENTSO-E, whose absence from record 83's
otherwise-complete per-adapter table is a silent coverage hole on the largest protocol
surface (FRV1-R6-002); and the two missing adapter guides for OCPP and ENTSO-E so
docs/protocols/ tells the same story as the crates (FRV1-R6-003). The four P2 items
(20305's test-only manifest constructor, the `EdgeAuditKind::Startup` misclassification
of the whole OTA audit stream, the MQTT comment drift, and the counter append-before-
check ordering) are polish and can follow, though FRV1-R6-005 and FRV1-R6-007 are
cheap enough to land with the P1 wave.
