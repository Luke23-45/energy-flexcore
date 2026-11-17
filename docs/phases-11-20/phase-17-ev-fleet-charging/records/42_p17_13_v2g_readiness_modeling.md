# P17.13 — V2G readiness modeling: per-EVSE/vehicle bidirectional capability on top of the frozen five-condition gate, OCPP 2.1 hook design records, the North-American AC path as a separate future integration

| Field | Value |
|---|---|
| Subtask | P17.13 (V2G readiness modeling): bidirectional capability per EVSE/vehicle in the domain model (the five-condition `V2xConfig` gate already exists — extended additively, never redefined); OCPP 2.1 bidirectional/DER hooks (NotifyAllowedEnergyTransfer, AFRRSignal, SetDERControl) as DESIGN records; the North-American AC path (IEEE 2030.5 V2G-AC / SAE J3072 / UL 1741 SC) recorded as a SEPARATE future integration [S-125][S-4][S-131][S-144]. ADR-010 discipline: readiness records never authorize dispatch |
| Wave / agent | Phase 17 wave 2, Agent A |
| Status | **WRITTEN-UNRUN** (additive domain module + written-unrun tests; nothing compiled or executed). No V2G session ever existed, no bidirectional energy was ever transferred, no device was ever readied in the field, no conformance/certification/safety claim is made anywhere. |
| Sources | [S-125] ocpp.md OCPP 2.1 (the hook names — UNVERIFIED), [S-4] SunSpec IEEE 2030.5 V2G-AC Profile v1.0, [S-131] CharIN V2G, [S-144] QualityLogic V2G-AC implementation; SAE J3072 is named in phase-17 plan §2 but has NO §11 source entry — labelled UNVERIFIED/ASSUMPTION in the module statement; report §11.5 and domain record `09_11_load_ev_models.md` (the five-condition gate) |
| Files touched | `crates/flexcore-domain/src/ev_v2g_readiness.rs` (NEW); `crates/flexcore-domain/tests/ev_v2g_readiness.rs` (NEW, written-unrun); `crates/flexcore-domain/src/lib.rs` (additive `pub mod ev_v2g_readiness;` entry — the shared-file edit, re-read before edit, additive single entry) |

## 1. The five-condition gate is CONSUMED, never redefined

The §11.5 `V2xConfig` gate (EV, EVSE, protocol, interconnection agreement,
AND market rules; OFF by default; decided once; enabled only with all five
conditions evidenced) is byte-identical — no line of `ev_fleet_model.rs`
was touched by this batch. The new module is ADDITIVE on top of it:

| Readiness verdict | Device declares bidirectional capability (with evidence) | Five-condition gate enabled |
|---|---|---|
| `NOT_CAPABLE` | no | either |
| `CAPABLE_BUT_GATE_DISABLED` | yes | no |
| `READIED` | yes | yes |

`Readied` is a documented capability ASSESSMENT. The record holds the gate
snapshot (the frozen `V2xConfig` value), so the path to `Readied` runs
ONLY through the gate's own `V2xConfig::enable` procedure — this module
adds no enable path of its own and cannot bypass the gate. A charge-only
device is `NOT_CAPABLE` regardless of any gate state (the device truth
governs). A capability claim without a non-empty evidence string is a
construction error (the §11.5 evidence discipline applied per device).

## 2. ADR-010 discipline: readiness never authorizes dispatch

`READINESS_NEVER_AUTHORIZES_DISPATCH` (a module constant, tested verbatim):
a readiness record is an input, a record, and a proposal — never a
command. The structural guarantee: the module's public surface exposes
only records, verdict enums (`NOT_CAPABLE` /
`CAPABLE_BUT_GATE_DISABLED` / `READIED`), and statements — there is no
function that returns a `CommandState`, a `DispatchCommand`, or any
lifecycle state, and the ledger (`V2gReadinessLedger`, deterministic
`BTreeMap`) records ASSESSMENTS (replacement is a record update, never a
lifecycle transition). Every dispatch — including any future
bidirectional dispatch — enters the frozen 14-state lifecycle at PROPOSED
and passes the three gates. The frozen transition table and the
never-granted set are untouched (no byte changed in `command_state.rs` or
`kernel/manifest.rs`).

## 3. Per-EVSE / per-vehicle

The record is keyed per subject (`EVSE:<key>` / `VEHICLE:<key>`,
BTreeMap-deterministic), because the §11.5 conditions and the device
capability are per-device truths, not fleet-wide ones. The ledger exposes
`assessment`, `readiness_of`, `snapshot_keys`, and size queries — nothing
else.

## 4. The OCPP 2.1 hooks are DESIGN records, never adapter actions

`DECLARED_OCPP21_HOOKS` (in the recorded order) with per-hook notes, each
carrying its UNVERIFIED label and the lifecycle rule that would govern it:

| Hook (UNVERIFIED name, [S-125]) | Design-record note |
|---|---|
| `NotifyAllowedEnergyTransfer` | The energy-transfer permission report in the 2.1 bidirectional block. If it ever lands it is an INPUT record (the P17.06 vehicle-proposal pattern) — never an auto-command |
| `AFRRSignal` | The aFRR signal surface of the 2.1 DER/bidirectional blocks. An optimization INPUT feeding scheduling — never a real-time control path (no primary frequency response, no synthetic inertia, no sub-cycle control; foundations §0.3) |
| `SetDERControl` | The 2.1 DER control functional block's control object. If it ever lands it is a COMMAND candidate: PROPOSED entry, three gates, ISSUED-only relay (the P17.05 rule) |

NONE of these is an action of the CSMS adapter's closed vocabulary
(`flexcore_ocpp_adapter::framing::Action` is untouched — no widening); no
adapter in this program sends, receives, or parses any of them. OCPP 2.1
is cited for capability SCOPE only — never as a claim that any EVSE,
vehicle, or device implements or conforms to it (the report §11.5 / P3.11
scope statement, unchanged). Widening the vocabulary or the subprotocol
set to reach these hooks would be a reviewed scope change (P17-E-1 stays
OPEN; D5 stays human-closed).

## 5. The North-American AC path: a SEPARATE future integration

`NA_AC_PATH_STATEMENT` records the IEEE 2030.5 V2G-AC path (SunSpec V2G-AC
Profile v1.0 [S-4], the SAE J3072 AC bidirectional interface named in the
phase plan — no §11 source entry, UNVERIFIED/ASSUMPTION — and UL 1741 SC
equipment listings, with the profile landscape [S-131][S-144]) as a
SEPARATE FUTURE INTEGRATION: OUT OF SCOPE for the OCPP CSMS adapter (the
adapter's `no_bidirectional_v2g_control` not_modelled entry, unchanged),
not designed, not implemented, not claimed anywhere in this program. The
statement exists so the path is neither silently promised nor silently
omitted.

## 6. Test coverage (written-unrun)

In-module `#[cfg(test)]` tests plus `tests/ev_v2g_readiness.rs` pin: the
full 2×2 readiness matrix (device capability × gate state); the fail-closed
constructions (empty subject key, untraceable capability claim); the gate's
own enable procedure still refusing incomplete five-condition sets (the
gate is consumed, not redefined); ledger determinism (BTreeMap key order)
and assessment replacement; the hook names/notes (UNVERIFIED labels pinned
so a change is a conscious edit); the NA-AC statement's sources and
separation wording; and the ADR-010 statement + verdict-only surface.

## 7. Honest boundaries and handoffs

1. **WRITTEN-UNRUN** — the first human build gates the module like
   everything else; a Readied verdict in a test fixture is a SYNTHETIC
   assessment over synthetic evidence labels (the named-fixture
   discipline of record `09_11_load_ev_models.md`), never a field claim.
2. **Handoff to P17.15** (same wave): the simulated V2G behaviour profile
   attaches to charge points ONLY through an ENABLED gate
   (`V2gBehaviourLink::attach` refuses a disabled gate) — the simulator
   cannot manufacture readiness this module would not record.
3. **Handoff to wave-2 Agent B**: the hooks (§4) are scope citations, not
   deliverables; the AFIR/OCPI records should not describe them as
   implemented. P17-E-1 and D5 remain OPEN (nothing here closes either).
