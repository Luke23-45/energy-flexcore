# P14.18 — Edge-agent adapter seam extension: the generic-rest decision, the edge-to-cloud mapping, and the fleet uplink budgets

| Field | Value |
|---|---|
| Subtask | P14.18 (wave 3, Agent A) — policy for real protocol bridging at the edge |
| Deliverable class | Registry policy change + design-and-model module + WRITTEN-UNRUN tests |
| Honesty boundary | Nothing published, nothing connected, nothing exchanged. The injected uplink transport in every shipped composition is the adapter's own `UnwiredUplinkTransport` (the refusing default). The PLANNED protocol ids construct nothing. |

## 1. THE GENERIC-REST DECISION: REMOVED (explicitly, one way as instructed)

**Decision: `generic-rest` is REMOVED from the edge-agent adapter
registry. It is not spec'd.**

Reasoning (recorded, none silent):

1. **The placeholder's premise expired.** `generic-rest` was registered
   RESERVED in Phase 6 as "a recorded placeholder SLOT for a second
   adapter" — a slot held open while the edge had exactly one concrete
   adapter and no named second protocol. Phase 14 named the protocols:
   the adapter tier (P14.01) now carries six concrete protocol ids with
   reviewed scope declarations and manifests. A permanently
   never-constructible slot next to six named protocols is scaffolding
   without an owner — exactly what AGENTS.md §1 forbids ("introduce
   crates/slots only when they have real code, clear ownership, and a
   stable dependency direction").
2. **The extension rule it stood in for is better stated without it.**
   The honest rule is: *an id is registered when its implementation
   lands, never before.* A RESERVED slot inverted that (registered
   before implementation, forever). The new PLANNED registrations below
   carry expiry: each names the wave/record that must land before the
   id can construct, and a PLANNED id never becomes Implemented by a
   config edit — only by a reviewed code landing.
3. **Nothing depended on it.** `generic-rest` was never constructible;
   the registry's RESERVED refusal path is exercised through the
   `register` seam (documented, and the tests prove both arms); the
   shipped composition roots (`main.rs`) never referenced the id.

What changed:

- `crates/edge-agent/src/adapters/registry.rs`: the id is gone from
  `with_default_allowlist()`; the module id-table records the removal
  and its history (RESERVED, refused construction, Phase 6–13);
  `AdapterStatus::Reserved` itself is UNCHANGED (the status variant
  stays for reviewed future use; only the placeholder registration is
  removed). The registry's inline tests pin: the id is NOT allowlisted
  (`NotAllowlisted`), the six Phase 14 protocol ids ARE PLANNED, the
  allowlist length is 8 (1 Implemented + 1 TestOnly + 6 Planned).
- `crates/edge-agent/tests/adapter_acl_and_registry.rs` and
  `crates/edge-agent/tests/conformance_suite_p6_19.rs`: the two test
  sites that pinned the RESERVED slot updated to pin the REMOVED id
  (`NotAllowlisted`) and the PLANNED refusals instead — a removal
  necessarily updates the tests that pinned the slot; the P6.19 case's
  refusal assertion is preserved (a non-allowlisted id still refuses
  construction).
- `crates/edge-agent/src/adapters/mod.rs` and `src/lib.rs`: the two
  doc-table lines updated (documentation of the recorded change).

## 2. The Phase 14 protocol ids registered PLANNED (the P14.02 contract, executed)

The P14.02 contract stated: "Registered as PLANNED in the edge-agent
registry only when the edge seam wave (P14.18) lands — this constant is
the canonical spelling." P14.18 is that wave; the six ids are now
registered `AdapterStatus::Planned` with named wave references:

| Registered id | Canonical spelling source | Owning record |
|---|---|---|
| `ieee-2030.5` | `flexcore_protocol_20305::ADAPTER_ID` | record 02 |
| `openadr-vtn` | `flexcore_openadr_adapter::ADAPTER_ID` (added this batch) | record 21 |
| `modbus-client` | `flexcore_modbus_adapter::ADAPTER_ID` (added this batch) | record 23 |
| `dnp3-outstation` | `flexcore_dnp3_adapter::ADAPTER_ID` (added this batch) | record 24 |
| `mqtt-sparkplug-uplink` | `flexcore_mqtt_adapter::ADAPTER_ID` (added this batch) | record 43 |
| `cim-cgmes-import` | `flexcore_cim_adapter::ADAPTER_ID` (added this batch) | record 44 |

The five newly added `ADAPTER_ID` constants use the spellings each
crate's OWN scope/module tests already construct with — nothing was
invented. The registry registers the ids as reviewed string literals
with comments citing the canonical constants (pulling the constants in
by dependency would add five adapter-crate edges to the edge agent for
data the composition does not use yet; a follow-up may re-point to the
constants when the concrete edge implementations land).

**What PLANNED means here, precisely**: the concrete edge-side
implementations (a protocol adapter implementing the edge `EmsAdapter`
port or the edge-side of a protocol bridge) DO NOT EXIST. Construction
is refused with the named wave reference — the honest fail-closed
answer, exactly the `RegistryError::NotYetImplemented` path. No bridge
exists, no device is bridged, no claim is made.

## 3. The edge-to-cloud protocol mapping (the record 43 §2 composition point, wired as designed)

`crates/edge-agent/src/adapters/edge_to_cloud.rs` — the module the MQTT
record explicitly named as "wave 3's wiring":

- [`SparkplugUplinkChannel`] implements the edge's own `UplinkChannel`
  port by converting canonical edge samples into the MQTT adapter's
  `UplinkPoint` shapes and publishing through an INJECTED
  `MqttUplinkTransport`. The shipped default is the adapter's own
  `UnwiredUplinkTransport` — the honest, refusing default; the edge
  stays in degraded buffering exactly as the P6.05 runner contract
  requires. (New dependency: `flexcore-mqtt-adapter` on the edge agent —
  composition→composition, checker-legal; and
  `flexcore-protocol-ports` for the shared budget vocabulary — see
  record 83.)
- **The declared mapping is closed**: exactly four edge quantities map
  (active power, SOC, energy, availability — the adapter's own
  `UplinkQuantity` set). An unmapped quantity (reactive power, text
  states, baseline reference) is a NAMED refusal — never a silently
  dropped or reinterpreted sample.
- **Quality is marked, never upgraded**: a non-GOOD sample's metric is
  marked with the declared `.degraded` suffix; the platform decides
  trust, the edge never hides degradation.
- **No command path**: the `UplinkChannel` port has no receive method
  (P6.05) and this module adds none — no subscribe, no command decode,
  no setpoint surface. Absence is structural.

## 4. The bandwidth budgets for fleet uplinks

`FleetUplinkBudget` (same module) — declared, validated, no defaults:

- `max_samples_per_batch` — the per-forward cap (the runner's poll
  window feeds this);
- `max_forwards_per_interval` / `interval_millis` — the per-node
  message-rate budget, exposed through the SHARED
  `flexcore_protocol_ports::observability::MessageRateBudget` shape (one
  rate vocabulary across the platform — record 83; consumed, never
  redefined);
- `fleet_node_budget` — how many edge nodes the deployment budgets
  uplinks for; the `n`th+1 node registration is refused (fleet scale is
  declared, never unbounded);
- an unregistered node's forward is refused — no silent unlimited
  default exists.

The bounded-queue discipline needs no new buffer here: the edge's
store-and-forward spine is already bounded and order-preserving (P6.05
`LocalBuffer` port; P6.09 durable implementation with audited
drop-oldest) — the budgets bound the RATE into that spine, not its
capacity.

## 5. Files created / modified

| File | Status |
|---|---|
| `crates/edge-agent/src/adapters/edge_to_cloud.rs` | CREATED (mapping + budgets + tests) |
| `crates/edge-agent/src/adapters/registry.rs` | MODIFIED (generic-rest removed; six PLANNED ids; id-table and tests updated) |
| `crates/edge-agent/src/adapters/mod.rs` | MODIFIED (edge_to_cloud module + doc lines) |
| `crates/edge-agent/src/lib.rs` | MODIFIED (one doc-table line) |
| `crates/edge-agent/Cargo.toml` | MODIFIED (two additive dependency lines with recorded rationale) |
| `crates/edge-agent/tests/adapter_acl_and_registry.rs` | MODIFIED (the RESERVED-slot test pins the removal + the PLANNED ids) |
| `crates/edge-agent/tests/conformance_suite_p6_19.rs` | MODIFIED (Case 10 pins the PLANNED refusal + the removed id) |
| `crates/adapters/protocol-{openadr,modbus,dnp3,mqtt,cim}/src/lib.rs` | MODIFIED (ONE additive `ADAPTER_ID` const each — recorded under record 81 §6, consumed here) |

Byte-untouched: `crates/edge-agent/src/main.rs` (the composition roots
call `with_default_allowlist().with_openems_available()` — unchanged
semantics), the never-granted set, the P6.05–P6.20 module code (only
docs/registry/tests touched), `ledger_phase_14.md`.

## 6. Human gates named

(a) the first workspace build (everything here is WRITTEN-UNRUN);
(b) the P6.11 OpenEMS lab run (unchanged, still open); (c) when the
first protocol adapter's edge-side implementation is proposed, its
registry flip is a reviewed change with a mapping-table plan and a
P6.02 dependency-audit note (the registry's own extension rule).
