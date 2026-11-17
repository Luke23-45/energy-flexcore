# Phase 14 — Wave 2, Agent B close-out (P14.13–P14.16)

| Field | Value |
|---|---|
| Batch | Phase 14 wave 2, Agent B: P14.13 (1547 settings-profile repository), P14.14 (settings-management workflow), P14.15 (DOE consumption interface), P14.16 (adapter permission model + ADR-010 extension) |
| Status | WRITTEN-UNRUN throughout. Nothing was built (no `cargo build`/`check`), no test was run (no `cargo test`), nothing was installed. The three permitted verification tools were the ONLY verification (outputs quoted verbatim in §3). No settings change was issued, no DOE was ingested, no device was touched, no kernel ran. |
| Honesty boundary | No capability claim about any device, envelope, or operator surface. Every secondary-source standard detail carries an UNVERIFIED label (records 61 §4, 62 §5, 63 §4). The frozen 14-state/27-edge set and the never-granted permission set are byte-identical (§4). |
| Decision records | `61_p14_13_settings_profile_repository.md`, `62_p14_14_settings_management_workflow.md`, `63_p14_15_doe_consumption_interface.md`, `64_p14_16_adapter_permission_model.md`, this close-out |

## 1. Files created / modified (complete list)

| File | Created/Modified | Notes |
|---|---|---|
| `crates/flexcore-domain/src/ieee1547_settings.rs` | CREATED | P14.13: the function catalog (closed 9-entry vocabulary), ride-through categories, declared curve-quantity axes, validating `FunctionSetting`, jurisdiction default tables as documents, the insert-only `SettingsDocumentStore`, per-device `DeviceSettingsState` with declared function support — document-store posture, zero transcribed values, tests embedded |
| `crates/flexcore-domain/src/settings_management.rs` | CREATED | P14.14: `SettingsAuthority` (declared ranks), `SettingsChangeCandidate`, `SettingsChangeProposal` (CONST PROPOSED entry, frozen-table refusals), `resolve_competing_settings` (rank → declared primacy → creation → command id), `CertifiedBaseline` (utility-owned gate), tests for primacy/override/lifecycle |
| `crates/flexcore-domain/src/doe_envelope.rs` | CREATED | P14.15: `ConnectionPointId`/`EnvelopeScope`, `ImportExportLimits`/`StaticConnectionLimit`, `DynamicOperatingEnvelope` + `UpdateCadence`, `evaluate_envelope` → `EffectiveEnvelope` (lapse reasons NotYetInEffect/Expired/Stale), `effective_envelope_bounds_mw` (constraint data only), breach/curtailment records, simulator-handoff note in the module header, lapse/bounds/telemetry tests |
| `crates/flexcore-domain/src/lib.rs` | MODIFIED (additive) | Three module declarations + P14-citing comment blocks; the existing module list order preserved (alphabetical), nothing removed |
| `crates/flexcore-kernel/src/adapter_permissions.rs` | CREATED | P14.16: `AdapterBinding` (identity match, command-relay-only check with the one-variant authority tripwire), `BoundAdapterRegistry`, `bind_profile_adapters` (posture + ceiling re-check + fail-closed unknown-adapter refusal), `never_granted_review_statement()`, the disjointness test, profile-wiring tests |
| `crates/flexcore-kernel/src/lib.rs` | MODIFIED (additive) | One module declaration + P14-citing comment block; nothing removed |
| `crates/flexcore-kernel/Cargo.toml` | MODIFIED (additive) | ONE dependency line: `flexcore-protocol-ports = { workspace = true }` with the explanatory comment (kernel is composition-side; no core-crate edge changes) |
| `docs/architecture/adr/ADR-010-EXT-01-protocol-adapter-authority.md` | CREATED | The ADR-010 extension record (additive companion — ADR-010.md untouched; rationale in record 64 §4) |
| `docs/phases-11-20/phase-14-protocol-device-connectivity/records/61–65_*.md` | CREATED | The four decision records + this close-out |

Untouched, verified: `crates/flexcore-kernel/src/manifest.rs` (the
never-granted set byte-identical; not in the batch's changed-file set),
`crates/flexcore-domain/src/command_state.rs` (the frozen 14-state/27-edge
lifecycle byte-identical; not in the changed-file set),
`crates/flexcore-simulator/**` (wave-3 Agent A's territory — the P14.15
simulator handoff is a recorded seam, record 63 §5), `docs/foundations/*`,
`docs/plan/*`, `.agent/*`, `docs/phases-11-20/phase-14-protocol-device-
connectivity/{context,plan,ledger_phase_14}.md` (no checkbox ticked by
this agent — the coordinator applies tick flips), the wave-1 and wave-2
Agent-A adapter crates, root `Cargo.toml` (no new members this batch), and
`tools/check_dependency_rule.py` (no new entries needed — no new crate, no
new edge). The never-granted set, quoted verbatim in record 64 §3.

## 2. Design decisions of record (none silent)

1. **Domain placement, no new crate.** All three P14.13–P14.15 modules
   live in `flexcore-domain` (settings and DOE vocabulary is domain data;
   the subtask's placement note). The crate's dependency line is
   untouched — `flexcore-contracts` only, no serde, no protocol-crate
   types; adapters map their native spellings IN at their own boundary.
2. **The settings repository ships no numbers.** Jurisdiction tables
   (MA TSRG [S-19] first) are structure + citation + entered data; an
   empty registered table is the honest starting state, and transcribing
   secondary-source values would be the hallucinated-fact risk AGENTS.md
   §3 rule 1 forbids. The transcription is a named human gate (record 61
   §6).
3. **The utility-override hook changes precedence, never the gates** —
   pinned by test: a utility override enters at PROPOSED and is refused
   by the frozen table for PROPOSED → ISSUED/APPROVED/FEASIBLE exactly
   like an aggregator change (record 62 §2).
4. **Profile DATA unchanged in P14.16.** Adapter manifests travel BESIDE
   the kernel profile (bindings validated at the composition boundary),
   never inside it: an adapter manifest is not a
   `flexcore-plugin-manifest-v1` document, and extending the P7.15
   closed-key parser would touch a frozen surface for no enforcement gain.
   The kernel gained one additive workspace-pinned dependency
   (`flexcore-protocol-ports`) — legal because the kernel is a
   COMPOSITION crate (core crates never depend on it), confirmed by the
   checker.
5. **ADR-010 extension as a companion record** (`ADR-010-EXT-01-…`), not
   an edit: ADR-010 is Proposed/pending review, reserves authority changes
   for a superseding ADR, and this extension grants no authority (record
   64 §4). ADR-010.md is byte-identical.

## 3. Permitted-tool outputs (verbatim)

Run from the repo root after the last code and record edit; all three
exited 0. The final run is quoted (an earlier mid-batch run printed
`...holds for 25 workspace members.` — see the note below).

```text
$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 27 workspace members.
EXIT=0
```

(Encoding note, recorded honestly, same artifact as the wave-1 close-out:
the script's literal text is `OK — dependency rule holds for 27 workspace
members.`; the em dash renders as `ù` in this Windows console's code
page. **Count provenance:** this batch added no crate and no checker
entry — the pre-batch checker counted 25 members (the wave-1 close-out's
number). During this batch, wave-2 Agent A landed ITS member/checker
entries in the shared files (its ownership: protocol-104/61850/mqtt/cim
crate additions), which raised the verified count to 27 at this batch's
final run. Both counts are green; the shared-file edits are A's, not
this batch's.)

```text
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

## 4. Byte-identity verification (the two frozen sets)

1. **The never-granted permission set** (`crates/flexcore-kernel/src/
   manifest.rs`): re-read before and after the batch; the constant is
   quoted verbatim in record 64 §3; `git status` shows `manifest.rs` is
   NOT in the batch's modified-file set. The kernel-side re-check and the
   disjointness test are additive code in the NEW module only.
2. **The frozen 14-state / 27-edge command lifecycle** (`crates/
   flexcore-domain/src/command_state.rs`): UNTOUCHED — not in the
   modified-file set. `settings_management.rs` consumes
   `command_state::try_transition` and adds no edge; its tests reuse the
   frozen table as the refusal authority.

## 5. Deviations

1. **None against the hard bans.** Nothing installed, nothing built, no
   test executed, no placeholders (grep across all new/modified files:
   zero TODO/FIXME/todo!/unimplemented!), no new external dependency (the
   one new kernel line is the existing `flexcore-protocol-ports`
   workspace pin), no ledger/plan/context/foundations/.agent edit, no
   checkbox tick.
2. **Recorded judgment calls** (recorded in §2, none silent): domain
   placement instead of a new crate; profile-data-unchanged wiring; the
   ADR companion-record form; the settings store shipping no transcribed
   values.
3. **Honest limitation carried forward**: every new module is
   WRITTEN-UNRUN; the first human build may surface recorded defects
   (e.g. the test fixtures use hand-built values that must compile
   against the validating constructors). Failures at that gate are fixed
   as code edits with a record update, never silently.

## 6. Handoff notes

1. **Wave-2 Agent A (P14.09–P14.12)**: no shared file touched by this
   batch (no root-Cargo or checker edit was needed) — your additive
   entries follow the wave-1 pattern unchanged. The P14.16 kernel
   dependency on `flexcore-protocol-ports` is composition-side and does
   not affect any core-crate edge you might add.
2. **Wave-3 Agent A (P14.17–P14.21)**: (a) the DOE profile seam is
   recorded in `doe_envelope.rs`'s module header and record 63 §5 —
   consume `evaluate_envelope` + `effective_envelope_bounds_mw`, supply
   synthetic TEST-ONLY envelope fixtures, never redefine the semantics;
   (b) simulated adapters construct the same `ProtocolAdapterManifest`
   and pass the same `adapter_permissions` checks — a simulated adapter
   cannot gain authority its real counterpart lacks; (c) P14.20/P14.21
   consume the manifest cadence and `ConnectionState::name()` per wave-1
   records 01/02.
3. **Phase 15/16 consumers**: DOE bounds via
   `flexcore_domain::doe_envelope::effective_envelope_bounds_mw` only;
   settings state via the P14.13 repository types — no parallel
   vocabulary.
4. **Human gates**: (a) the first workspace build (every module and test
   here is WRITTEN-UNRUN); (b) the standard-text verification passes
   (records 61 §4, 62 §5, 63 §4); (c) the MA TSRG transcription pass
   (record 61 §6); (d) confirmation of the never-granted review (record
   64 §3 — the file predates and postdates the batch unchanged).
5. **Ledger**: no checkbox was ticked by this agent; the coordinator
   applies the tick flips with the exact-match procedure using records
   61–64 as citations.
