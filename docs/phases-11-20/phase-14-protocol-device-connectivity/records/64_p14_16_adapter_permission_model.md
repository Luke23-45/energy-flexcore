# P14.16 — Adapter permission model

| Field | Value |
|---|---|
| Subtask | P14.16 — per-adapter capability manifests wired into kernel profiles; the never-granted review (review only — the set stays byte-identical); the command-relay-only rule enforced by kernel profiles; the ADR-010 extension record stating adapters hold no dispatch authority |
| Wave / agent | Phase 14 wave 2, Agent B (P14.13–P14.16) |
| Status | WRITTEN-UNRUN: the kernel module and its tests are written, never compiled, never executed. No adapter was composed, no profile was loaded in a running kernel, no composition plan was produced by execution (all composition evidence in this record is code-level, not runtime). |
| Sources | the wave-1 common layer (`crates/flexcore-protocol-ports/src/manifest.rs`, `scope.rs` — records 01/02/21/23/24); the kernel capability-manifest discipline (`crates/flexcore-kernel/src/manifest.rs` P7.05, `profile.rs` P7.15); ADR-010; conventions §6.3–§6.4; phase-14 context §3 constraints 1–2 |
| Files touched | `crates/flexcore-kernel/src/adapter_permissions.rs` (NEW), `crates/flexcore-kernel/src/lib.rs` (additive module registration), `crates/flexcore-kernel/Cargo.toml` (one additive workspace-pinned dependency line: `flexcore-protocol-ports`), `docs/architecture/adr/ADR-010-EXT-01-protocol-adapter-authority.md` (NEW — the extension record; ADR-010.md itself byte-identical) |

## 1. What landed

| Piece | Where | Honest status |
|---|---|---|
| `AdapterBinding` — adapter id + the adapter's own `ProtocolAdapterManifest` held BY VALUE; the binding id must equal the manifest's adapter id (one adapter, one identity) | `adapter_permissions.rs` | Implemented; UNRUN |
| The command-relay-only check (`AdapterBinding::check_command_relay_only`): relay capability requires a relay-covering control scope; `ExternalEventIngest` requires the Control resource class; the authority is re-matched against the ONE-VARIANT `AuthorityDeclaration::SupervisoryRelayOnly` — a kernel tripwire that refuses any future dispatch-authority variant at the boundary | same | Implemented; UNRUN |
| `BoundAdapterRegistry` — validated, duplicate-free binding set keyed by adapter id (BTreeMap determinism); `command_relay_adapters()` lists only relay-declaring adapters — the composition root wires ONLY these, and only to the relay path | same | Implemented; UNRUN |
| `bind_profile_adapters(profile, registry, requested)` — the profile-side wiring: posture must be simulation-shadow (the v0.x ceiling, conventions §6.7); the ceiling is re-checked against the never-granted set (defence in depth); every requested adapter must be bound (fail-closed `UnknownAdapter`) | same | Implemented; UNRUN |
| The never-granted review as data: `never_granted_review_statement()` plus the disjointness test pinning that the adapter capability vocabulary and the §5.7 permission vocabulary never collide | same | Implemented; UNRUN |
| Tests: the byte-identical four-id quote, capability/permission disjointness, relay-scope and ingest-class refusals, identity-mismatch and duplicate refusals, profile wiring (unknown adapter, sorted binding), the parser's own never-granted ceiling refusal re-pinned at the boundary, deterministic relay listing | `#[cfg(test)] mod tests` | Written complete; UNRUN |
| The ADR-010 extension record | `docs/architecture/adr/ADR-010-EXT-01-protocol-adapter-authority.md` | Written; additive companion record — ADR-010.md UNTOUCHED (§4) |

## 2. Manifests wired into kernel profiles — and why they travel beside the profile

Profile DATA itself is unchanged: the P7.15 closed-key parser
(`profile.rs`) is not extended, and the plugin manifest schema
(`manifest.rs`) is not extended. An adapter manifest is NOT a plugin
manifest (`flexcore-plugin-manifest-v1`) and must never be parsed by the
plugin schema — the adapter tier declares its contract through
`flexcore-protocol-ports`, and the kernel consumes that tier as
composition data: `AdapterBinding`s are constructed at the composition
root (one per deployed adapter, from the adapter's own manifest) and
validated against the profile at the binding boundary
(`bind_profile_adapters`). The result (`BoundProfileAdapters`) is the
reviewed record the composition root consumes. This keeps the three
vocabularies separate and reviewed: plugin permissions (§5.7, unchanged),
adapter capabilities (`DeclaredCapability`, wave-1), and adapter
authority (`AuthorityDeclaration`, one variant).

Dependency note: `flexcore-kernel` gained ONE additive dependency line
(`flexcore-protocol-ports`, workspace pin). The kernel is a COMPOSITION
crate (`tools/check_dependency_rule.py`): core crates never depend on it,
so this edge composes the CORE protocol-ports layer without changing any
core crate's allowed-edge set. The checker needs no new entries (no new
crate, no new member); output quoted in the batch close-out (record 65).

## 3. The never-granted review (review ONLY — the set is byte-identical)

The set, quoted verbatim from `crates/flexcore-kernel/src/manifest.rs`
(lines 76–81, UNTOUCHED by this batch):

```rust
pub const NEVER_GRANTED_PERMISSIONS: [&str; 4] = [
    "dispatch.authorize",
    "dispatch.issue",
    "secret.read",
    "network.any",
];
```

Review verdict, per permission:

| Permission | Phase 14 review | Action |
|---|---|---|
| `dispatch.authorize` | No adapter surface needs or expresses it; authorization stays the platform's third gate. Adapter authority is the one-variant `AuthorityDeclaration` type, not a permission string. | NONE — set unchanged |
| `dispatch.issue` | Relayed commands are already APPROVED by the lifecycle; the adapter never constructs ISSUED. Issuance stays in the platform (the frozen table has no adapter-reachable path to it). | NONE — set unchanged |
| `secret.read` | Adapters carry protocol credentials as deployment configuration (the P7.19 secret-REFERENCE rule); the adapter tier requests no secrets. | NONE — set unchanged |
| `network.any` | Each adapter declares its own protocol scope and cadence in its manifest; there is no unconstrained-network capability to grant. | NONE — set unchanged |

Enforcement points re-checked this batch (all additive, none weakening):
the P7.05 manifest gate and the P7.15 ceiling gate refuse the set at
parse (pre-existing, untouched); the new binding boundary re-checks the
ceiling (`NeverGrantedInCeiling` — defence in depth for hand-constructed
`Profile` values, unreachable from parsed data); and the disjointness
test pins that no `DeclaredCapability` name can ever collide with a
never-granted permission id, so no capability name can smuggle a
permission through a ceiling.

**Byte-identity verification (honest method, this is a code review not an
execution):** `manifest.rs` was re-read before and after the batch; the
`NEVER_GRANTED_PERMISSIONS` block is character-for-character the
pre-batch text (it appears in this record's quote above, and in the test
`never_granted_set_is_exactly_the_frozen_four_and_disjoint_from_
capabilities`, which pins the same four ids). `git diff` on
`crates/flexcore-kernel/src/manifest.rs` shows no change (§6).

## 4. The ADR-010 extension — why a companion record, not an edit

ADR-010 is in **Proposed** status pending the §23 Stage 0 exit-gate
review (P0.29) and reserves authority changes for "a new ADR superseding
this one". This extension grants no authority and changes no ADR-010
decision, so superseding is neither warranted nor honest — and editing a
Proposed ADR mid-program would blur the Phase 0 review provenance. The
extension therefore lands as an ADDITIVE companion record,
`docs/architecture/adr/ADR-010-EXT-01-protocol-adapter-authority.md`,
which states: protocol adapters are supervisory transports; external
control objects enter the lifecycle at PROPOSED; relay is not authority;
the type surface cannot express dispatch authority; the never-granted set
is untouched; settings changes and DOEs ride the same rails. ADR-010.md
itself is byte-identical (§6).

## 5. Honest boundaries

No kernel ran; no profile was loaded; no composition plan was computed at
runtime; no adapter was registered in any running registry. The
never-granted review is a CODE REVIEW with the file re-read, not an
execution output. All module behavior statements describe written code
(written-unrun), never observed behavior (conventions §7.1).

## 6. Verification of the byte-identity claims

Verified by re-read and (for the record) by `git status`/`git diff`
inspection at batch end: `crates/flexcore-kernel/src/manifest.rs` and
`crates/flexcore-domain/src/command_state.rs` are NOT in the batch's
changed-file set. The batch's changed files are exactly: the two domain
modules + lib.rs registration, the kernel module + lib.rs registration +
Cargo.toml dependency line, the ADR extension record, and the phase-14
records 61–65. No ledger checkbox was ticked by this agent.

## 7. Handoff notes

1. **Wave-3 Agent A (P14.17–P14.21)**: the simulated adapters construct
   their manifests exactly like the real ones (`ProtocolAdapterManifest::
   new`) — the binding checks apply unchanged, so a simulated adapter
   cannot gain authority its real counterpart lacks. P14.20/P14.21
   consume the manifest cadence and `ConnectionState::name()` as recorded
   in wave-1 records 01/02.
2. **Phase 18 (device identity/OTA)**: the adapter binding surface is the
   natural place to bind device-identity state to adapter manifests; no
   identity field is added here.
3. **Human gates**: first build; the never-granted review above is
   confirmable by reading `manifest.rs` (it predates and postdates this
   batch unchanged).
