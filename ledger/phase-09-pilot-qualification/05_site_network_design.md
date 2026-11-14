<!-- P9.05 — Site network and connectivity design (decision record). Design TEMPLATE only: zone/flow diagram skeleton, allowlist template, EXAMPLE-labelled illustrative allocations only. No real site, IP, VLAN, port, or customer review exists. -->
# P9.05 — Site Network and Connectivity Design (Design Template)

Date: 2026-11-14. Subtask: P9.05 (`ledger_phase_9.md` §P9.05).
Owner role: Platform engineering (with the customer network/OT owner).
Report trace: §23 Stage 9; §3.3; §17.2; §18.3.
Inputs: P9.04 integration design (TEMPLATE — `04_customer_integration_design.md`);
customer network architecture and firewall policy — **does not exist yet**;
P9.06 boundary review (run in parallel; this design is its input —
`06_ot_it_boundary_d6.md`).

## 1. Honest status — the plain statement

**No pilot site exists, so no site network design exists and none is
claimed.** This record delivers the network-design TEMPLATE: the zone/flow
diagram skeleton (§3–§4), the connectivity-pattern options (§5), the
allowlist structure (§6), the resilience and buffering design (§7), and the
segmentation requirements handed to P9.06 (§8). Every site-specific value —
zone membership, endpoint addresses, ports, VLAN IDs, firewall rules,
buffer sizes, measured bandwidth/latency — is EMPTY or explicitly labelled
EXAMPLE. **No IP address, VLAN ID, hostname, or port in this record is a
real allocation, a plan of record, or an instruction to configure anything.**
Nothing was configured, installed, or connected; no customer network/OT
owner review occurred (acceptance criterion (d) is NOT met and not claimed).

The only network property fixed below is one the BUILT code already
enforces structurally (§5.1): the edge initiates every connection
outbound; there is no inbound path. Its concrete endpoint/port values are
still customer-specific and EMPTY.

## 2. What is fixed by the design as-built (citable, not site-specific)

These are structural properties of the Phase 6 edge-agent code, cited as
the baseline the site design must preserve — they hold for any site and
require no site data:

| Property | Evidence (real seams) |
|---|---|
| The edge agent contains NO listen/bind/accept surface; its only socket operation is an outbound `TcpStream::connect_timeout` in the uplink transport | `crates/edge-agent/src/uplink_mtls.rs` (`RustlsUplinkTransport::new`, `MtlsUplinkChannel::compose`); enforced by the lexical-scan test `crates/edge-agent/tests/no_inbound_listener.rs` (refuses `TcpListener`, `UdpSocket`, `::bind(`, `.listen(`, `.accept(` tokens) |
| Connection is outbound-initiated edge → central platform; the central platform never dials into the site; response-direction frames arrive ON the outbound session | §3.3; `docs/security/edge-ot-boundary.md` §2; P6.07 record |
| mTLS both directions; scheme `wss://`/`https://` required, `ws://`/`http://` refused; minimum version TLS 1.3 by DEFAULT (`tls12-floor` opt-in per signed site D6 record only) | `crates/edge-agent/src/uplink_mtls.rs` (`TlsMinimum`, `MtlsUplinkConfig::validate`); `edge-ot-boundary.md` §3 |
| Network allowlist is exactly: (a) the control-plane endpoint, (b) the site-EMS endpoint from adapter configuration — anything else is absent by construction | `edge-ot-boundary.md` §2 (P6.20 §2); `MtlsUplinkConfig::dial_target` |
| Command path disabled by default; even enabled, it creates NO inbound listener | `crates/edge-agent/src/command_path.rs` (`CommandPathRuntime`); `edge-ot-boundary.md` §5 |
| Offline behaviour: bounded store-and-forward, audited drop-of-oldest, at-least-once commit; no new commands while disconnected | `crates/edge-agent/src/buffer_file.rs` (`FileLocalBuffer`), `crates/edge-agent/src/audit_buffer.rs` (`FileAuditBuffer`); P6.18 offline-behaviour statement (§2 of `18_21_audit_offline_isolation.md`) |

## 3. Zone diagram skeleton (§17.2) — zones named, membership EMPTY

The report's five-zone model is fixed; which customer equipment sits in
which zone is site-specific and EMPTY. All flow arrows are keyed to the §4
flow table; every flow's protocol/port/direction/initiator cells are EMPTY
except F1's structural direction, fixed by §2 above.

```text
Zone 1  Enterprise IT            (browser, identity, admin)      membership: <EMPTY>
Zone 2  Central application      (control plane, DB, broker,     membership: <EMPTY>
                                  plugin host, workers)
Zone 3  Customer integration/DMZ (FlexCore edge agent, approved  membership: <EMPTY>
                                  gateways)
Zone 4  OT/site control          (site EMS, SCADA, PLCs, local   membership: <EMPTY>
                                  controllers)
Zone 5  Field/device             (inverters, batteries, EVSE,    membership: <EMPTY>
                                  meters, relays)
```

Flow skeleton (labels used in §4):

```text
[Zone 1] --(operator browser, IdP)--> [Zone 2]
[Zone 2] <--(F1: OUTBOUND-INITIATED mTLS session)== [Zone 3 edge agent]   direction FIXED, endpoint EMPTY
[Zone 3 edge agent] --(F2: EMS adapter hop)--> [Zone 4 site EMS]
[Zone 4 site EMS] --(F3: device/gateway protocol)--> [Zone 5]
[Zone 2] --(F4: market/program, ONLY if D1 closes)--> external market
```

Rule pre-printed: only documented flows cross zone boundaries; no
undocumented flow exists between zones (acceptance criterion (c)) — any
flow discovered later is added by a design change with a customer review,
never by an untracked firewall exception.

## 4. Flow table — ALL site cells EMPTY

One table per site. Every flow names protocol, port(s), direction,
initiator, and the firewall/allowlist rule that implements it (or a tracked
action to create it — the ledger's verification method). Initiator
vocabulary: the side that opens the connection (§3.3 requires the edge for
F1; central never initiates).

| Flow ID | From zone → to zone | Purpose | Protocol | Port(s) | Initiator | Firewall rule reference | EXAMPLE? |
|---|---|---|---|---|---|---|---|
| F1 | Zone 3 → Zone 2 | Edge uplink session (telemetry, commands, health) | mTLS (`wss://`/`https://`) | `<EMPTY>` | **edge (fixed)** | `<EMPTY>` | no |
| F2 | Zone 3 → Zone 4 | Edge → site EMS adapter hop | `<EMPTY — EMS-specific; NOTE: the OpenEMS REST hop is http:// today, a recorded open item>` | `<EMPTY>` | edge | `<EMPTY>` | no |
| F3 | Zone 4 → Zone 5 | EMS → device gateways / assets | `<EMPTY>` | `<EMPTY>` | site EMS | `<EMPTY>` (customer-owned) | no |
| F4 | Zone 2 → external market | Market/program participation | `<EMPTY>` | `<EMPTY>` | control plane | `<EMPTY>` | no |

EXAMPLE ONLY — ILLUSTRATIVE SHAPE, NOT AN ALLOCATION (this row shows the
grain a filled table must reach; every value in it is invented for
illustration and MUST be replaced by customer-authored values):

| Flow ID | From → to | Protocol | Port(s) | Initiator |
|---|---|---|---|---|
| F1 (EXAMPLE) | DMZ edge host → central platform | mTLS WSS | EXAMPLE `tcp/443` | edge |

No EXAMPLE row may survive into a filled design; a filled table that
inherits an EXAMPLE value is rejected in review (§10, check 5).

## 5. Connectivity pattern — options and the EMPTY selection

Report §3.3: the site connection is initiated outward from the customer
edge to the central platform, **or through a customer-controlled
broker/gateway** where the customer requires it; customer security
architecture may require a DMZ, unidirectional gateway, jump host, or other
approved pattern. Which pattern the pilot site requires is a site-specific
reviewer question (P6.03 open item 1 / F4; `edge-ot-boundary.md` §9 item 1)
— selection EMPTY.

| Option | What it requires | Design fit with the built edge |
|---|---|---|
| 5.1 Direct outbound edge → central (DEFAULT/RECOMMENDATION) | One outbound allowlist rule from the DMZ edge host to the central platform endpoint | Exact match to the built transport (§2) |
| 5.2 Customer-controlled broker/gateway in between | Broker reachable from the edge; the edge dials the broker, the broker forwards | Compatible: the edge still initiates outbound; dial target becomes the broker endpoint (`MtlsUplinkConfig::dial_target` is configuration) |
| 5.3 DMZ two-leg pattern (edge in Zone 3; separate legs to Zone 2 and Zone 4) | Two allowlisted legs, both outbound-initiated from the edge host | Matches the report topology and the two-endpoint allowlist shape |
| 5.4 Unidirectional gateway | A data diode/gateway on the telemetry path; the command path would be structurally impossible on a strictly one-way link | Compatible with the DEFAULT read-only posture (command path disabled); a later command-enablement design must reconcile with the diode — recorded as a condition, not assumed solvable |
| 5.5 Jump-host / vendor remote-access pattern | Customer-controlled bastion for any interactive access | Orthogonal to the data path: FlexCore requires no shell access to the edge (§18.3: no shell access for plugins; the agent ships no listener); the site's bastion rules govern any human maintenance access |

Selection cell: `<EMPTY — chosen by the customer network/OT owner at the
design review>`; any option other than 5.1 is recorded with its conditions
and feeds P9.06 as a boundary decision.

## 6. Allowlist structure — explicit, per flow, deny-by-default

Pre-printed rules (not fillable, §17.2/§3.3/§18.3):

1. Explicit source/destination/port per flow; deny-by-default everywhere.
2. **No inbound internet access into any OT network** — no flow's initiator
   may be the internet or the central platform dialing in (acceptance
   criterion (b)); F1's initiator is the edge (§2).
3. **No direct database, NATS, or arbitrary plugin network access from any
   field-device network** (§17.2 verbatim rule); Zone 5 devices reach only
   their gateways/EMS.
4. The edge host's outbound set is exactly the two allowlisted endpoints
   (control plane; site EMS) — absent by construction otherwise.
5. No "temporary" rule: every rule carries an owner and a review date; a
   temporary rule without an expiry is the ledger's named failure mode.

| Rule ID | Implements flow | Source | Destination | Port | Protocol | Action | Owner | Expiry/review date |
|---|---|---|---|---|---|---|---|---|
| `<EMPTY>` | F1 | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | mTLS | allow | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | F2 | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | allow | `<EMPTY>` | `<EMPTY>` |
| (deny rows for the pre-printed prohibitions) | — | Zone 5 | DB/NATS/plugin surfaces | any | any | **deny** | customer network/OT owner | standing |

## 7. Resilience and buffering budgets — assumptions EMPTY, bound bounded

- **Link loss behaviour (fixed by design):** the edge buffers telemetry
  locally in the bounded store-and-forward journal (`FileLocalBuffer`:
  record-bound capacity, audited drop-of-oldest, in-order at-least-once
  upload on reconnect; `FileAuditBuffer` mirrors it for audit records) and
  issues NO new commands while disconnected — the command channel is the
  outbound session itself, which is down (P6.18 offline-behaviour
  statement). The site EMS remains authoritative for fast local safety
  behaviour; a central outage does not disable site-local protection
  (§21.2, §17.5 invariant 10).
- **Bounded offline-buffer size:** the buffer is bounded by construction
  (capacity asserted `> 0` at open); the concrete bound for the pilot is a
  deployment configuration/`RetentionPolicy` decision — cell EMPTY. The
  ledger's failure mode "buffering unbounded so a long outage exhausts
  local storage (§18.3)" is prevented structurally (never unbounded) and
  numerically only when the human sets the bound; the sizing input is the
  outage duration × telemetry rate assumption below.
- **Bandwidth/latency budgets: RECORDED ASSUMPTIONS PENDING MEASUREMENT.**
  No bandwidth or latency number is asserted in this record — any number
  written before measurement would be an invented fact (AGENTS.md rule 1).
  The budget table exists so the human review can enter assumed values
  explicitly labelled as assumptions, and so P9.18 (telemetry freshness)
  and P9.39 (performance/load at pilot scale) replace them with measured
  values:

| Budget line | Assumed value (RECORDED ASSUMPTION — label mandatory) | Measured value (P9.18/P9.39) | Source of the measurement |
|---|---|---|---|
| Uplink telemetry steady-state rate | `<EMPTY ASSUMPTION>` | `<EMPTY>` | `<EMPTY>` |
| Uplink peak/replay-after-outage rate | `<EMPTY ASSUMPTION>` | `<EMPTY>` | `<EMPTY>` |
| Acceptable ingest delay / freshness limit per use case | `<EMPTY ASSUMPTION>` | `<EMPTY>` | `<EMPTY>` |
| Acceptable round-trip for command ack (operator-approved mode) | `<EMPTY ASSUMPTION>` | `<EMPTY>` | `<EMPTY>` |
| Maximum supported outage duration at the chosen buffer bound | `<EMPTY ASSUMPTION>` | `<EMPTY>` | `<EMPTY>` |

- **Encryption at rest for edge journals (§18.3):** recorded DEPLOYMENT
  REQUIREMENT, not implemented (P6.18 record, decision D-8) — the network
  design must treat the edge journal path as in-scope for OS-level site
  protection per the D6 review; carried open.

## 8. Segmentation requirements handed to P9.06

This design is P9.06's input. What it hands over, per §17.2:

| # | Requirement the review must decide on | Where defined here |
|---|---|---|
| S1 | Zone membership for the actual site (does the site's DMZ match the report topology; EMS inside OT with the edge as the only crossing point — P6.03 Z2/Z3) | §3 |
| S2 | Per-flow approval: approved / approved-with-condition / rejected, for every §4 row | §4 |
| S3 | Connectivity pattern (§5 options; unidirectional-gateway condition) | §5 |
| S4 | The deny-by-default rule set incl. standing denies | §6 |
| S5 | TLS minimum/cipher position for the uplink (DEFAULT TLS 1.3; reviewer may pin otherwise per site — configuration, never a code change) | §2; `edge-ot-boundary.md` §3 |
| S6 | Buffer bound + outage-duration assumption as a reviewed safety input | §7 |

## 9. What this design does NOT establish

- It does not configure any network: no firewall was touched, no rule
  created, no endpoint contacted.
- It does not prove the flows work (P9.14 lab integration owns the first
  end-to-end evidence; D6 must close before any read-only pilot activity).
- It does not measure any bandwidth/latency value (P9.18/P9.39).
- It does not approve itself: acceptance criterion (d) requires the
  customer network/OT owner's review — NOT OBTAINED, UNSIGNED.
- It does not close D6 (P9.06 owns the review; closure is a human
  decision).

## 10. Reviewer verification procedure

1. Confirm §1's plain statement is unchanged and no real IP/VLAN/hostname/
   port is asserted anywhere outside an EXAMPLE-labelled row.
2. Confirm every §3 membership, §4 site cell, §5 selection, §6 rule cell,
   and §7 budget cell is `<EMPTY>`, and each EXAMPLE row is labelled.
3. Walk the flow skeleton flow-by-flow against §17.2 and §3.3: confirm F1's
   initiator is the edge, no flow is inbound from the internet into OT, and
   the §6 standing denies cover DB/NATS/plugin access from field networks.
4. Trace each §2 property to the named source file/test; confirm
   `no_inbound_listener.rs` exists.
5. Confirm no EXAMPLE value survived into any non-EXAMPLE cell, and no
   §6 rule lacks an owner or expiry column entry in a (future) filled
   design.
6. Confirm §9/§8: the record claims neither configuration, measurement,
   nor customer review; the P9.06 handover table is intact.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test".

## 11. Filling commands (for the human executor)

- Fill §3 membership and §4 flows WITH the customer network/OT owner from
  their actual architecture and firewall policy; every flow gets a matching
  firewall rule or a tracked action with an owner and date.
- Select the §5 pattern; record conditions; enter §6 rules with owners and
  expiry/review dates; delete every EXAMPLE row.
- Enter §7 assumptions labelled RECORDED ASSUMPTION; set the buffer bound;
  file the design + zone/flow diagram as the P9.05 evidence named in
  `CHECKLIST.md` ("Site network design approved") only after the customer
  network/OT owner review is recorded in §12.
- Hand §8 to the P9.06 review; the review verdicts feed back as required
  changes to this design (re-issue with a revision note).

## 12. Customer network/OT owner review record — NOT REVIEWED

| Field | Value |
|---|---|
| Reviewer (customer network/OT owner, named) | `<EMPTY>` |
| Review date | `<EMPTY>` |
| Outcome (approved / approved-with-conditions / changes required) | **NOT RECORDED** |
| Conditions / required changes | `<EMPTY>` |
| Signature | **UNSIGNED** |

## 13. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No real site network exists; the design cannot be reviewed or approved | §1, §12 |
| P9.04 is a template (no adapter inventory) — F2/F3 protocol cells cannot be filled yet | Inputs |
| A "temporary" inbound rule or undocumented vendor remote-access path appearing at commissioning | ledger P9.05 risk row; §6 rule 5 |
| Edge buffer sizing done from unmeasured assumptions; a long outage exhausts storage if the bound is set wrong | §7; ledger P9.05 risk row |
| Unidirectional-gateway pattern (if selected) structurally conflicts with any future command path | §5 option 5.4 condition |
| OpenEMS REST hop is `http://` today — a cleartext leg inside the OT boundary until TLS closes it (recorded open item, P6.10/P6.11) | §4 F2 note; `edge-ot-boundary.md` §9 item 5 |
| Encryption at rest for edge journals not implemented — deployment requirement until a license-audited crypto dependency closes it | §7; P6.18 D-8 |

Status: **WRITTEN-UNRUN** — a network design template cannot be "run"; the
only executable steps (customer architecture review, firewall-rule creation,
lab flow verification under P9.14) have not happened and are not scheduled.
No network was configured; no pilot exists and none is claimed.
