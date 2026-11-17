# MQTT 5 + Sparkplug B Uplink Adapter Guide

| Field | Value |
|---|---|
| Scope | The FlexCore `protocol-mqtt` adapter (`crates/adapters/protocol-mqtt/`): the edge-agent uplink OPTION — Sparkplug B state management over MQTT 5, publishing canonical fleet telemetry to a broker |
| Owner record | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/43_p14_11_mqtt_sparkplug_b_uplink.md` |
| Implementation status | **WRITTEN-UNRUN.** Payload/identity model, topic namespace, BIRTH/DEATH state machine, credential bindings, uplink plan, and the transport SEAM exist as written code; nothing has compiled, no broker was reached, no connection opened, no topic was published (`UnwiredUplinkTransport` refuses with `SeamNotWired`) |
| Conformance status | **No Sparkplug B conformance or certification claim of any kind.** No certification program is named in the repository's source register ([S-39][S-40] are secondary comparisons — UNVERIFIED-ABSENCE; evidence plan, records 101 §2) |
| Control status | Publish-only. The transport trait has NO subscribe/receive method — the command path does not exist structurally; nothing listens (the edge dials OUT to the broker) |

## 1. What the adapter is (and is not)

| Aspect | Recorded state |
|---|---|
| Role | Edge publisher (common CLIENT role; MONITOR-ONLY scope; TELEMETRY + DISCOVERY resources) |
| Namespace | `spBv1.0` with the segment order `namespace/group/message_type/edge_node[/device]` — DECLARED, UNVERIFIED against the Sparkplug B text; no free-form topic strings (`ValidatedTopic`) |
| State management | `BirthDeathTracker` — NBIRTH/DBIRTH/NDATA/DDATA/NDEATH/DDEATH/STATE handling (message-kind spellings UNVERIFIED) with the trust rules of record 43 §3 |
| Uplink mapping | `SparkplugUplinkPlan` maps canonical points (MW / MWh / SOC fraction / declared booleans) to metrics; the canonical unit travels IN the declared metric name (Sparkplug metrics carry no unit field — UNVERIFIED; a point without a unit is refused) |
| Not productized | The NATS-at-the-grid-edge evidence [S-40] is NOTED, not productized (`no_nats_at_the_edge` in the scope's `not_modelled` list); the `nats` crate stays the platform-internal bus |

## 2. Deployment (what a deployment would configure)

| Configuration | Honesty note |
|---|---|
| `MqttConnectionConfig` | plaintext REFUSED (the TLS rule); a trust-anchor reference is REQUIRED from the deployment's credential record; broker address/auth declared per deployment |
| `DeclaredQos` | the declared default (QoS 1) is UNVERIFIED against the specification's per-kind expectations |
| Sequence parameters | `SEQ_MODULUS = 256` and the saturating `BdSeq` are DESIGN PINS pending the verification pass (UNVERIFIED) |

## 3. Security (credentials, per-device identity)

| Topic | Design position |
|---|---|
| Per-device credentials | `DeviceCredentialBinding` (client id, username, `CredentialSourceReference` handle) — NO secret field, absence is structural (the never-granted `secret.read` rule); the serialized shape carries no secret-looking field (pinned by test) |
| Credential set | `CredentialBindingSet` refuses duplicate client ids and orders deterministically |
| Secrets custody | Phase 13 secrets surface resolves the references; the adapter never reads raw material |
| Broker trust | The broker connection requires TLS and a deployment-declared trust anchor reference (the P6.07 edge-uplink posture carries forward) |

## 4. Operations (budgets, observability, failure modes)

| Topic | Design position |
|---|---|
| Trust rules | No DATA trusted without a live BIRTH; a DEATH ends the session; a stale DEATH is refused (it must not kill a newer generation); a re-BIRTH is accepted only as a generation change; DATA must continue the seq chain — a gap is refused and RECORDED (recovery is a re-BIRTH, never a guess); a connection-lost report ends a live session |
| Refusal names | `data_without_live_birth`, `death_without_live_birth`, `sequence_mismatch`, `stale_rebirth`, `generation_mismatch`, `invalid_payload` — every observation is Accepted or Refused BY NAME; the state is not publicly writable |
| Offline behavior | The store-and-forward re-publish semantics are the `BoundedHistory` capability declaration (the P6.09 pattern) |
| Connection states | The common `ProtocolConnection` vocabulary — vocabulary only: no adapter implements the port in this baseline (the recorded FRV1-R6-001 deferral; the shipped default refuses fail-closed by name), so connection reporting rides the adapter's own session/birth-death rules until the first adapter composition. The P11 taxonomy consumes the names, never redefines them |

## 5. Honest boundary

No broker was stood up or reached, no topic was published, no message
was exchanged, no BIRTH or DEATH was observed on a wire. All ordering
rules are secondary-source readings, UNVERIFIED-labeled at their
definitions; the implementation-time verification pass against the
Sparkplug B text is a named human gate (record 43 §4). FlexCore makes
no Sparkplug B conformance claim.
