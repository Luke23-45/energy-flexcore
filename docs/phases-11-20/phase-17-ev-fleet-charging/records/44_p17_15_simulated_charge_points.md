# P17.15 — Simulated charge points: the in-process OCPP-J loopback wired THROUGH the `CsmsTransport` seam, with the SYNTHETIC V2G behaviour profile gated by the five-condition gate

| Field | Value |
|---|---|
| Subtask | P17.15 (simulated charge points): `flexcore-simulator` extension — simulated OCPP charge points speaking OCPP-J against the CSMS adapter (hardware-free testing; fulfills the P14.17 promise that named the OCPP counterpart for Phase 17); V2G behaviour profile linkage added — the existing EVSE profile is charging-only, and the addition is honestly labelled SYNTHETIC |
| Wave / agent | Phase 17 wave 2, Agent A |
| Status | **WRITTEN-UNRUN**: module + complete test files, never compiled or executed. No charge point is connected, no socket/TLS/HTTP exists anywhere in the addition, no OCPP message was ever exchanged with a real device, no conformance/interoperability/certification claim is made. Every identity carries the `sim-ocpp` prefix (never presentable as a real charge point). |
| Sources | P14.17 record 81 (the simulated-counterpart pattern + the OCPP promise), wave-1 record 01 §7 (the `CsmsTransport` seam contract), `flexcore_ocpp_adapter` (the surfaces consumed), report §11.5 / record `09_11_load_ev_models.md` (the five-condition gate) |
| Files touched | `crates/flexcore-simulator/src/adapters/sim_ocpp.rs` (NEW); `crates/flexcore-simulator/src/adapters/mod.rs` (additive module entry + table row — this agent's file); `crates/flexcore-simulator/src/profiles.rs` (additive linkage COMMENT only — the EVSE profile's charging-only behaviour and code are untouched); `crates/flexcore-simulator/Cargo.toml` (additive workspace-member dependency line — shared file, re-read before edit); `crates/flexcore-simulator/tests/p17_15_simulated_ocpp_charge_points.rs` (NEW, written-unrun) |

## 1. The seam is honoured BOTH ways

The wave-1 handoff is implemented verbatim: the simulator wires THROUGH
the `CsmsTransport` seam (`flexcore_ocpp_adapter::csms::CsmsTransport`),
never around it. `SimulatedCsmsTransport` implements the trait; its
`serve` runs the DEFAULT SCRIPT against a real `CsmsServer` and returns
Ok only for a completed exchange. The contrast is pinned by test: the
`UnwiredCsmsTransport` STILL refuses with `CSMS_TRANSPORT_SEAM_NOT_WIRED`
(the seam contract unchanged — wiring the simulator changed nothing about
the real transport gate), while the simulated transport completes its
IN-PROCESS loopback through the same trait.

## 2. What the loopback is (and is not)

The "wire" is the function boundary between the simulated charge point
and the server skeleton, and every frame crosses it through the adapter's
OWN codec (`OcppFrame::encode` / `OcppFrame::decode`), negotiation
(`session::negotiate`), registry (`BTreeMap` accept/duplicate/cap rules),
trackers, and queue surfaces. NOT simulated: the CSMS-side response
generation (the block handlers are the adapter's implementation-time work;
the loop records decoded frames, never an invented acceptance). The
script is deterministic: declared scenario base instant + fixed per-step
offsets, step-indexed unique ids (`sim-NN`), no wall-clock read, no
randomness (the Phase 2 simulator discipline at the OCPP tier).

## 3. The default script and the relay step

Deterministic depot session: connect (negotiate + accept) → Boot →
status(Available) → Heartbeat → status(Occupied) → TransactionEvent
Started → Updated → [CSMS-initiated SetChargingProfile relay + Accepted
answer, when a fixture is declared] → TransactionEvent Ended →
status(Available) → named end ("script complete").

The relay step composes the profile through the REAL composer and the
REAL ISSUED-only relay gate (`charging::into_relay_request`); the
`CommandState::Issued` it passes is the LABELLED HARNESS FIXTURE
(`HARNESS_ISSUED_FIXTURE_STATEMENT` travels with the module — not a real
dispatch, no gate was passed, no charge point was contacted). The
simulated station VALIDATES the drained payload with the model's own
rules: a well-behaved station answers Accepted (with the stored-not-applied
statement recorded verbatim in the session log) or CALLERROR for a payload
it cannot interpret — never a blind accept. An unsupported subprotocol
offer (e.g. `ocpp1.6`) meets the REAL fail-closed negotiation and the
P17-E-1 refusal — the subprotocol set is not widened.

## 4. The V2G behaviour profile: SYNTHETIC and GATED

The existing Phase 2 EVSE profile (`scenario::EvseParams`, consumed by the
simulated charge point) is CHARGING-ONLY ("no V2G" in profiles.rs) — that
statement and its code are unchanged (the profiles.rs edit is a linkage
COMMENT only). The additive `SimulatedV2gProfile` (discharge window + power)
is labelled `SYNTHETIC` (`SIMULATED_V2G_IS_SYNTHETIC_STATEMENT`: stated
fixture data, never a validated physical model, never evidence about real
vehicles, never a dispatch authorization) and can attach to a charge point
ONLY through `V2gBehaviourLink::attach`, which refuses a DISABLED
five-condition `V2xConfig` gate with the named `V2gGateDisabled` error —
the frozen §11.5 gate is structurally in the path and cannot be bypassed
by the simulator. A charge point without the link declares 0.0 discharge
everywhere.

## 5. Dependency-rule verification (composition legality)

`flexcore-simulator` is a COMPOSITION crate and `flexcore-ocpp-adapter` is
a COMPOSITION crate in `tools/check_dependency_rule.py` (the P14.17
precedent: six protocol adapters were added the same way); the
composition→composition edge is legal with NO checker change. The additive
`Cargo.toml` line is `flexcore-ocpp-adapter = { workspace = true }` (the
workspace entry existed since wave 1; no new EXTERNAL dependency). The
final `python -I tools/check_dependency_rule.py` run (close-out record 59)
reports 29 workspace members green.

## 6. Test coverage (written-unrun)

In-module tests plus `tests/p17_15_simulated_ocpp_charge_points.rs` pin:
the full depot script through the real surfaces (10 logged steps, sim
identity in every line, ENDED snapshot, named end reason); two simulated
charge points registering deterministically + the duplicate refusal; the
relay step through the real composer/relay/tracker path with the
stored-not-applied statement verbatim; the seam contract both ways
(unwired refuses, simulated completes); the relay-fixture transport
variant; the 1.6 refusal meeting the real negotiation (P17-E-1 reason,
nothing registered); the V2G profile gating (disabled gate refused,
enabled gate attaches through the gate's own procedure, window semantics,
charging-only default); the harness-fixture Issued contrast (PROPOSED
refused by the real relay gate); fail-fast constructions; and the honest
connection report (NEGOTIATED, profile 3).

## 7. Honest boundaries

1. **WRITTEN-UNRUN** — conventions §1.3; execution is a human gate.
2. **No hardware, no wire** — no socket, no TLS, no HTTP, no WebSocket
   anywhere in the addition; the real transport remains an
   implementation-time human gate (record 01 §7).
3. **Never a real peer, never presentable as one** — `sim-ocpp` identity
   prefix on every identity string; the boot payload's vendor/model are
   SYNTHETIC declarations, never real products.
4. **No capability claim** — the loopback proves nothing about real
   charge points, real networks, or the OCA text; every protocol detail
   remains under the wave-1 UNVERIFIED registers (plan §7.5).
