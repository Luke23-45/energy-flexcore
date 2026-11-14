<!-- P10.01 — Release scope freeze and supported-adapter matrix (decision record). FREEZE RECORD over the CURRENT state: no release candidate exists as a built artifact, no adapter is conformant, no VERIFIED cell is filled, and the freeze is UNSIGNED — signature, candidate build, and every VERIFIED column are human gates. Nothing was built, run, or installed. -->
# P10.01 — v1.0.0 Release Scope Freeze and Supported-Adapter Matrix

Date: 2026-11-14. Subtask: P10.01 (`ledger_phase_10.md` §P10.01).
Owner role: Product (with engineering). Report trace: §22.3 release gate 1
("documented scope and supported adapters are complete"); §2.1 (v1.0.0
definition); §2.2–§2.3 (asset/protocol boundary).
Inputs: Phase 9 exit-gate record P9.30
(`ledger/phase-09-pilot-qualification/30_phase_exit_gate_record.md`); the §2.1
release-ladder row for v1.0.0; the P9.04 integration design
(`ledger/phase-09-pilot-qualification/04_customer_integration_design.md`); the
P9.37 adapter-conformance record
(`ledger/phase-09-pilot-qualification/37_adapter_conformance_pilot.md`);
`ledger/phase-10-v1-release-hardening/ENTRY_NOTE.md` (boundaries 1, 3, 6).

## 1. Honest status — the plain statement

**This is a freeze RECORD, not a demonstrated freeze.** Nothing in this
repository was built, run, tested, or installed while writing it (the loop
bans were held), and:

- **No release candidate exists as a built artifact.** The candidate exists
  as the described build procedure in P10.18; the candidate identifier cell
  below is `<PENDING P10.18>` and is filled only when P10.18 records the
  gated commit, tag, build ID, and artifact hashes (ENTRY_NOTE boundary 1).
- **The freeze is UNSIGNED.** Step 4 of the subtask ("have product and
  engineering sign the scope freeze") is a human gate; the signature block in
  §7 is empty.
- **No adapter is conformant.** Every VERIFIED cell in the matrix (§5) is
  empty until a P9.37-class execution runs against the actual pilot adapter
  version and files its report (§2.3: "Implementing an adapter is not a
  conformance claim").
- **The Stage 1–9 gate exceptions are carried, not upgraded.** The v1.0.0
  control-status row depends on Stage 9 evidence that does NOT exist (§3).

Ticking P10.01 means "implemented, awaiting human build/test/execution" —
the same honest convention as every prior phase ledger.

## 2. The frozen v1.0.0 scope — the release-ladder row, verbatim

The v1.0.0 row of the §2.1 release-definition table, quoted verbatim (AGENTS.md
§3 rule 1 — no paraphrase of the product definition):

| Release | Product capability | Control status |
|---|---|---|
| **v1.0.0 — Pilot-ready FlexCore** | Stable operator workflow, reproducible benchmark, deployment/upgrade model, security review, site integration test harness | Operator-approved control is the default; production dispatch only after customer-specific acceptance and commissioning |

And the §21.4 control-authority row it lands in:

| Mode | Authority | Writes to devices | Default use |
|---|---|---|---|
| Operator-approved | Each command requires an authorized human approval | Yes, after approval | v1.0.0 default |

**The Stage 9 evidence dependency is recorded NOT MET.** "Operator-approved
control is the default" and "production dispatch only after customer-specific
acceptance and commissioning" both gate on the §23 Stage 9 exit — customer/site
and applicable-authority approval of a real deployment and its control scope.
P9.30 §5 records that gate as **NOT MET — prepared, not demonstrated**: no
customer, no site, no deployment, no executed commissioning chain, no executed
field acceptance. Nothing in Phase 10 re-runs the pilot qualification; P10.15
qualifies the mode **on paper at most** and records that the Stage 9 evidence
chain does not exist. Consequence, stated plainly: **this scope freeze does not
authorize operator-approved control at any site.** A v1.0.0 published on these
exceptions would ship the artifacts of a pilot-ready release WITHOUT the
qualification evidence the label claims; the go/no-go record (P10.27) must
either hold the release until Stage 9 re-entry conditions are met (P9.30 §5) or
waive them explicitly, signed, and time-bounded — never silently.

### 2.1 What v1.0.0 contains (the five §25 pilot-release capabilities)

| # | Capability (§25) | Phase 10 owner | Status at this freeze |
|---|---|---|---|
| 1 | Stable operator workflow | P10.23 (walkthrough), P10.14 (browser cells) | WRITTEN-UNRUN; console built in Phase 5, walkthrough UNHELD |
| 2 | Reproducible benchmark | P10.09–P10.10 | Cells EMPTY; no baseline run exists (P9.41 finding) |
| 3 | Deployment/upgrade model | P10.24 (draft), P10.11–P10.13 (tests) | Templates prepared; candidate tests UNRUN |
| 4 | Security review | P10.25 (consolidation), P10.16–P10.17, P10.04, P10.08, P10.20 | Consolidation records; assessment verdict cells EMPTY (P9.07) |
| 5 | Site integration test harness | P10.22 | Content = P9.14/P9.16–P9.20 procedures (UNRUN); packaging pending |

### 2.2 What v1.0.0 does NOT contain (excluded by the §2.1 ladder and §0.3)

- **Bounded automation** — post-1.0, per deployment, site, and asset class,
  after separate qualification (§21.4); not built, enabled, or qualified. No
  automation envelope exists in the release configuration (P10.15 §6, P10.30).
- **Advanced coordination / grid intelligence** — the v1.x and v2.x rows
  (DMPC/ADMM, GNN, safe MARL, market analytics) are outside this release.
- **Certified protocol conformance** — from adapter implementation alone
  (§2.3 closing rule; the matrix in §5 enforces this cell by cell).
- **Hard-real-time, protection-grade, or safety-case claims** — §0.3, §14.5,
  AGENTS.md §3 rules 2–3. FlexCore is a supervisory orchestration layer.
- **Universal market qualification or FERC Order 2222 compliance** (§2.3,
  ADR-011).
- **Any write authority on the 0.x line** — the entire 0.x line remains
  simulation-only; field write authority begins at v1.0.0 WITH operator
  approval, which is not qualified here (§3 below).

## 3. Entry basis — the Stage 1–9 exceptions (recorded, not upgraded)

Per ENTRY_NOTE.md, Phase 10 starts on the recorded exceptions; this freeze
records them honestly. The Stage 1–8 rows are carried from P9.01 §2 (which
carried them from the phase records); Stage 9 is the P9.30 exit gate.

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
| 9 | NOT MET (prepared, not demonstrated — no customer/site/deployment exists) | `ledger/phase-09-pilot-qualification/30_phase_exit_gate_record.md` §5 |

Consequence for the freeze: the human signatories (§7) acknowledge the freeze
**with** this exception table, not on top of a demonstrated Stage 1–9 baseline.
A signature does not upgrade any NOT HELD row into a pass.

## 4. Asset and protocol boundary (§2.2–§2.3)

- **Asset boundary (§2.2):** the initial support table (BESS, PV inverter,
  flexible C&I load, EVSE/EV fleet, site meter; generic DER capability
  interface for wind / thermal storage / heat pumps; backup generation NOT a
  first-class target) describes the TARGET initial capability. No pilot asset
  inventory exists to freeze — the P9.01 §3.C inventory is ALL CELLS EMPTY,
  and the freeze of the actual fleet belongs to the filled, signed P9.01
  freeze, not to this record.
- **Protocol boundary (§2.3):** the §2.3 integration table (OpenEMS; OpenADR
  3.0; SunSpec Modbus; OCPP 2.0.1/2.1; IEEE 2030.5/CSIP; IEC 61850;
  ISO/RTO-specific APIs) with its initial postures is carried into §5 as the
  adapter matrix skeleton. The §2.3 closing rule is binding on every row:
  *"v0.1.0 ships simulation adapters only. Implementing an adapter is not a
  conformance claim; each production adapter must pass the relevant
  conformance or interoperability suite before it is described as certified or
  qualified."*
- **Simulation adapters only, today.** The only adapter code that exists is
  the OpenEMS REST adapter and its FlexCore-side conformance suite
  (`crates/edge-agent/`, P6.10–P6.11, P6.19) — all WRITTEN-UNRUN against a
  fixture shape; the OpenEMS REST API was never executed when the adapter was
  written (`ledger/phase-06-edge-protocol/10_11_openems.md` §2).

## 5. Supported-adapter matrix — every VERIFIED cell EMPTY/UNVERIFIED

Filling rules (binding, from P9.04 §5–§6 and P9.37):

1. A row exists for an adapter ONLY if the signed P9.01 freeze names it in
   scope. Until then the rows below are TEMPLATE rows; "in scope" stays empty.
2. Adapter versions, profiles/variants, and firmware levels are filled from
   the pilot fleet's INSTALLED versions, confirmed with the customer — never
   from datasheets or from `RESEARCH.md` background (e.g. the 2026.11.0
   OpenEMS version there is background, not a pin).
3. The conformance column may say "verified" ONLY with a cited P9.37-class
   execution report naming `flexcore-edge-adapter-conformance-suite-v1`
   (`crates/edge-agent/tests/conformance_suite_p6_19.rs`, WRITTEN-UNRUN) run
   against the recorded adapter version. Until such a report exists, the
   column is `<UNVERIFIED>` and the words "certified", "conformant",
   "compliant", or "qualified" may not appear about the adapter anywhere.
4. OpenADR/OCPP/2030.5/61850 protocol suites exist for OTHER ecosystems and
   are later-adapter obligations (report [R27]/[R29]) — they do not substitute
   for the FlexCore-side suite.

| Row | Integration (§2.3) | Role | Adapter (code, if any) | Version | Profile / variant | Qualified against (P9.04/P9.15 cross-ref) | Conformance status |
|---|---|---|---|---|---|---|---|
| 1 | OpenEMS | Site-level EMS / edge integration | `crates/edge-agent` OpenEMS REST adapter (GET-only scan; P6.10–P6.11) | `<EMPTY — installed fleet version, never assumed>` | `<EMPTY>` | `<EMPTY — P9.04 §2 row 1 unfilled; P9.15 HIL UNRUN>` | `<UNVERIFIED — no P9.37 execution exists>` |
| 2 | OpenADR 3.0 | Demand-response program/event/report adapter | none | `<EMPTY>` | `<EMPTY — VEN profile per program>` | `<EMPTY — later adapter (§2.3)>` | `<UNVERIFIED — no adapter, no suite>` |
| 3 | SunSpec Modbus | DER/inverter/storage models over Modbus | none | `<EMPTY>` | `<EMPTY — model + register map revision>` | `<EMPTY>` | `<UNVERIFIED — read/telemetry-first posture (§2.3)>` |
| 4 | OCPP 2.0.1 / 2.1 | EV charging management | none | `<EMPTY — 2.0.1 vs 2.1 per fleet/vendor, confirmed, never assumed>` | `<EMPTY>` | `<EMPTY>` | `<UNVERIFIED — adapter planned, not delivered (§2.2)>` |
| 5 | IEEE 2030.5 / CSIP | DER / smart-inverter profile | none | `<EMPTY>` | `<EMPTY — regional profile>` | `<EMPTY — later adapter [R27]>` | `<UNVERIFIED>` |
| 6 | IEC 61850 | Substation / enterprise exchange | none | `<EMPTY>` | `<EMPTY>` | `<EMPTY — later enterprise integration (§2.3)>` | `<UNVERIFIED>` |
| 7 | ISO/RTO-specific APIs | Market registration/bids/dispatch | none | `<EMPTY>` | `<EMPTY — region-specific>` | `<EMPTY — D1-gated market capture (P9.11, all answers EMPTY)>` | `<UNVERIFIED>` |

Acceptance criterion (b) check: no row above describes any adapter as
certified or conformant, and no row CAN without a cited test result — the
column text itself carries the prohibition.

## 6. Release-candidate identifier and the drift procedure

### 6.1 The frozen candidate identifier

| Field | Value |
|---|---|
| Commit / tag | `<PENDING P10.18 — no candidate build exists>` |
| Build ID | `<PENDING P10.18>` |
| Artifact hashes | `<PENDING P10.18>` |
| Scope-freeze date | 2026-11-14 |

The scope of this freeze applies to the candidate P10.18 records. Any change
to the candidate after P10.18 records it **re-opens this subtask** (ledger
step 3): the affected matrix rows and scope statements are re-checked against
the new candidate, the re-freeze is re-signed, and the downstream verifications
(P10.02, P10.18, P10.26) re-run against the new identifier. While the cell
reads `<PENDING P10.18>`, this freeze is provisional by definition — a scope
written against a moving candidate is exactly the P10.01 risk row, so no
release-notes content (P10.26) may be derived from this document before the
identifier is filled.

### 6.2 Drift procedure — what changes the scope, and what happens

| Trigger | Classification | Required action |
|---|---|---|
| Candidate commit/build changes (P10.18 re-build) | Re-verify | Re-run affected verifications against the new identifier; re-sign if scope rows change |
| Adapter version/profile/firmware changes at the pilot | Scope change | Fill/re-fill the §5 row from installed-fleet confirmation; the prior conformance record (if any) is void for the new version — a P9.37-class re-run is required before any "verified" text |
| New adapter enters scope (or one is removed) | Re-freeze | Add/remove the §5 row with its qualification cross-reference; re-sign §7; update P10.26 release notes |
| Capability added/removed vs §2.1 (e.g. anything beyond the five §25 items) | Re-freeze | This document re-issued with a new freeze date and an explicit revision note; §2.2 exclusions re-affirmed; P10.27 gate evidence re-assembled |
| Control-status change (mode promoted at any site) | Out of scope of a document change | Requires §22.3 gates + site-specific acceptance (§21.4) + the Stage 9 chain; this freeze records it, never grants it |
| An adapter described as conformant anywhere without a cited suite result | Finding | Revert the text; trace to a P9.37-class execution or delete the claim (P9.37 §8 check 3) |

The drift procedure's fill commands are in §8; the reviewer procedure in §9.

## 7. Freeze signature — UNSIGNED (human gate)

| Role | Name | Signature | Date |
|---|---|---|---|
| Product owner | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Engineering owner | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

Signatures acknowledge §2 (scope verbatim), §3 (the Stage 1–9 exceptions —
especially the Stage 9 NOT MET dependency of the operator-approved default),
§5 (the adapter matrix with all VERIFIED cells empty), and §6 (the drift
procedure). A freeze with any §5 row claimed "verified" without a cited suite
report, or signed without the §3 table, is not a valid freeze.

## 8. Filling commands (human, at execution time)

```sh
# Candidate identifier (after P10.18 runs) — the exact values P10.18 records:
git rev-parse HEAD
git tag -l "v1.0.0*"

# Installed adapter versions — fill §5 from the pilot fleet, never from docs:
#   (per site) read the OpenEMS version from the running instance's REST
#   endpoint and the firmware levels from the device inventory; the command
#   form is P9.04 §2 and P9.37 §4 — every cell requires the installed value.

# Conformance evidence — only a P9.37-class execution fills a VERIFIED cell:
#   cargo test -p edge-agent --test conformance_suite_p6_19   # human gate
#   (the execution report names flexcore-edge-adapter-conformance-suite-v1
#    and the recorded adapter version; see P9.37 §7 command list)

# Claim sweep (no "certified/conformant" text without a cited result):
grep -rn "certified\|conformant\|compliant" docs/ ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md

# Repository verification after any edit to this record:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 9. Reviewer-verification procedure

1. **Verbatim check:** diff the §2 table against the §2.1 ladder row and the
   §21.4 mode row in
   `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` — the
   capability and control-status cells must be word-identical.
2. **Exception check:** trace every §3 row to the named record; confirm no
   row is described as met.
3. **Matrix check:** for each §5 row, confirm (a) version and profile cells
   are empty (not assumed from documentation), (b) the qualification
   cross-reference is empty or names a real P9.04/P9.15 artifact, (c) the
   conformance cell is `<UNVERIFIED>` — and search the repository
   (command in §8) for any "certified/conformant" text about adapters.
4. **Candidate check:** confirm the §6.1 cells read `<PENDING P10.18>` and
   that no release-notes or go/no-go text (P10.26/P10.27) cites this freeze
   as a demonstrated freeze.
5. **Signature check:** confirm §7 is unsigned; a signed copy must carry the
   §3 exception table.
6. **Status check:** this record is WRITTEN-UNRUN — nothing was built, run,
   or installed; the repository verification commands in §8 are the human's
   to run.

## 10. Open items — traced to the carried registers (not restated, not closed)

| Open item | Carried in |
|---|---|
| Pilot freeze UNSIGNED — no customer/site exists | P9.01 (`01_pilot_scope_freeze.md`); P9.27 register LR-001.. headline items |
| Stage 9 gate NOT MET; re-entry conditions listed | P9.30 §5 (`30_phase_exit_gate_record.md`) |
| D1/D3/D5/D6 closures (open or proposal-only) for the deployment | P9.03–P9.06, P9.10–P9.11; `.agent/DECISIONS.md` gate table — consolidated in P10.16, not re-decided here |
| No adapter conformance execution exists (suite WRITTEN-UNRUN) | P9.37 (`37_adapter_conformance_pilot.md`); P6.19 suite |
| OpenEMS TLS-on-hop open item; `http://` composes today | P6.10 D-10.7; carried in P9.37 §2 |
| ScenarioRecord v3 non-synthetic refusal blocking pilot replay | P9.38 — Phase 7 follow-up |
| Market capture: all 12 answers EMPTY, no-market-contact DEFAULT | P9.11 (`11_market_program_requirements.md`) |
| Limitations/residual risks carried into the release notes item-for-item | P9.27 register (LR-001..LR-225) → P10.26 |
| D4 license/dependency audit — cargo-deny allowlist question stays a recorded decision point | `.agent/DECISIONS.md`; P10.19 (not this record — nothing installed to generate SBOMs) |
| Stage 1–8 gate rows carried verbatim from P9.01 §2 | `ledger/phase-09-pilot-qualification/01_pilot_scope_freeze.md` §2 |

Build/test status (honest): **WRITTEN-UNRUN.** No build, no test run, no CI
run, no install, no adapter execution, no signature. The tick on P10.01 reads
"implemented, awaiting human build/test/execution and signature."
