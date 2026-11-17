# P14.11 — MQTT 5 + Sparkplug B edge transport: the edge-agent uplink option

| Field | Value |
|---|---|
| Subtask | P14.11 — the MQTT 5 + Sparkplug B edge-transport adapter: Sparkplug B BIRTH/DEATH state management, per-device credentials, topic namespace design; the NATS-at-the-edge evidence noted without productizing it [S-39][S-40] |
| Wave / agent | Phase 14 wave 2, Agent A |
| Status | WRITTEN-UNRUN: model types, declared namespace, session state machine, credential bindings, uplink option, transport SEAM, scope declaration — written, never compiled or executed. No broker was reached, no connection opened, no message exchanged, no topic published (conventions §1; the first human build is the program gate) |
| Sources | [S-39] (Ubidots MQTT-vs-Sparkplug secondary comparison), [S-40] (OpenFMB — the NATS-at-the-grid-edge evidence, NOTED not productized); the P6.07 uplink rules (`crates/edge-agent/src/uplink_mtls.rs`), the P6.09 buffer discipline, the P14.01 common port layer (record 01) |
| Files touched | `crates/adapters/protocol-mqtt/` (NEW: `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/topic.rs`, `src/birth_death.rs`, `src/credentials.rs`, `src/transport.rs`, `src/uplink.rs`, `src/scope.rs`, `tests/topic_namespace.rs`, `tests/birth_death.rs`); additive root `Cargo.toml` entries; additive `tools/check_dependency_rule.py` entries |

## 1. What landed (deliverable class: design-and-model)

| Module | Content |
|---|---|
| `model.rs` | Sparkplug B identity and payload model types: `GroupId`/`EdgeNodeId`/`DeviceId` (validated; 128-char declared bound UNVERIFIED), `SparkplugMessageKind` (NBIRTH/DBIRTH/NDATA/DDATA/NDEATH/DDEATH/STATE/NCMD spellings — ALL UNVERIFIED), `SparkplugSeq` (wrap at the DECLARED `SEQ_MODULUS = 256` — UNVERIFIED wrap point), `BdSeq` (declared saturating, non-wrapping — a design pin pending the check), `SparkplugMetric` (type/value agreement enforced at construction), `SparkplugPayload` (seq required for BIRTH/DATA) |
| `topic.rs` | The DECLARED topic namespace: `SPARKPLUG_B_NAMESPACE = "spBv1.0"` (UNVERIFIED prefix), the segment order `namespace/group/message_type/edge_node[/device]` (UNVERIFIED grammar), `ValidatedTopic` — no free-form topic string; kind/device-segment mismatch is a construction error |
| `birth_death.rs` | The BIRTH/DEATH session state machine: `BirthDeathTracker` with `AwaitingBirth`/`Live{seq,bd_seq}`/`Dead{reason,bd_seq}`; every observation is `Accepted` or `Refused` BY NAME (`data_without_live_birth`, `death_without_live_birth`, `sequence_mismatch`, `stale_rebirth`, `generation_mismatch`, `invalid_payload`); the state is not publicly writable — a skipped BIRTH is not expressible |
| `credentials.rs` | Per-device credential bindings: `DeviceCredentialBinding` (client id, username, `CredentialSourceReference` handle) with **NO secret field — absence is structural** (the never-granted `secret.read` rule); `CredentialBindingSet` with duplicate-client-id refusal and deterministic ordering; a test pins the serialized shape carries no secret-looking field |
| `transport.rs` | The MQTT wire SEAM: `MqttUplinkTransport` (publish-only; NO subscribe/receive method — the command path does not exist), `MqttConnectionConfig::validate` refusing plaintext (the P6.07 TLS rule) and requiring a trust-anchor reference from the deployment's credential record, `DeclaredQos` (UNVERIFIED default), `UnwiredUplinkTransport` with `SeamNotWired` |
| `uplink.rs` | The edge-agent uplink OPTION: `UplinkPoint` (canonical values only — MW / MWh / SOC fraction / declared boolean), the metric-name convention **the canonical unit travels in the declared name** (Sparkplug metrics carry no unit field — UNVERIFIED; a point without a unit is refused, never defaulted), `SparkplugUplinkPlan` mapping points → metrics + device-scoped DATA topics + the BIRTH topic; the edge-agent `UplinkChannel` seam (`crates/edge-agent/src/ports.rs`) is NAMED as the wave-3 composition point — **no `flexcore-edge-agent` dependency** (nothing depends ON composition crates except apps) |
| `scope.rs` | CONSUMES the common port layer: `uplink_scope()` (role CLIENT — edge-initiated, nothing listens; control scope MONITOR ONLY; resources TELEMETRY + DISCOVERY) and `uplink_manifest()` (the one common manifest shape, `SupervisoryRelayOnly`) |

## 2. Dependence on Agent A's P14.01 layer

Same pattern as records 21/23/24: the crate consumes the common
`ProtocolScopeDeclaration` and `ProtocolAdapterManifest`
(`flexcore-protocol-ports`, workspace pin) — one scope/manifest
vocabulary, no parallel one. Role mapping: the uplink publisher = the
common CLIENT role (the adapter dials OUT to the broker; nothing
listens — the P6.07 structural rule). The monitor-only gate refuses any
command-relay request at the common scope gate.

**Capability vocabulary gap — recorded, not papered over:** the common
`DeclaredCapability` set (record 01) expresses READ-side capabilities
and has no outbound-publish variant. The manifest therefore declares
`Discovery` (the BIRTH inventory declaration — named in the common
vocabulary's own documentation) and `BoundedHistory` (the offline
store-and-forward re-publish semantics, the P6.09 pattern), and the gap
travels in the manifest notes and this record. Adding a
Publish/Telemetry variant is a REVIEWED common-layer change (record 01
is the owning record) — a handoff item to wave 3 / the human. Until
then the manifest under-declares rather than mis-declares.

## 3. The BIRTH/DEATH trust rules (the subtask's core deliverable)

Sparkplug's state management is what makes MQTT usable for DER fleets:
a BIRTH declares the FULL metric inventory on every (re)connection; a
DEATH declares disconnection (the MQTT will). The machine makes the
trust rules structural:

1. **No DATA is trusted without a live BIRTH** — refused by name before
   any inventory exists.
2. **A DEATH (own generation) ends the session**; DATA after death is
   refused. A stale DEATH (older bdSeq generation) is REFUSED — it must
   not kill a newer session.
3. **A re-BIRTH is accepted only as a generation change** (new bdSeq);
   a same-generation re-birth is refused — a silent inventory reset
   would hide a publisher defect.
4. **DATA must continue the sequence chain** (`seq` = successor of the
   last accepted, wrapping at the DECLARED modulus); a gap is refused
   and RECORDED — recovery is a re-BIRTH, never a guess.
5. **The transport seam's connection-lost report** ends a live session
   (`DeathReason::ConnectionLost`) — the machine and the broker-side
   will agree by design.
6. STATE is recorded, never guessed into a session effect (the
   rebirth/STATE interaction is an implementation-time check).

All ordering rules are secondary-source readings ([S-39][S-40]);
each is UNVERIFIED-labeled at its definition, with the implementation-
time check named in the module docs.

## 4. UNVERIFIED register (implementation-time checks against the Sparkplug B text — [S-39][S-40] are secondary)

| # | Detail | Where |
|---|---|---|
| 1 | The namespace prefix (`spBv1.0`) and the topic grammar/segment order; the STATE topic's shape | `topic.rs` constants + `ValidatedTopic` |
| 2 | Message-type strings (NBIRTH/DBIRTH/NDATA/DDATA/NDEATH/DDEATH/STATE/NCMD) | `model.rs::SparkplugMessageKind::topic_segment` |
| 3 | The `seq` wrap point (declared 256) and reserved values | `model.rs::SEQ_MODULUS` |
| 4 | The `bdSeq` width/wrap semantic (declared saturating u64) and which messages carry it | `model.rs::BdSeq`; `SparkplugPayload` |
| 5 | The payload field set (protobuf `metrics`/`seq`/`bdSeq`/`timestamp`/alias/flags) and its encoding | `model.rs::SparkplugPayload` (the ENCODING is a transport-seam concern) |
| 6 | The identity character sets/length bounds (declared 128) and client-id expectations | `model.rs` identity types; `credentials.rs` |
| 7 | The QoS the specification expects per message kind (declared QoS 1 default) | `transport.rs::DeclaredQos` |
| 8 | The rebirth/STATE interaction and DEATH-seq carriage | `birth_death.rs` rules 5–6 |
| 9 | Whether metrics carry a unit field (declared: no — the unit travels in the metric name) | `uplink.rs` convention |

A mismatch found at those gates is a recorded edit to the named
constant/module — never a silent reinterpretation (plan §7 note 6).

## 5. The NATS-at-the-edge evidence — noted, not productized [S-40]

OpenFMB demonstrates NATS operating at the grid edge [S-40]; the
evidence is recorded here and in the scope's `not_modelled` list
(`no_nats_at_the_edge`). FlexCore's designed edge transport in this
batch is MQTT 5 + Sparkplug B; the existing `crates/adapters/nats`
crate stays the platform-INTERNAL bus (a different surface, already
landed); no NATS edge transport, no OpenFMB profile, and no second edge
protocol is added. A future NATS-at-the-edge decision is a
deployment E-decision, not a wave-2 change.

## 6. Honest boundaries

The transport is unwired: no MQTT client library exists in the
dependency line, no broker was reached, no connection was opened, no
topic was published or subscribed, no message was exchanged. No
Sparkplug B conformance or certification claim of any kind (conventions
§7.3). No secret material exists in the crate (structural). No command
path exists (structural). The two test files (`tests/topic_namespace.rs`,
`tests/birth_death.rs`) are written COMPLETE and UNRUN — execution is a
human gate. The simulated counterpart (a simulated broker + edge node)
is P14.17's deliverable (wave 3).

## 7. Handoff

- **Wave 3 (P14.17/P14.18/P14.20)**: the simulated edge node/broker
  exercises `transport::MqttUplinkTransport`; P14.18's edge-to-cloud
  mapping consumes `uplink::SparkplugUplinkPlan`; P14.20's budget binds
  to the manifest cadence and the refusal names (the P11 taxonomy
  consumes, never redefines). The common Publish-capability gap (§2) is
  the reviewed vocabulary change to consider there.
- **The human gates**: first workspace build; the mapping/session-test
  execution; the Sparkplug B text verification pass (§4); the TLS/trust-
  anchor wiring decision (the P6.07 record governs).
