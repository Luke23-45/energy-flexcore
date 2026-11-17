# SunSpec Modbus Client Adapter Guide

| Field | Value |
|---|---|
| Scope | The FlexCore `protocol-modbus` adapter (`crates/adapters/protocol-modbus/`): a SunSpec information-model client over Modbus TCP/RTU for inverters (101/103), meters (201/203), and battery models (80x) |
| Owner record | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/23_p14_07_sunspec_modbus_client.md` |
| Implementation status | **WRITTEN-UNRUN.** Model types, register-map registry, unit conversion, polling budgets, and the transport SEAM exist as written code; nothing has compiled, no TCP connection has opened, no serial line was accessed, no register was read (`UnwiredTransport` answers every read with `SeamNotWired`) |
| Conformance status | **No conformance, certification, or compliance claim of any kind.** No SunSpec certification program is named in the repository's source register (UNVERIFIED-ABSENCE — evidence plan, records 101 §2) |
| Control status | Read/monitor only. The transport trait has NO write method — absence is structural; reactive sign is DECLARED per device profile, never inferred |

## 1. What the adapter is (and is not)

| Aspect | Recorded state |
|---|---|
| Role | Modbus CLIENT polling SunSpec devices/gateways (the common CLIENT role, MONITOR-ONLY scope) |
| Information models | 101/103 inverters, 201/203 meters, 801/802/804/805 battery models — model ids pinned in one place, ALL UNVERIFIED against the SunSpec specifications [S-16] |
| Register maps | REVIEWED PER-DEVICE-MODEL DATA in `RegisterMapRegistry` — no runtime discovery, no guessed maps; an unregistered device model is a named refusal |
| Direction | Reads only; the write direction does not exist structurally |

## 2. Deployment (what a deployment would configure)

Every connection value is DECLARED — the code has no defaults, not even
for well-known ports:

| Configuration | Honesty note |
|---|---|
| `TransportConfig::Tcp { host, port, unit_id }` or `Rtu { port_name, baud, parity, unit_id }` | TCP port 502 / RTU framing parameters are UNVERIFIED conventions, declared per deployment, never assumed |
| The register map per device model | registered per reviewed device model BEFORE use; lab-verified against the real device before registration is the named human gate (record 23 §4) |
| The reactive-sign convention per device profile | REQUIRED — no convention-free conversion exists; the same numeric VAr value maps to opposite directions under the two declared conventions (pinned by test) |
| The `DeclaredScaling` / scale factors | applied per the map's SF registers (the ×10^SF rule UNVERIFIED); fail-closed plausibility band |

## 3. Security

| Topic | Design position |
|---|---|
| Transport security | Modbus TCP/RTU has no native encryption/authentication; the security posture of a Modbus hop is a DEPLOYMENT decision (dedicated segment / gateway policy) — no security claim is made or implied |
| Secrets | None handled by the adapter; deployment credentials travel as secret REFERENCES (the never-granted `secret.read` rule) |
| Credentials | None in the crate — the adapter authenticates nothing at the protocol layer |

## 4. Operations (budgets, observability, failure modes)

| Topic | Design position |
|---|---|
| Polling budgets | `GatewayPollingBudget` (max registers per read — the 125-register Modbus read limit, UNVERIFIED; devices per gateway; minimum poll interval) — declared constraints, never measured limits; `assert_read_fits` / `assert_scan_fits` fail-closed |
| Block splitting | `reads_needed_for` computes the deterministic block plan for large models |
| Connection states | The common `ProtocolConnection` vocabulary — consumed by the P11 taxonomy, never redefined |
| Failure modes | Unregistered device model → named refusal; short register windows refused; missing scale-factor registers refused; overlapping/out-of-range map fields refused at registration; unit-kind mismatch refused, never coerced; unwired transport → `SeamNotWired` |
| Unit conversion | W → MW (/1e6, export sign passes through), VAr → MVAr under the declared convention, Wh → MWh, percent → SOC fraction via the named boundary constructor |

## 5. Honest boundary

No device was polled, no register was decoded from a real device, no
gateway exists. Per-device register maps are lab-verified BEFORE
registration (a human gate); a map that disagrees with the lab device is
a recorded registry edit, never a silent reinterpretation. The
simulated-device counterpart is P14.17's deliverable surface; simulated
data is never evidence about real devices. FlexCore makes no SunSpec
conformance claim.
