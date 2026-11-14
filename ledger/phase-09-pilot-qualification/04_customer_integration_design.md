<!-- P9.04 — Customer-specific integration design (decision record). Design TEMPLATE only: every customer-specific value EMPTY. No customer exists; nothing installed or connected; no adapter claimed conformant. -->
# P9.04 — Customer-Specific Integration Design (Design Template)

Date: 2026-11-14. Subtask: P9.04 (`ledger_phase_9.md` §P9.04).
Owner role: Integration engineering (with power systems for capability mapping).
Report trace: §23 Stage 9; §2.3; §5.3; §25.2 (D5).
Inputs: P9.01 freeze (TEMPLATE, UNSIGNED — `01_pilot_scope_freeze.md`);
P9.02 reconciliation (PROCEDURE, UNRUN — `02_site_asset_reconciliation.md`);
D5 decision position (OPEN — §2 below); vendor documentation for the site
EMS and each in-scope device/protocol — **none of which exist yet**.

## 1. Honest status — the plain statement

**No customer exists, so no customer-specific integration design exists and
none is claimed.** This record delivers the integration-design TEMPLATE that
the real per-site design document must fill: the integration-point inventory
(§4), the conformance obligations (§5), the read/write posture (§6), the
unit/sign/status mapping tables (§7), the failure-behaviour tables (§8), the
scheduling/market interface rows (§9), and the exclusion list (§10). Every
customer-identifying cell — adapter versions, profiles, endpoints, firmware
levels, unit mappings, quality-state selections — is EMPTY or
placeholder-named. Nothing was installed, connected, probed, or configured;
an integration design cannot be produced for a customer that does not exist
(ENTRY_NOTE boundary 1).

The ledger's acceptance criteria for P9.04 are written for a real design
review. They are NOT met by this template and are not claimed: criterion (a)
"every integration point has a named adapter, version, and profile" cannot
be satisfied while every version/profile cell is EMPTY; criterion (c)
"unit/sign mappings are recorded per device profile" cannot be satisfied
while the reactive-power sign declaration is EMPTY. The template's job is to
make those criteria mechanically checkable when a human fills it — not to
simulate their satisfaction.

## 2. D5 position — carried OPEN with a RECOMMENDATION

D5 ("first protocol and asset mix"; gate "before v0.4.0"; owner
Product + integration) is **SCHEDULED/OPEN** — `.agent/DECISIONS.md`, which
this record does not edit. The report's recommended baseline rests on the
§2.3 integration table: OpenEMS as the priority site-EMS integration for
lab/demo and selected pilot sites; SunSpec Modbus read/telemetry first;
OpenADR 3.0, OCPP 2.0.1/2.1, IEEE 2030.5/CSIP as later or planned adapters
(the last with regional profile and certification planning, report [R27]);
OpenADR certification "separate and must not be claimed from protocol
parsing alone" (report [R29]).

| Field | Status |
|---|---|
| D5 status | **OPEN** — agent-undecided per ENTRY_NOTE boundary 2; "OpenEMS priority" is a RECOMMENDATION, not a decision (`.agent/DECISIONS.md` gate table) |
| RECOMMENDATION (DEFAULT label) | The pilot integration set STARTS from the §2.3 initial-posture table (OpenEMS site integration; read/telemetry-first posture elsewhere) and narrows to the customer's actual fleet only from the signed P9.01 freeze §3.C asset inventory |
| What would change it | The customer's actual EMS/asset/protocol mix (vendor documentation, fleet confirmation of OCPP 2.0.1 vs 2.1, regional DER profile for IEEE 2030.5), or a human D5 decision recorded in `.agent/DECISIONS.md` + an ADR |
| Consequence while OPEN | Every adapter row below stays a TEMPLATE row; no adapter enters the pilot scope; P9.05/P9.06/P9.14 receive no adapter inventory from this record |

## 3. Scope rule — a row enters scope only from the signed freeze

The §2.3 table lists candidate integrations; it does not put any of them in
the pilot. An integration row in §4 becomes IN-SCOPE only when the signed
P9.01 freeze (§3.C asset inventory, §3.D service) names the site EMS, asset,
or market that requires it. A row that is never named by a signed freeze
stays out of scope, and no version/profile cell may be filled for it.
Conversely, an asset named by the freeze with no §4 row is a template defect
that the reviewer must reject (§11, check 3).

## 4. Integration-point inventory — ALL CELLS EMPTY

One table per site (`<SITE-ID-n>` from the signed freeze). Flow direction
uses the §3.2 vocabulary: telemetry in (device/EMS → FlexCore), command out
(FlexCore → device/EMS, only where §6 permits), market/program in/out
(D1-dependent, §9). Adapter categories follow report §5.3.

| Row | Site ID | Integration point | Adapter category (§5.3) | Protocol | Protocol version | Profile / variant | Flow direction | In scope? (freeze-named) |
|---|---|---|---|---|---|---|---|---|
| 1 | `<SITE-ID-1>` | Site EMS | Site integration | OpenEMS | `<EMPTY>` | `<EMPTY>` | telemetry in / command out | `<EMPTY — YES/NO>` |
| 2 | `<SITE-ID-1>` | `<DEVICE-1>` (BESS / PV inverter / EVSE / meter) | Asset adapter / Protocol adapter | SunSpec Modbus | `<EMPTY>` | `<EMPTY — model + register map revision>` | telemetry in / `<EMPTY — write only under §6>` | `<EMPTY>` |
| 3 | `<SITE-ID-1>` | Demand-response program endpoint | Market/program adapter | OpenADR 3.0 | `<EMPTY>` | `<EMPTY — VEN/VEN-HTTP profile per program>` | market/program in/out | `<EMPTY>` |
| 4 | `<SITE-ID-1>` | EV charging management system | Protocol adapter | OCPP 2.0.1 or 2.1 | `<EMPTY — per target fleet and vendor support, confirmed with the fleet, never assumed>` | `<EMPTY>` | telemetry in / command out | `<EMPTY>` |
| 5 | `<SITE-ID-1>` | DER / smart-inverter communication | Protocol adapter | IEEE 2030.5 / CSIP | `<EMPTY>` | `<EMPTY — regional profile>` | telemetry in / command out | `<EMPTY>` |
| 6 | `<SITE-ID-1>` | Substation / enterprise exchange | Protocol adapter | IEC 61850 | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

Ledger risk restated as a template rule: the OCPP variant (2.0.1 vs 2.1) and
every vendor firmware/profile level are confirmed **with the customer's
fleet against installed versions**, never assumed from documentation; a
version filled from a datasheet without installed-firmware confirmation is
treated as UNVERIFIED (the P9.02 vendor-data rule extends here).

## 5. Conformance obligations (§2.3) — the non-negotiable row

Report §2.3, quoted: *"v0.1.0 ships simulation adapters only. Implementing
an adapter is not a conformance claim; each production adapter must pass the
relevant conformance or interoperability suite before it is described as
certified or qualified."*

Consequences for this design and every downstream subtask:

1. Every in-scope adapter row carries a conformance-suite column (below).
   The suite is named per adapter and executed in the pilot configuration
   only by **P9.37** — an execution this record cannot perform and does not
   schedule (ENTRY_NOTE boundary 6).
2. Until a P9.37 execution report exists for the pilot version, the adapter
   is described as **implemented, not conformant**. The words "certified",
   "conformant", or "qualified" may not appear about any adapter in any
   Phase 9 document without citing that report.
3. OpenADR certification (report [R29]) and IEEE 2030.5/CSIP certification
   planning (report [R27]) are external certification-track obligations —
   recorded as carried obligations, not as FlexCore claims.

| Row (refs §4) | Conformance / interoperability suite that applies | Suite named by | Executed? |
|---|---|---|---|
| 1 | `<EMPTY — named from primary sources only, per AGENTS.md rule 1>` | human design review | NO (P9.37 owns execution; UNRUN) |
| 2 | `<EMPTY>` | human design review | NO (P9.37) |
| 3 | `<EMPTY — certification is separate, report [R29]>` | human design review | NO (P9.37) |
| 4 | `<EMPTY>` | human design review | NO (P9.37) |
| 5 | `<EMPTY>` | human design review | NO (P9.37) |
| 6 | `<EMPTY>` | human design review | NO (P9.37) |

## 6. Read/write posture — pre-printed ceiling (§2.3; §17.5; P6.14)

The posture is boilerplate derived from report §2.3 and the built edge
design; it is NOT fillable and no customer may waive it:

- **Read/telemetry first**: every adapter's default posture is
  read/telemetry only.
- **Write operations only in explicit test and qualified profiles**
  (§2.3): command-out flow exists for an adapter only if the adapter is on
  the per-site command allowlist AND the P6.14 command-path gate is enabled
  under its four signed, time-bounded conditions — including the condition
  that the site's D6 review record exists (condition d). Design seams:
  `crates/edge-agent/src/command_path.rs` (`CommandPathRuntime::evaluate`,
  `enablement_at`, fail-closed `GateRefusal` vocabulary) and
  `crates/edge-agent/src/policy.rs` (`SitePolicyEngine::admit_command`,
  `is_adapter_enabled` — the per-site/per-adapter allowlist).
- Per-admission, every command is still verified before forwarding:
  signature, expiry against a `Synchronized` clock only, monotonic
  sequence, scope (`crates/edge-agent/src/command_verify.rs`,
  `CommandAuthorization::validate`, `CommandVerificationRejection` codes).
- No adapter is "write-capable by default"; enabling write for an adapter
  that was qualified for reads is the ledger's named failure mode and is
  structurally refused (`adapter_not_allowlisted`).

The §4 "command out" direction cell may be filled YES only when the above
chain exists for that adapter in the signed control-scope document (P9.26).
For the pilot's at-most control ceiling, see P9.01 §3.F: read-only/shadow
first, operator-approved control at most, bounded automation excluded.

## 7. Unit / sign / status mapping — per device profile, ALL CELLS EMPTY

Report §10.2 and §6.4 (anti-corruption layer) govern: canonical internal
units are MW, MVAr, MVA, MWh; voltage kV at the nominal base (per-unit in
solver representation, base declared); current A or per-unit on an explicit
base; frequency Hz; SOC a fraction in [0,1] at the API boundary (percentages
converted at the adapter, display-only); **P > 0 = export/injection at the
point of connection; P < 0 = import/consumption**. Reactive-power sign is
NOT universal: **each grid model and device profile declares whether Q > 0
means injection (generator convention) or consumption (load convention)**
— the declaration cell below is the point of that rule and stays EMPTY
until declared per device profile, never assumed.

One mapping row set per device profile (`<SITE-ID-n>` / `<ASSET-ID-n>` /
`<DEVICE-PROFILE-n>` from P9.02):

| Vendor / protocol field | Vendor unit / encoding | Canonical field | Canonical unit | Conversion rule | Sign mapping applied | Declared by |
|---|---|---|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | active power | MW | `<EMPTY>` | P>0 export at PCC (fixed convention) | §10.2 (fixed) |
| `<EMPTY>` | `<EMPTY>` | reactive power | MVAr | `<EMPTY>` | **`<EMPTY — Q>0 injection or consumption, DECLARED per device profile>`** | `<EMPTY — grid model + device profile owner>` |
| `<EMPTY>` | `<EMPTY>` | apparent power | MVA | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | energy | MWh | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | SOC | fraction [0,1] | `<EMPTY — e.g. percent→fraction>` | n/a | §10.2 (fixed) |
| `<EMPTY>` | `<EMPTY>` | voltage | kV nominal base / per-unit (base declared) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| `<EMPTY>` | `<EMPTY>` | status codes | canonical state set | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

Vendor status-code mapping table (anti-corruption layer, §6.4 — external
terms/units/timestamps/status codes converted at the boundary, never leaked
into the domain):

| Vendor status code | Meaning per vendor documentation | Canonical representation | Retained as diagnostic metadata? |
|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 8. Failure behaviour per adapter — ALL SELECTIONS EMPTY

The six canonical telemetry quality states are fixed (§10.4): `GOOD`,
`SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE` — exactly one per value;
richer source states map to exactly one of the six at the adapter boundary
with the original source code retained as diagnostic metadata; a quality
flag is never silently converted into a normal reading; every use case
defines which states it accepts. The candidate mappings below are
pre-printed from §10.4; the selection per adapter (and the freshness limit
that separates `STALE` from `MISSING`) is per use case and stays EMPTY
until the customer's use cases are frozen (P9.01 §3.D) and measured
(P9.18).

| Failure mode (ledger step 5) | Candidate canonical state(s) | Selection for row 1 (OpenEMS) | Selection for row 2 (SunSpec) | … per adapter |
|---|---|---|---|---|
| Timeout (no response within declared window) | `MISSING` or `STALE` (per freshness limit) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Partial response (some points missing in a frame) | per-point: `MISSING`; frame-level plausibility failure: `SUSPECT` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Stale data (age exceeds use-case freshness limit) | `STALE` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Source reports value invalid | `BAD` (source code retained) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Plausibility/range/consistency check fails | `SUSPECT` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Value substituted/interpolated (if any) | `ESTIMATED` — never consumed as measured | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

Binding rules (not fillable): a value is never upgraded from a worse state
to `GOOD` by adapter code; `ESTIMATED` values are never inputs to a passing
feasibility check (§17.5 invariant 9); the stale/missing hold-reject
behaviour is verified end-to-end by P9.18, not by this design.

## 9. Scheduling / market interfaces — D1 OPEN, all cells EMPTY

D1 (target jurisdiction and market) is **OPEN** (`.agent/DECISIONS.md`;
P9.01 §3.H). Therefore no market/program interface can be designed: which
ISO/RTO or demand-response program — if any — the pilot participates in is
unknown, and §2.3 requires region-specific market adapters ("not a generic
OpenADR mapping"; FERC Order 2222 is mediated per-RTO/ISO, report [R1]).

| Interface | Row in §4 | Status |
|---|---|---|
| Market registration / bids / dispatch / telemetry / settlement | 3 (or a region-specific adapter row) | **BLOCKED ON D1** — design cell EMPTY; the participation-or-not record is P9.11's deliverable, from program documentation only |
| Demand-response program events | 3 | BLOCKED ON D1 — same |
| Day-ahead / intraday scheduling cycle (internal, no market) | n/a | Internal workflow only; requires no external interface; its operational procedure is P9.22 |

If the human signatories record "no market participation" (P9.11), rows 3/9
are struck from scope with that citation; they are never filled from a
neighbouring region's rules (ledger P9.11 risk).

## 10. Deliberate exclusions — to be completed by the human design review

The template pre-prints the structural exclusions; the customer-specific
list ("what is deliberately not integrated in this pilot", ledger step 6)
is filled by the human review against the signed freeze:

- Structural (not fillable, report §0.3/§1.6/§17.5 and AGENTS.md §3):
  protection, primary frequency response, synthetic inertia, relay
  protection, inverter inner-loop control, sub-cycle control, grid
  switching, hard-real-time control. FlexCore is supervisory orchestration;
  site-local safety/protection remain independent (§17.5 invariant 10).
- Customer-specific: `<EMPTY — e.g. an asset class present at the site but
  not enrolled; a protocol supported by the EMS but excluded from the pilot;
  a market program declined>` — one row per exclusion, each with a stated
  basis (freeze scope, capability gap, or customer decision):

| Excluded item | Basis | Recorded by |
|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 11. Reviewer verification procedure

1. Confirm §1's plain statement is unchanged and no "installed",
   "connected", or "integrated" claim exists anywhere in the record.
2. Confirm every §4/§7/§8 identity, version, profile, and selection cell is
   `<EMPTY>` and §4 row "In scope?" is `<EMPTY>` everywhere (no row entered
   scope without a signed freeze).
3. Traceability check: for a filled (future) design — every in-scope row
   names its adapter, protocol version, profile, and flow direction; every
   asset named by the signed freeze has a row; every row traces to vendor
   documentation (primary source, AGENTS.md rule 1) and a §5 suite row.
4. Confirm §5's §2.3 quote is verbatim and no "certified"/"conformant"
   wording about any adapter exists without a P9.37 citation.
5. Confirm §6 is intact (read-first ceiling, P6.14 gate chain, allowlist
   refusal) and no row claims write capability outside it.
6. Confirm the §7 reactive-power sign cell is EMPTY (never assumed) and the
   §10.2 fixed conventions are quoted correctly (MW/MVAr/MVA/MWh; P>0
   export; SOC fraction [0,1]).
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the template is written; no design review has convened.

## 12. Filling commands (for the human executor)

- Fill §4 per signed P9.01 freeze site/asset, from vendor documentation of
  the INSTALLED versions (fleet-confirmed, not datasheet-assumed); mark the
  In-scope column only from the freeze.
- Fill §5 suite names from the primary standards/program sources; P9.37
  owns execution — do not execute from this record.
- Fill §7/§8 per device profile during the design review with the customer;
  declare the reactive-power sign per grid model and device profile in
  writing (the declaration is the deliverable, not an inference).
- Fill §10 exclusions with the customer; file the completed document as the
  P9.04 evidence named in `CHECKLIST.md` ("Integration design complete").
- Downstream unblock order: this record feeds P9.05 (network design),
  P9.06 (boundary review), and P9.14 (lab integration) — they receive the
  adapter inventory only from the FILLED document.

## 13. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No real pilot customer exists; no integration design can be produced or reviewed | ENTRY_NOTE boundary 1; §1 |
| D5 OPEN — the adapter/protocol set rests on a RECOMMENDATION, not a decision | §2; `.agent/DECISIONS.md` |
| D1 OPEN — no market/program interface can be designed; a late D1 closure reopens this design | §9 |
| Vendor firmware/profile drift: documented profile ≠ installed version (e.g. OCPP variant assumed instead of fleet-confirmed) | ledger P9.04 risk row; §4 template rule |
| Write capability enabled by default in an adapter only qualified for reads — structurally refused in code, but a filled design could still mis-state posture | ledger P9.04 risk row; §6 |
| Conformance suites never executed (P9.37 UNRUN): no adapter may be called conformant for the whole pilot period | §5 |
| Reactive-power sign undeclared per device profile — a filled design that omits the §7 declaration would feed wrong Q into feasibility | §7; §10.2 |

Status: **WRITTEN-UNRUN** — a design template cannot be "run"; the only
executable steps (design review with the customer, vendor-documentation
trace, suite execution under P9.37) have not happened and are not
scheduled. Nothing was installed or connected at any site; no pilot exists
and none is claimed.
