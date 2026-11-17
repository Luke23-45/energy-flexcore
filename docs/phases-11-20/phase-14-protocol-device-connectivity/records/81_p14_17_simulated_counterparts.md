# P14.17 — Simulator integration: the simulated protocol counterparts

| Field | Value |
|---|---|
| Subtask | P14.17 (wave 3, Agent A) — simulator integration: every adapter testable against simulated peers with no hardware |
| Deliverable class | Simulated-peer modules + WRITTEN-UNRUN test suites (the P6.08 `simulated` fixture class, carried to the protocol tier) |
| Honesty boundary | Code, never execution. No protocol connected, no device reached, no message exchanged, no server bound, no broker contacted, no TLS handshake, no profile file read. Every composing test is WRITTEN-UNRUN; execution is a human gate. |
| Sources cited | [S-1][S-3][S-5] (2030.5/CSIP — via record 02, not re-transcribed), [S-10] (OpenADR 3.0 — record 21), [S-16] (SunSpec — record 23), [S-14][S-15] (DNP3 — record 24), [S-39][S-40] (MQTT/Sparkplug — record 43), [S-17][S-18] (CIM — record 44), [S-158][S-159] (DOE — record 63) |

## 1. The dependency-rule placement decision (the arbiter spoke first)

The subtask asked the question: may `flexcore-simulator` depend on the
adapter crates, or must the simulated counterparts live elsewhere?
**Answer: yes, it may.** `tools/check_dependency_rule.py` classifies
`flexcore-simulator` as a COMPOSITION crate (`COMPOSITION_CRATES`, with
the P2.14 recorded placement rationale in its own manifest header), and
the checker's edge policy checks ONLY core crates ("if name not in
CORE_CRATES: continue — adapters/apps may use infrastructure"; core →
composition is the forbidden direction). The six simulated counterparts
therefore live in `crates/flexcore-simulator/src/adapters/` — the
subtask's preferred placement — with these composition-side edges:

- `flexcore-simulator` → `flexcore-protocol-ports` (CORE; adapters
  depend on ports — the normal direction);
- `flexcore-simulator` → the six adapter crates (COMPOSITION →
  COMPOSITION; the checker skips composition crates entirely);
- `flexcore-simulator` → `flexcore-kernel` (COMPOSITION → COMPOSITION;
  the kernel's own placement note records the same classification logic
  for `flexcore-research`). This edge exists so the simulated adapters
  pass the SAME `flexcore_kernel::adapter_permissions` checks their real
  counterparts pass — the wave-2 handoff, record 63 §6.

No new crate, no checker entry, no new member. One additive line in the
root manifest's `[workspace.dependencies]`
(`flexcore-kernel = { path = "crates/flexcore-kernel" }` — the kernel
was a member but absent from the single-version table) and the matching
dependency lines in `crates/flexcore-simulator/Cargo.toml`, each with
the recorded rationale. **No transport, stack, broker, parser, or
runtime dependency is added by any line**: every adapter crate is
model-only (its wire seams stay UNWIRED, records 02/21/23/24/43/44).

## 2. The six simulated counterparts (one per named seam)

| Simulated peer | Module | Seam it lands on (owning record) |
|---|---|---|
| 2030.5 device client + in-process server transport | `crates/flexcore-simulator/src/adapters/sim_20305.rs` | `server::{AggregatorTransport, ResourceRequestHandler}` (record 02 §5) |
| OpenADR VEN | `adapters/sim_openadr.rs` | `endpoints::VtnEndpointDesign` + `vtn::{VtnRegistry, DeliveryPlanner}` (record 21 §3) |
| SunSpec Modbus device | `adapters/sim_modbus.rs` | `transport::ModbusTransport` (record 23 §4) |
| DNP3 master | `adapters/sim_dnp3.rs` | `transport::Dnp3Transport` (record 24 §4) |
| MQTT broker sink | `adapters/sim_mqtt.rs` | `transport::MqttUplinkTransport` + `birth_death::BirthDeathTracker` (record 43 §2) |
| CIM profile fixture source | `adapters/sim_cim.rs` | `rdf_profile::RdfProfileSource` (record 44 §2) |

Shared discipline (the `adapters/mod.rs` header): deterministic by
construction (no wall-clock read, no randomness — the injected-instant
rule); every identity string carries a `sim-…` prefix so a simulated
record is identifiable from its identity alone, forever; no value these
fixtures produce is evidence about real device or peer behaviour.

Two SEAM-LEVEL additions were required for honest refusals (both
additive enum variants, no existing variant touched, verified against
every existing match):

1. `protocol-modbus::transport::TransportError::WindowOutOfRange` — the
   simulated device serves a DECLARED register block; an out-of-window
   read is a named refusal, never a truncation or a zero-fill. A real
   transport maps its peer's exception response onto the same variant.
2. `protocol-mqtt::transport::MqttTransportError::PublishRefused` — the
   simulated broker sink validates each payload against the adapter's
   own rules before accepting; a malformed payload is a named refusal,
   never a fabricated receipt.

## 3. The permission binding (the wave-2 handoff, honored)

`adapters/permissions.rs` binds all SIX adapters' reviewed capability
manifests — via the crates' OWN constructors (`vtn_manifest`,
`client_manifest`, `outstation_manifest`, `uplink_manifest`,
`import_manifest`; 2030.5's manifest is reconstructed with the exact
fields its crate's server tests pin) — into
`flexcore_kernel::adapter_permissions::{AdapterBinding,
BoundAdapterRegistry}`, each through `check_command_relay_only`. The
fleet binds with relay-only authority fleet-wide, pinned by test. The
MQTT manifest's recorded publish-capability under-declaration (record
43 §2) is CONSUMED AS-IS — the gap stays a reviewed common-layer
handoff to the human; this batch does not paper over it and does not
edit the common vocabulary.

## 4. The DOE profile seam (the record 63 §5 handoff, honored)

`crates/flexcore-simulator/src/doe_profile.rs` is the DOE behaviour
profile the record 63 §5 handoff names: it consumes
`flexcore_domain::doe_envelope::{evaluate_envelope,
effective_envelope_bounds_mw, DynamicOperatingEnvelope,
StaticConnectionLimit}` — the domain's semantics, never redefined — and
adds only the simulator's two jobs: per-tick sampling at an INJECTED
instant, and clipping a synthetic asset tick into the effective bounds
with named domain-type breach/curtailment records. TEST-ONLY fixture
constructors are provided; every produced record is synthetic data.

**Recorded decision — the v1 scenario schema is UNCHANGED**: a DOE
profile is a composition-time profile consumed by the engine's caller,
NOT a new `AssetType` in `scenario.rs`. Extending the frozen P2.13
versioned scenario artefact schema for a Phase 14 profile would churn a
frozen surface for no simulation gain; the `scenario.rs` and
`profiles.rs` Phase 2 code is byte-untouched by this batch.

## 5. UNVERIFIED items carried (none new; all inherited)

This batch adds NO new external standard claims. Every UNVERIFIED
label it inherits (CSIP paths, OpenADR route literals, SunSpec register
semantics, Sparkplug spellings, CIM attribute spellings, DNP3 object
groups, DOE primary-source semantics) stays exactly where its owning
record placed it; the simulated counterparts exercise design-surface
types and never touch the UNVERIFIED wire details at all.

## 6. Files created / modified

| File | Status |
|---|---|
| `crates/flexcore-simulator/src/adapters/mod.rs` | CREATED (module map + the hard boundary) |
| `crates/flexcore-simulator/src/adapters/sim_20305.rs` | CREATED (device client + scripted server transport + tests) |
| `crates/flexcore-simulator/src/adapters/sim_openadr.rs` | CREATED (VEN + tests) |
| `crates/flexcore-simulator/src/adapters/sim_modbus.rs` | CREATED (device + tests) |
| `crates/flexcore-simulator/src/adapters/sim_dnp3.rs` | CREATED (master + fixture inventory + tests) |
| `crates/flexcore-simulator/src/adapters/sim_mqtt.rs` | CREATED (broker sink + tracker + tests) |
| `crates/flexcore-simulator/src/adapters/sim_cim.rs` | CREATED (fixture source + tests) |
| `crates/flexcore-simulator/src/adapters/permissions.rs` | CREATED (kernel permission binding + tests) |
| `crates/flexcore-simulator/src/doe_profile.rs` | CREATED (DOE profile + tests) |
| `crates/flexcore-simulator/src/lib.rs` | MODIFIED (two additive module declarations) |
| `crates/flexcore-simulator/Cargo.toml` | MODIFIED (8 additive dependency lines with the recorded rationale) |
| `crates/flexcore-simulator/tests/p14_17_simulated_adapters.rs` | CREATED (the integration suite — WRITTEN-UNRUN) |
| `Cargo.toml` (root) | MODIFIED (ONE additive `[workspace.dependencies]` line: `flexcore-kernel`) |
| `crates/adapters/protocol-modbus/src/transport.rs` | MODIFIED (ONE additive error variant + Display arm) |
| `crates/adapters/protocol-mqtt/src/transport.rs` | MODIFIED (ONE additive error variant + Display arm) |
| `crates/adapters/protocol-{openadr,modbus,dnp3,mqtt,cim}/src/lib.rs` | MODIFIED (ONE additive `ADAPTER_ID` const each — the P14.02 constant pattern, canonical spellings matching each crate's own scope tests; consumed by P14.18's registry and the permission binding) |

Byte-untouched by this batch: `crates/flexcore-simulator/src/scenario.rs`,
`profiles.rs`, `engine.rs`, `source.rs`, `rng.rs`; the kernel's
`manifest.rs` (the never-granted set); `flexcore-domain` (DOE semantics);
`flexcore-protocol-ports` (the publish-capability gap stays a handoff);
`ledger_phase_14.md` (no checkbox ticked); docs/foundations, docs/plan,
.agent.
