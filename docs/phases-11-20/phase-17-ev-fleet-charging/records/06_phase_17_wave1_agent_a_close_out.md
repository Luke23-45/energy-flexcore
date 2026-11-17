# Phase 17 — Wave 1, Agent A close-out (P17.01–P17.05): CSMS core + composer

| Field | Value |
|---|---|
| Wave / agent | Phase 17 wave 1, Agent A |
| Subtasks | P17.01 (record 01), P17.02 (record 02, P17-E-2 registered OPEN), P17.03 (record 03), P17.04 (record 04), P17.05 (record 05) |
| Status | WRITTEN-UNRUN: all deliverables are design-baseline code + written-unrun test suites + records; nothing compiled, nothing executed, no charge point connected, no OCPP message exchanged with a real device, no conformance/certification/metrology/safety claim anywhere |
| Bans honoured | No installs; no builds; no tests run (execution = human gate); zero placeholder macros or placeholder markers in any file written (grep-verified); permitted verification tools only, outputs quoted verbatim below; no edits to `ledger_phase_17.md`, `plan.md`, `context.md`, foundations, plan docs, `.agent/*` — every ledger checkbox remains unticked by this agent |

## 1. Files created / modified

**NEW crate `crates/adapters/protocol-ocpp/` (package `flexcore-ocpp-adapter`, COMPOSITION classification):**

| File | Content |
|---|---|
| `Cargo.toml` | Deps: `flexcore-protocol-ports`, `flexcore-domain`, `serde`, `serde_json` (workspace pins). NO WebSocket library, NO TLS, NO HTTP framework — the transport is a designed UNRUN seam |
| `src/lib.rs` | Module map, `ADAPTER_ID = "ocpp-csms"`, `PROTOCOL_EDITION = "OCPP 2.0.1 (P17-E-1 OPEN; 2.1 additive)"`, honest-status header |
| `src/scope.rs` | The common `ProtocolScopeDeclaration` (SERVER role, COMMAND RELAY control scope, Telemetry/Control/Discovery/EventLog, 7-entry `not_modelled` incl. `no_ocpp_1_6` and `no_pki_ca_operations`, validated authority statement) + the one `ProtocolAdapterManifest` with `SupervisoryRelayOnly` |
| `src/model.rs` | The payload model types (BootNotification, StatusNotification, Heartbeat, TransactionEvent, the security block, the SetChargingProfile block) + `ModelError`, each with UNVERIFIED register pointers |
| `src/framing.rs` | The OCPP-J codec (CALL 2 / CALLRESULT 3 / CALLERROR 4, closed action vocabulary, closed error codes, whole-refusal decode) + `CallTracker` (one-outstanding-CALL per direction) |
| `src/session.rs` | Subprotocol negotiation (`ocpp2.0.1`/`ocpp2.1`; `ocpp1.6` refused with the P17-E-1 reason; no unspecified default) + the session state machine (`AwaitingSubprotocol → Negotiated → Provisioned → Ended`) |
| `src/csms.rs` | The server skeleton: deterministic `BTreeMap` registry, fail-closed construction, bounded outbound queues with tracker unwinding, honest `ConnectionReport`, `CsmsTransport` seam + `UnwiredCsmsTransport` (`CSMS_TRANSPORT_SEAM_NOT_WIRED`) |
| `src/security.rs` | P17.03: `OnboardingPolicy` (Profile 3 structural), `DeviceCertificateRecord` (labels only), `CertificateFlowLedger` (REQUESTED → SIGNED_AVAILABLE → INSTALLED / REJECTED; issuance is an EXTERNAL gate), `IdentityProofingLedger` (fail-closed registration), `SecurityEventLedger` (declared classification, Unclassified for unknowns, records never reactions) |
| `src/hosting.rs` | P17.02: `CsmsHostingShape` (edge-agent-hosted with STRUCTURAL store-and-forward vs control-plane-hosted with required recorded reason), `CsmsPlacement::depot_default` (the P17-E-2 DEFAULT implemented) |
| `src/provisioning.rs` | P17.04 mapping: `ProvisioningPolicy` (fail-closed DEFAULT allowlist; `allow_all` requires a recorded reason), boot → `BootRecord`, status via the DOMAIN mapping (unmapped refused), link-loss touches ONLY the link state, TransactionEvent with never-invented transactions |
| `src/charging.rs` | P17.05 adapter side: `into_relay_request` (lifecycle-typed — ISSUED only), `record_accepted_status` (stored-not-applied statement), `parse_composite_schedule_response` (like-with-like units, shape refusals) |
| `tests/framing_codec.rs`, `tests/adapter_surface.rs` | WRITTEN-UNRUN integration suites (wire shapes, negative framing, tracker discipline, registry determinism, hosting shapes, proofing fail-closed, relay-issued-only, reconciliation) |

**NEW domain files (flexcore-domain, additive modules — no frozen content touched):**

| File | Content |
|---|---|
| `src/evse_registry.rs` | P17.04 registry extension: `EvseIdentity`, `BootRecord`, link-vs-operational state separation (`CONNECTION_LOSS_IS_NOT_UNAVAILABILITY`), `map_status_to_operational_state` (declared mapping, refuses unmapped), `EvseTransactionRecord`/`State`, `TimestampPair`, explicit provision/re-boot |
| `src/ev_charging_composer.rs` | P17.05 composer: the three compose entry points (external constraints @ evseId 0 + isGridCritical / TxDefault / TxProfile), recorded precedence statement, minChargingRate pairing with named to-zero decisions, W/A conversion with verbatim formulas, `ChargingProfilePlan` (no lifecycle state, structurally), `reconcile_composite_schedule` (Aligned / named Mismatch / UnitMismatch) |
| `tests/ev_charging_composer.rs` | WRITTEN-UNRUN composer suite (hand-computed values: 0.4 MW→400 kW, 30 kW→25 A @400 V 3-ph, 6.9 kW→30 A @230 V 1-ph, 50 kW→100 A @500 V DC; min-rate 6 A→7.2 W-kW zeroing; precedence ranks; reconciliation mismatches) |

**Shared files (additive single-entry edits, re-read before each edit; Agent B edited the same files concurrently — both sets of entries are present and independent):**

- `Cargo.toml`: member `"crates/adapters/protocol-ocpp"` with placement note + workspace dependency `flexcore-ocpp-adapter = { path = "crates/adapters/protocol-ocpp" }`.
- `tools/check_dependency_rule.py`: `"flexcore-ocpp-adapter"` added to `COMPOSITION_CRATES`; `"crates/adapters/protocol-ocpp"` added to `MEMBER_DIRS`.
- `crates/flexcore-domain/src/lib.rs`: additive `pub mod ev_charging_composer;` and `pub mod evse_registry;` entries with decision-record pointers.

**Records written:** `01_p17_01_ocpp_csms_adapter_architecture.md` (registers P17-E-1 OPEN, DEFAULT (a)), `02_p17_02_csms_hosting_boundary.md` (registers P17-E-2 OPEN, DEFAULT (a)), `03_p17_03_security_profile_3_onboarding.md`, `04_p17_04_boot_status_transaction_mapping.md`, `05_p17_05_smart_charging_composer.md`, this close-out (06).

## 2. Verbatim permitted-tool outputs (final state, all exit 0)

```text
$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 29 workspace members.
EXIT=0

$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0
```

(The `ù` in the checker line is the console rendering of the source's
em-dash; the check reports 29 workspace members — 28 pre-existing + this
batch's additive `crates/adapters/protocol-ocpp`.)

## 3. Crate/checker diffs (this agent's additive hunks only)

- `Cargo.toml`: `+ "crates/adapters/protocol-ocpp"` (member, with the
  placement note naming the designed UNRUN WebSocket seam and decision
  record 01) and `+ flexcore-ocpp-adapter = { path = "crates/adapters/protocol-ocpp" }`
  (workspace dependency table, single version source).
- `tools/check_dependency_rule.py`: `+ "flexcore-ocpp-adapter"` in
  `COMPOSITION_CRATES` (composition side, no transport dependency, nothing
  depends ON it except future composition roots) and
  `+ "crates/adapters/protocol-ocpp"` in `MEMBER_DIRS`.
- `crates/flexcore-domain/src/lib.rs`: `+ pub mod ev_charging_composer;`
  and `+ pub mod evse_registry;` (additive module entries with
  decision-record pointers; no existing module line changed).

## 4. Deviations and honesty notes

1. **No deviations from the brief's bans or scope.** The transport is a
   designed seam exactly as mandated (no WebSocket dependency added).
2. **WRITTEN-UNRUN defects are possible** (the whole 0–10 base has never
   compiled): the code was hand-reviewed twice after writing (a malformed
   match arm in `CallTracker::track_reply`, a duplicate enum variant and
   an unusable mutable borrow in the transaction-update path, a missing
   `ModelError` enum, a bad `Copy` derive on `EvseLinkState`, and a
   key-order-dependent test assertion were found and fixed) — but the
   first human build remains the gate and may surface more.
3. **Shared-file concurrency:** Agent B added its own member/checker/module
   entries concurrently; both edits were applied additively and the final
   checker run reports 29 members green — no conflict.
4. **UNVERIFIED everywhere it belongs:** every OCPP field name, message
   code, action spelling, subprotocol string, error code, purpose
   spelling, precedence order, security-event name, and wire formula in
   the new code carries an UNVERIFIED label naming the implementation-time
   check against the OCA text (records 01/03/04/05 §registers).
5. **Lifecycle integrity:** the frozen 14-state/27-edge table and the
   never-granted set are untouched (no byte changed in
   `crates/flexcore-domain/src/command_state.rs` or
   `crates/flexcore-kernel/src/manifest.rs`); charging-profile payloads
   are buildable only from `CommandState::Issued` via the typed relay; no
   path exists from external input to a lifecycle state.

## 5. Handoffs

**To Agent B (wave 1):**
- `flexcore_domain::evse_registry` is the vocabulary for your
  constraint-record surfaces (P17.06) — `NotifyChargingLimit`/
  `ClearedChargingLimit` records should key off `EvseIdentity` rather
  than raw strings, and the `CONNECTION_LOSS_IS_NOT_UNAVAILABILITY`
  statement is the shared offline rule.
- P17.11's depot site-cap composition feeds `compose_external_constraints_
  profile` (record 05 §2); the composer consumes envelopes in canonical MW
  and never decides the cap itself.

**To wave 2 (Agent A — P17.12–P17.15):**
- P17.12 consumes the certificate-flow states (record 03 §3) — the
  issuance gate is EXTERNAL; keep it that way.
- P17.14's conformance suite should import `tests/framing_codec.rs`'s
  negative cases as its malformed-frame/subprotocol-mismatch subset.
- P17.15's simulated charge points speak against `CsmsServer` +
  `UnwiredCsmsTransport`'s seam contract (record 01 §7) — wire the
  simulator THROUGH the seam trait, not around it.
- The recorded `ocpp1.6` refusal reason is P17-E-1's; wave 2 must not
  widen the subprotocol set.

**To the human gates:** first workspace build; test execution; transport
seam wiring decision; P17-E-1 (via D5, human-closed) and P17-E-2 (first
depot site + connectivity profile); all OCA-text verification items in the
records' UNVERIFIED registers.
