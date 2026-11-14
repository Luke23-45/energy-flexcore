<!-- P9.01 — Pilot scope, customer, and site freeze (decision record). Template only: every identity/value cell EMPTY or placeholder-named. No pilot exists. -->
# P9.01 — Pilot Scope, Customer, and Site Freeze (Freeze Template)

Date: 2026-11-14. Subtask: P9.01 (`ledger_phase_9.md` §P9.01).
Owner role: Product (with legal for the customer agreement). Report trace:
§23 Stage 9; §25.2 (D1, D2); §2.1 (v1.0.0 definition).
Inputs: D1, D2, D3 positions (all OPEN — see §5); the Stage 1–8 NOT-HELD
exception records (§2); the Phase 8 exit record
`ledger/phase-08-advanced-coordination/30_retrospective_exit_gate.md`;
`ledger/phase-09-pilot-qualification/ENTRY_NOTE.md` boundaries 1–3.

## 1. Honest status — the plain statement

**No pilot exists. There is no customer, no site, no signed agreement, and
none is claimed.** This record delivers the freeze TEMPLATE that a real
`pilot-scope-v1.0` document must fill — every customer-identifying,
site-identifying, and asset-identifying cell is EMPTY or placeholder-named
(`<CUSTOMER-LEGAL-NAME>`, `<SITE-ID-1>`, …). Per the ledger's own guidance
(P9.01 risks) and ENTRY_NOTE boundary 1: *if a real customer does not exist
yet, that fact is recorded as an open risk in P9.27, not as a decision.*
The freeze sign-off is a HUMAN gate; this template is UNSIGNED.

Nothing here was executed and nothing here could be: freezing a scope
requires a customer agreement or letter of intent, which does not exist.

## 2. Entry basis — the Stage 1–8 NOT-HELD exceptions (recorded, not upgraded)

Per ENTRY_NOTE.md, Phase 9 starts on the recorded exceptions. P9.01 records
this honestly; the table restates the source records — it does not upgrade
any NOT HELD into a pass, and a customer signature on the freeze below does
not upgrade them either.

| Stage | Gate status | Record |
|---|---|---|
| 1 | NOT HELD (P1.26 NOT RUN; P1.31 blocker recorded) | `ledger/phase-01-rust-foundation/26_exit_gate_record.md`, `31_exit_gate_review.md` |
| 2 | NOT HELD (P2.26 NOT RUN; P2.29 blocker recorded) | `ledger/phase-02-asset-telemetry/26_exit_gate_record.md`, `29_exit_gate_review.md` |
| 3 | NOT HELD | `ledger/phase-03-flexibility-scheduling/30_handover_phase4.md` |
| 4 | NOT HELD (prepared) | `ledger/phase-04-grid-feasibility-dispatch/32_handover_phase5.md` §4 |
| 5 | NOT HELD (prepared, not demonstrated) | `ledger/phase-05-operator-console-simulator/30_stage5_exit_record.md` |
| 6 | NOT HELD (lab run UNRUN) | `ledger/phase-06-edge-protocol/24_exit_gate_lab_run_record.md`, `30_retrospective_release_gate.md` |
| 7 | NOT HELD (witnessed run UNRUN) | `ledger/phase-07-plugin-sdk/28_exit_gate_run_record.md`, `30_retrospective_release_gate.md` |
| 8 | NOT HELD (prepared, not demonstrated) | `ledger/phase-08-advanced-coordination/30_retrospective_exit_gate.md` §5 |

Consequence for the freeze: the human signatories acknowledge the freeze
**with** the Stage 1–8 exception statement in §2.2 of the template, not on
top of a demonstrated Stage 1–8 baseline.

## 3. The `pilot-scope-v1.0` freeze template

Filling rule: every `<…>` cell is filled by a human from a primary source
(customer agreement, site records, nameplates) and the filled document is
signed per §3.I. A template with any `<…>` cell still unfilled is NOT a
frozen scope and blocks all P9.02–P9.30 work.

### 3.A Customer identity — ALL CELLS EMPTY

| Field | Value |
|---|---|
| Customer legal name | `<CUSTOMER-LEGAL-NAME>` |
| Registration / jurisdiction | `<EMPTY>` |
| Contract or letter-of-intent reference | `<EMPTY>` |
| Customer signatory (name, role) | `<EMPTY>` |
| Customer technical contact (P9.02 interface) | `<EMPTY>` |

### 3.B Site identity — ALL CELLS EMPTY (§10.1 site IDs; no "and similar sites")

| Site ID (§10.1) | Portfolio ID | Location | Timezone | PCC reference | Grid-model reference |
|---|---|---|---|---|---|
| `<SITE-ID-1>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` (P9.03 output) |

Additional sites: add rows only by re-issuing the freeze (scope creep rule,
§4). A site that is not a named row is out of scope.

### 3.C Asset inventory per site — ALL CELLS EMPTY

Canonical units throughout: rated power in MW (or MVAr where the asset is
a reactive resource), rated energy in MWh, apparent power in MVA; SOC as a
fraction in [0,1] at the API boundary (§10.2). `P > 0` = export at the
point of connection.

| Site ID | Asset ID (§10.1) | Asset type | Vendor / model | Rated power (MW) | Rated energy (MWh) | Phase | Bus reference | Capability profile ref |
|---|---|---|---|---|---|---|---|---|
| `<SITE-ID-1>` | `<ASSET-ID-1>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` (P9.02 output) |

### 3.D Service scope — UNSELECTED (exactly one §1.5 first-generation use case)

Tick exactly one; the freeze is invalid with zero or multiple selections.

- [ ] Portfolio visibility
- [ ] Flexibility estimation
- [ ] Day-ahead / intraday scheduling
- [ ] Peak management
- [ ] Demand response

Service value cell: `<EMPTY — one of the above to be ticked by the human signatories>`.
A use case outside the §1.5 first-generation list is out of scope for v1.0.0.

### 3.E Market / program — UNFILLED (P9.11 linkage)

| Field | Value |
|---|---|
| Market/program participation in scope? | `<EMPTY — YES/NO>` |
| If YES: market/program name, region, product | `<EMPTY>` (captured per P9.11 from program documentation, never assumed) |
| If NO: explicit "no market participation" record | `<EMPTY>` |

### 3.F Control-authority mode ceiling — pre-printed ceiling (§21.4), per-site mode cells EMPTY

Release-ladder position (AGENTS.md §4): v1.0.0 is pilot-ready with
operator-approved control BY DEFAULT; bounded automation is post-1.0, per
deployment, site, and asset class, after separate qualification. The pilot
ceiling is therefore **operator-approved control at most**, reached only by
the §21.4 ladder and §22.3 gates:

1. Read-only / shadow first (entry mode after commissioning, P9.13/P9.17).
2. Promotion to operator-approved mode per site, only after shadow
   acceptance (P9.17) and the §22.3 gate evidence.
3. **Bounded automation: EXCLUDED IN WRITING.** This clause is boilerplate
   that the signatories sign; it is not fillable:

> "The pilot scope excludes bounded automation in its entirety. No
> automation envelope exists in the pilot configuration (P9.26). Field
> write authority beyond individually operator-approved commands is not
> granted by this document at any point during the pilot."

Per-site current mode at freeze time: `<SITE-ID-1>: <EMPTY — simulation / read-only-shadow>`.

### 3.G Exclusions — pre-printed (§0.3, §1.6; AGENTS.md §3)

The following are out of scope for this pilot and are recorded as signed
exclusions, not as deferred features: protection; primary frequency
response; synthetic inertia; grid switching; hard-real-time control;
sub-cycle control; inverter inner-loop control; relay protection. FlexCore
is a supervisory orchestration layer; site-local protection and safety
remain with local certified controllers (§17.5 invariant 10).

### 3.H Decision positions at freeze time — ALL AGENT-UNDECIDED

| Decision | Status at freeze | Where recorded |
|---|---|---|
| D1 target jurisdiction and market | OPEN — interim position `<EMPTY>`, change condition `<EMPTY>` | `.agent/DECISIONS.md`; P9.11 linkage |
| D2 first portfolio and service | OPEN — interim position `<EMPTY>` (service tick in §3.D is the customer's, not a D2 closure) | `.agent/DECISIONS.md` |
| D3 network class and model fidelity | OPEN — RECOMMENDATION only, see `03_network_model_build.md` §2 | `.agent/DECISIONS.md`; P9.03 |

A freeze signed while a D-decision rests on an interim position is valid
only with that position and its change condition written into §3.H — the
residual risk goes to P9.27 (§6), never into a decision record.

### 3.I Sign-off — UNSIGNED

| Signatory | Role | Signature | Date |
|---|---|---|---|
| `<EMPTY>` | Customer authorized signatory | **UNSIGNED** | `<EMPTY>` |
| `<EMPTY>` | Neryva product ownership | **UNSIGNED** | `<EMPTY>` |

The freeze takes effect only on both signatures (acceptance criterion (d)).
No date is invented anywhere in this record.

## 4. Freeze procedure (the human steps)

1. Obtain the customer agreement or letter of intent; enter §3.A/§3.B from
   it. Never from a marketing description or an internal assumption.
2. Enter §3.C per site from the customer asset list (P9.02 reconciles it
   against the registry — a freeze whose asset list fails P9.02 is blocked,
   fail-closed, per `02_site_asset_reconciliation.md` §3).
3. Tick exactly one §1.5 use case (§3.D); complete §3.E with the P9.11
   capture or the explicit no-participation record.
4. Confirm §3.F mode ladder and sign the bounded-automation exclusion;
   confirm §3.G exclusions.
5. Enter D1/D2 interim positions with change conditions (§3.H) or close
   them by the human decision process first.
6. Both signatures (§3.I). File the signed document as the P9.01 evidence
   named in `CHECKLIST.md`.
7. Any widening (a second site, a new asset class, a service change)
   invalidates the freeze: re-issue as `pilot-scope-v1.1` with re-run of
   the affected P9.02–P9.30 subtasks. Mid-pilot widening without
   re-qualification is the ledger's named failure mode.

Acceptance criteria mapping (ledger): (a) every site/asset named by ID —
enforced by the no-`<…>`-cells rule and the no-"and similar sites" rule;
(b) service is a §1.5 use case — §3.D single-tick; (c) bounded automation
excluded in writing — §3.F clause; (d) both parties acknowledged — §3.I.

## 5. What this record does NOT do

- It does not select, name, or invent a customer, site, asset, or date.
- It does not close D1, D2, or D3 (agent-undecided per ENTRY_NOTE
  boundary 2); it only records where their positions will be written.
- It does not grant field write authority in any mode. Even the signed
  freeze grants nothing by itself: read-only/shadow entry additionally
  requires P9.06 D6 closure, and operator-approved mode requires the
  P9.17/P9.26 chain and §22.3 gates.
- It does not upgrade any Stage 1–8 NOT-HELD exception (§2).

## 6. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No real pilot customer exists yet; the freeze cannot be signed | ENTRY_NOTE boundary 1; ledger P9.01 guidance ("record as an open risk in P9.27, not as a decision") |
| D1 (jurisdiction/market) OPEN — the pilot scope rests on no recorded jurisdiction | §3.H; ledger P9.01 risk row |
| D2 (first portfolio and service) OPEN — the §3.D tick has no decision behind it | §3.H |
| D3 OPEN — the network class for the pilot model is a RECOMMENDATION only | `03_network_model_build.md` §2 |
| Stage 1–8 gates NOT HELD — the pilot qualifies a platform whose stage gates ran on recorded exceptions | §2 |
| Scope-creep exposure: any site/asset/service addition without re-issuing the freeze | §4 step 7 |

## 7. Reviewer verification procedure

1. Confirm every §3 identity/value cell is `<…>` EMPTY and every table
   row is a placeholder, not a name.
2. Confirm §3.D has zero ticks and §3.F/§3.G carry the mandated exclusion
   text verbatim.
3. Confirm §3.I reads UNSIGNED and no date field contains a value.
4. Trace the §2 stage rows to the eight named records; confirm none was
   upgraded.
5. Confirm the ledger tick for P9.01 says "implemented, awaiting human
   build/test" — never "tested", "verified", or "passed".

## 8. Filling commands (for the human signatories)

- Fill + sign: edit this template's §3 cells from primary sources, save the
  signed copy as `pilot-scope-v1.0` (PDF/markdown) beside this record, and
  reference it from `CHECKLIST.md`.
- Ledger tick (only after both signatures): `- [x]` on P9.01 in
  `ledger/phase-09-pilot-qualification/ledger_phase_9.md` is then justified
  by the acceptance criteria — the current `[x]` tick on this record means
  **implemented, awaiting human build/test**, i.e. the template exists; the
  freeze itself does not.
- Registry cross-check (next step): `02_site_asset_reconciliation.md`.

Status: **WRITTEN-UNRUN** — a template cannot be "run"; the only executable
step is the human filling and signing, which has not happened and is not
scheduled. No pilot exists yet and none is claimed.
