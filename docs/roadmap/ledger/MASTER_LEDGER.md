# FlexCore Roadmap Ledger — F-16 / F-17 Master Ledger

> **PLAN — Roadmap ledger, 2026-11-21.** Status: **FINALIZED (plan only)**.
> 7 waves, **76 subtasks** (verified by grep recount after the
> iteration-03 patch pass: 10/20/8/5/13/12/8), 79 files. Independently
> reviewed in iteration 01 (20 findings — 0 Critical / 2 High /
> 14 Medium / 4 Low, all patched) and re-reviewed in iteration 02
> (zero findings remaining), then adversarially audited in iteration 03
> (45 findings — 3 High / 17 Medium / 25 Low) and patched in the same
> session, then re-reviewed in iteration 04 (12 findings — 1 High /
> 4 Medium / 7 Low) and patched in the same session, then re-reviewed
> in iteration 05 (7 findings — 0 High / 1 Medium / 6 Low) and patched
> in the same session, then re-reviewed in iteration 06 (6 findings —
> 0 High / 1 Medium / 5 Low) and patched in the same session, then
> re-reviewed in iteration 07 (4 findings — 0 High / 1 Medium / 3 Low)
> and patched in the same session, then re-reviewed in iteration 08
> (3 findings — 0 High / 1 Medium / 2 Low) and patched in the same
> session, then re-reviewed in iteration 09 (1 finding — 0 High /
> 0 Medium / 1 Low) and patched in the same session, then re-reviewed
> in iteration 10 (1 finding — 0 High / 0 Medium / 1 Low) and patched
> in the same session, then re-reviewed in iteration 11 (zero findings)
> — see
> [`FINALIZATION.md`](FINALIZATION.md)
> and `ledger/_iterations/`. The FINALIZED stamp from iterations 01–02
> was SUSPENDED by the iteration-03 adversarial audit and is RESTORED
> by the iteration-11 independent re-review, which returned zero
> findings. "Finalized" describes the *plan* — all 76
> subtasks are unchecked and no implementation work is done. Nothing
> in this ledger is built, tested, or run. The blocking next decision is
> Wave 0, Gate 0 (AGENTS.md reconciliation, existing gates, D4, D1/D2,
> both features kept outside the v1.0.0 release critical path).

- Sources of truth: `docs/roadmap/foundations.md` (roadmap research;
  F-16 PROPOSED post-v1, F-17 RESEARCH candidate, generic household
  energy audit deprioritized, neither delays v1.0.0 gates, F-17
  single-country prototype rule, regional packs rule) and
  `docs/roadmap/flexcore_extensions_principal_engineering_design.md`
  (the system design this ledger is built from; research cut-off
  21 November 2026).
- Ledger cut-off: 21 November 2026. Design-doc claims were not
  re-verified against primary sources in this draft; re-verify versions,
  licenses and standard editions at implementation time.
- Status at creation: **all 76 subtasks unchecked** (`- [ ]`). Nothing in
  this ledger is done. Nothing is committed or pushed.
- How to use this file: pick the next wave whose entry gate is met, open
  its directory, work its subtask files in order. Tick a subtask only
  with evidence (see `README.md`).
- Spelling: "license" (US spelling).

## 1. Wave map

Waves 0–3 are F-16 work inside the FlexCore repository and modular
monolith (FlexCore control-plane work). Waves 4–6 are F-17 work in the
separate companion product and repository (companion-repo work with a
separate identity and database — NOT FlexCore control-plane work).
Wave 0 is the gate that protects the existing FlexCore release and must
pass before any F-16 or F-17 work starts.

| Wave | Design trace | Goal | Workstream | Subtasks |
|---|---|---|---|---|
| 0 — Gate 0 | Design §14 Gate 0; §0; §1.2; §1.3; §2.1 | Reconcile AGENTS.md, resolve existing gates, close/schedule D4, record D1/D2, record the §0 baseline, decide G1/G4, record the §2.1 cross-product architecture, record the F-16 G8 retention/consent decision, and keep both features outside the v1.0.0 critical path | FlexCore control-plane work | W0.01–W0.10 (10) |
| 1 — F-16 Phase 1, evidence-first assessment pilot | Design §3–§7; §14 F-16 P1 | Facility/building/boundary model, evidence ingestion, baseline analysis, BuildingSync v2.7.0, ASHRAE 211 boundary, writable Assessment Studio, Phase 1 readiness report, F-16 tariff pack; a real assessor/operator can create, correct, review and export a defensible evidence package | FlexCore control-plane work | W1.01–W1.20 (20) |
| 2 — F-16 Phase 2, measures + flexibility discovery | Design §5.4–§5.6; §14 F-16 P2 | Curated ECM catalog, measure scenarios/economics, flexibility discovery lifecycle, qualification checklist, safe mapping to existing Core assets, onboarding draft + human approval | FlexCore control-plane work | W2.01–W2.08 (8) |
| 3 — F-16 Phase 3, efficiency savings M&V + integrations | Design §7.3; §14 F-16 P3 | Efficiency savings M&V (IPMVP) as a separate context, pilot-only BMS/EMS/edge/interval integrations, catalog/sector expansion on real evidence | FlexCore control-plane work | W3.01–W3.05 (5) |
| 4 — F-17 Phase 0, market/calculation proof | Design §9.1, §10, §14 F-17 P0; §16 G5–G8 | One launch country/utility, authoritative tariff/cost sources, roof comparison, PV-only and PV+battery prototype on real interval and monthly-only data, PVWatts/REopt/SAM benchmark, homeowner usability tests | Companion-repo work (separate identity/database — NOT FlexCore control-plane work) | W4.01–W4.13 (13) |
| 5 — F-17 Phase 1, independent companion product | Design §8–§11, §13.4; §14 F-17 P1 | Consumer authentication + separate data store, property/bill/tariff/roof model, scenario comparison, financial model, immutable calculation report, export/deletion/consent controls, API surface, privacy/security design, §8.2 nine-step user journey trace, final G7 decision | Companion-repo work (separate identity/database — NOT FlexCore control-plane work) | W5.01–W5.12 (12) |
| 6 — F-17 later phases + regional packs | Design §12; §14 later phases; foundations.md §7 item 3 | More jurisdiction packs, utility-data connections, local roof imagery + verified installer quotes, financing/incentive support where reliable data exists, optional VPP enrollment handoff, regional energy planning packs | Companion-repo work (separate identity/database — NOT FlexCore control-plane work) | W6.01–W6.08 (8) |
| **Total** | | | | **76** |

Subtask counts are by "Acceptance criteria" lines across the wave files
(grep-verified 21 Nov 2026 after the iteration-03 patch pass:
10/20/8/5/13/12/8 = 76). File numbers match
subtask numbers (01 = W<w>.01); subtask IDs are unique and contiguous
1..N within each wave. Two subtasks were added in the iteration-01
patch pass and continue the sequence after their wave's exit-gate
subtask (W0.09, W5.11); they are worked before the exit gate that
evidences them, following the v1 ledger's own precedent (P2.29
depending on the review-added P2.31). Four more subtasks were added
in the iteration-03 patch pass (W0.10, W1.19, W1.20, W5.12) and
continue their wave sequences the same way; W5.10 was renamed from
"Phase 1 exit-gate review" to "Phase 1 scope review" in the same pass
because design §14 defines no F-17 Phase 1 exit gate.

## 2. Status dashboard (ledger FINALIZED as a plan — 2026-11-21; work all unchecked at start)

Ledger status: **FINALIZED — plan only** (see `FINALIZATION.md`). Work status:
**0/76 subtasks ticked**. Nothing is implemented, built, tested, or run.

| Wave | Subtasks | Done | Exit gate met? | Blocking decision |
|---|---|---|---|---|
| 0 — Gate 0 | 10 | 0 | - [ ] | AGENTS.md reconciliation (§1.2); D4; D1/D2; G1; G4; §2.1 architecture record (W0.09); F-16 G8 (W0.10) |
| 1 — F-16 Phase 1 | 20 | 0 | - [ ] | G2 (first facility archetype); G3 (first asset integration); readiness report (W1.19); tariff pack (W1.20) |
| 2 — F-16 Phase 2 | 8 | 0 | - [ ] | — |
| 3 — F-16 Phase 3 | 5 | 0 | - [ ] | Licensed IPMVP copy + practitioner review (W3.02) |
| 4 — F-17 Phase 0 | 13 | 0 | - [ ] | G5 (launch jurisdiction); G6 (roof imagery budget); G7 (REopt API vs local); G8 (retention/consent); G9 (license, via W0.03) |
| 5 — F-17 Phase 1 | 12 | 0 | - [ ] | G7 final decision (W5.12); G8 (retention/consent, if still open); §8.2 journey trace (W5.11) |
| 6 — F-17 later phases | 8 | 0 | - [ ] | Per-jurisdiction packs; aggregator program (for W6.05) |
| **Total** | **76** | **0** | | |

## 3. Entry / exit gates

Entry gate for a wave = previous wave's exit gate met, plus the decision
gates in the table above. The design §14 exit gate for each phase that
has one is restated in its wave's exit-gate review subtask with the
evidence required per item. F-16 Phase 3 and F-17 Phase 1 have NO
design §14 exit gate — W3.05 and W5.10 are scope reviews, not exit
gates — and Wave 6 has none either (W6.08 is a close-out).

| Wave | Entry gate | Exit gate (design §14, abbreviated) |
|---|---|---|
| 0 — Gate 0 | Design doc + foundations.md available; reviewers named (product, architecture, power systems, legal, security) | Gate 0 items 1–5 evidenced (AGENTS.md reconciled; existing gates resolved; D4 closed/scheduled; D1/D2 recorded; both features outside the v1.0.0 critical path). |
| 1 — F-16 Phase 1 | Gate 0 exit gate (W0.08) and the §2.1 architecture record (W0.09); G1 decided or recorded as an interim position (G2 is a Wave 1 deliverable, decided in W1.02) | A real assessor/facility operator can create, correct, review and export a defensible evidence package; every output has source and limitation records; no automatic dispatch qualification and no claims of formal ASHRAE conformance. |
| 2 — F-16 Phase 2 | F-16 Phase 1 exit gate | An independently reviewed candidate maps to existing Core contracts without schema shortcuts, and the Core simulation can consume the approved asset; this does not yet claim field readiness. |
| 3 — F-16 Phase 3 | F-16 Phase 2 exit gate | No §14 exit gate for Phase 3 — scope: efficiency-savings verification workflow in place (licensed IPMVP material + practitioner review); pilot-only integrations in place; catalogs expanded on real evidence. F-16 handover recorded (W3.05 scope review). |
| 4 — F-17 Phase 0 | Gate 0 exit gate (W0.08) and the §2.1 architecture record (W0.09) (G5 is a Wave 4 deliverable, decided in W4.01) | The prototype can honestly tell a homeowner when solar, battery or neither is financially sensible under the selected local tariff; no installer CRM or advanced 3D design needed. |
| 5 — F-17 Phase 1 | F-17 Phase 0 exit gate | Consumer authentication + separate data store; property, bill/interval import, tariff selection, manual/optional roof model; PV-only and PV+battery comparison; immutable calculation report; export, deletion and consent controls. Design §14 defines NO exit gate for F-17 Phase 1 — W5.10 is a scope review, not an exit gate. |
| 6 — F-17 later phases | F-17 Phase 1 scope review (W5.10) | No §14 exit gate for later phases — scope per item: each new jurisdiction pack sourced from authoritative sources with validity dates; utility connections consented; installer quotes verified and labelled; financing/incentives only where reliable data exists; VPP handoff only on explicit opt-in with aggregator review. Ledger close-out (W6.08). |

Note the two F-17 entry points: Wave 4 enters from Gate 0, not from
Wave 3. F-16 and F-17 proceed in parallel after Gate 0; neither depends
on the other's phase gates. F-16 is the proposed next FlexCore extension;
F-17 stays a research candidate until its Phase 0 prototype demonstrates
real demand (foundations.md §8). Do not launch both simultaneously:
F-16 first; F-17 stays a research candidate until its Phase 0 prototype
demonstrates real demand (W0.05). G2 and G5 are decided inside their own
waves (W1.02, W4.01), not before them; no wave's entry gate requires a
decision its own subtasks produce (iteration-01 finding 1-05).

## 4. Dependencies between waves

- W0 → all: Gate 0 baseline (§0 executive decisions, ADR-SHARED-01),
  D4 position, D1/D2 records, and the §2.1 cross-product architecture
  record (W0.09). Later waves reference `W0.xx` IDs and
  must not assume a Gate 0 result that was not evidenced.
- W1 → W2–W3: domain model, provenance, lifecycle, ingestion,
  baseline, API/events, Studio, persistence, BuildingSync, ASHRAE
  boundary, test plan. Wave 2 consumes the W1.02 pilot definition and
  the W1.06 records. The W1.03 asset-integration decision is consumed
  by Wave 3 (W3.03), not by Wave 2.
- W2 → W3: ECM catalog and scenarios (W2.01/W2.02) feed the Phase 3
  catalog expansion (W3.04); the qualification/approval flow (W2.03–
  W2.07) is the precondition for any later field-facing claim.
- W4 → W5: launch jurisdiction, tariff/cost pack, roof decision, tier
  rules, tariff model, provider seams, REopt decision, retention/
  consent decisions, ADR-F17-01. Wave 5 must not proceed on a
  jurisdiction or tariff assumption Wave 4 did not record.
- W5 → W6: the companion product (auth, data store, API, privacy/
  security, test plan, §8.2 journey trace in W5.11, final G7 decision
  in W5.12, scope review in W5.10) is what later packs, connections, quotes,
  financing and the enrollment handoff build on.
- W6 → close-out: W6.08 verifies every wave's exit gate and reconciles
  the counts; it must not assume a gate result from a wave whose
  review record is missing.
- Cross-feature dependency (narrow, governed): the only F-16/F-17
  exchange is the optional VPP enrollment handoff (W6.05/W6.06, design
  §12): a signed, versioned ResidentialDEREnrollmentPackage, explicit
  opt-in, aggregator verification, tenant-operator approval. There is
  no automatic synchronization of home plans into FlexCore, and the
  planner never receives field-control credentials.

Same-wave later-number dependencies (iteration-01 patch pass, extended
in the iteration-03 patch pass): twelve
Dependencies lines name a later-numbered subtask in the same wave —
W0.08→W0.09 (exit gate evidences the review-added §2.1 record),
W0.08→W0.10 (exit gate evidences the review-added F-16 G8 record),
W1.10→W1.20 (baseline consumes the review-added tariff pack),
W1.18→W1.19 (exit gate evidences the review-added readiness report),
W1.18→W1.20 (exit gate evidences the review-added tariff pack),
W4.04→W4.07 (prototype consumes the tier definitions),
W4.04→W4.08 (prototype consumes the tariff model),
W4.04→W4.09 (prototype consumes the provider seams),
W4.05→W4.08 (benchmark consumes the tariff model),
W5.10→W5.11 (scope review evidences the review-added journey trace),
W5.10→W5.12 (scope review evidences the review-added final G7 decision),
W6.05→W6.06 (handoff consumes the package definition). All twelve are
acyclic (verified by the iteration-02 dependency-graph check for the
original four, and by the same check re-run in iterations 03–06 for the
extended set: zero cycles); execution follows Dependencies, not ID order
(README.md step 3). This follows the v1 ledger's own precedent: P2.29
depends on the review-added P2.31.

Forward references are cross-references, not Dependencies: Dependencies
list prerequisites only (upstream inputs a subtask consumes). Where a
subtask names a downstream consumer, a test-oracle, or an affected
subtask that evidences or consumes its output (for example W0.09→W1.11/
W5.01, W1.17→W2.01/W2.02/W2.03/W2.05/W2.08, W1.20→W1.10/W2.02,
W5.03→W5.12, W6.01→W6.07, W6.06→W6.05), that ID is a forward
cross-reference recorded in Inputs, steps or acceptance, and adding it
to Dependencies would create a cycle. The W5.08↛W5.09 case is an
instance of this class (W5.09 tests the controls W5.08 traces). W4.07
step 1 documents the pattern for its own case (the W4.04 prototype is
a later verification consumer of the tier definitions, not a
prerequisite for them).

## 5. §16 open-gate mapping (design §16 — must not be guessed)

| ID | Gate | Owner | Status | Ledger owner |
|---|---|---|---|---|
| G1 | F-16 jurisdiction (electricity market, regulatory, data-retention/legal environment, tariff source) | Product + legal | OPEN | W0.06 |
| G2 | F-16 first facility archetype (office, warehouse, hotel, campus or another class) | Product + power systems | OPEN | W1.02 |
| G3 | F-16 first asset integration (actual PV/BESS/HVAC-thermal/EVSE combination at the pilot facility; which protocol/EMS can expose it) | Power systems + integration | OPEN | W1.03 |
| G4 | Professional audit responsibility (who signs formal audit reports; qualifications/insurance/liability in the chosen market) | Product + legal + qualified engineering reviewer | OPEN | W0.07 |
| G5 | F-17 launch jurisdiction (country and distribution utility supplying authoritative tariff, export-credit, installed-cost, incentive and interconnection data) | Product | OPEN | W4.01 |
| G6 | Roof imagery budget and coverage (is a paid roof data provider economically viable for that jurisdiction; what manual fallback is required) | Product + numerical engineering | OPEN | W4.03 |
| G7 | REopt API v3 vs local REopt.jl (API terms/quotas/data sharing vs local operational burden, Julia runtime, solver licensing/maintenance) | Numerical engineering + license/operations review | OPEN | W4.10 (interim position) + W5.12 (final decision) |
| G8 | Data retention and consent (legal retention, account deletion, backup purge, data-sharing obligations in the launch jurisdiction) | Product + privacy/security + legal | OPEN | W0.10 (F-16) + W4.11 (F-17) |
| G9 | License (BuildingSync, REopt and all dependencies/datasets/providers included in D4 or the companion equivalent) | Legal + engineering | OPEN | W0.03 (D4 audit) |

## 6. §15 ADR mapping (design §15 — 12 ADRs)

| ADR | Decision to record | Owner / gate | Ledger owner |
|---|---|---|---|
| ADR-F16-01 | Facility / Building / EnergyBoundary is a bounded context separate from Core Site / Asset | Product + domain architecture | W1.05 |
| ADR-F16-02 | BuildingSync v2.7.0 importer/exporter and selected Schematron use case | Product + building-energy reviewer + license review | W1.15 |
| ADR-F16-03 | Utility bills and interval datasets are separate records; bills are not telemetry | Domain architecture | W1.09 |
| ADR-F16-04 | Energy efficiency baseline/M&V is separate from P16 demand-response baseline/event M&V | M&V practitioner + product | W3.02 |
| ADR-F16-05 | Flexibility candidate → engineering qualification → onboarding draft → human approval | Power systems + security/OT + product | W2.07 |
| ADR-F16-06 | Dedicated writable Assessment Studio; existing operator console remains read-only | Frontend/API owners | W1.13 |
| ADR-F17-01 | Household planner is a separate application/repository, identity boundary and database | Product + privacy/security | W4.12 |
| ADR-F17-02 | Local tariff pack is required for bill-savings claims; unsupported tariff constructs refuse | Product + local energy-market expert | W4.08 |
| ADR-F17-03 | PVWatts/roof/REopt engines are behind versioned provider adapters | Numerical engineering + license/operations review | W4.09 |
| ADR-F17-04 | Monthly-bill-only profile is not promoted to measured hourly data | Product + numerical validation | W4.07 |
| ADR-SHARED-01 | Extract shared pure models only after a proven common calculation exists | Architecture review | W0.05 |
| ADR-DATA-01 | Source, quality, uncertainty and verification are independent provenance dimensions | Data architecture + safety review | W1.07 |

All 12 ADRs are PROPOSED in the design doc, not approved. Each owning
subtask records its ADR as proposed, pending approval by its named
owners; no subtask records an ADR as decided unless its owners have
decided it.

Sequencing note (G1↔G2, iteration-01 finding 1-06): G1 (W0.06) is
decided from the D1 record alone and does not depend on the pilot
archetype; G2 (W1.02) then selects the archetype inside the decided
jurisdiction. If the archetype genuinely constrains the jurisdiction,
that is recorded as a G1 revisit trigger in W1.02, not as a hidden
dependency.

## 7. Definition of done (whole ledger)

The ledger is done — i.e. F-16 and F-17 are ready to be built from —
only when **all** of the following hold:

1. Every wave's exit gate is met with evidence in its exit-gate review
   subtask (W0.08, W1.18, W2.08, W4.13); for F-16 Phase 3 and F-17 Phase 1
   there is no design §14 exit gate — the W3.05 and W5.10 scope reviews are
   evidenced instead; the W6.08 close-out verifies all of the above.
2. Every §16 gate G1–G9 is decided or recorded as a scheduled decision
   with owner, venue and date; none is silently decided or silently
   assumed by a later wave.
3. All 12 §15 ADRs are filed with owner and status; any ADR still
   proposed is listed as open, not treated as decided.
4. The F-16 test plan (W1.17, design §13.3) and the F-17 test plan
   (W5.09, design §13.4) are executed for their phase scope with
   evidenced results; no test is recorded as passed without evidence.
5. Every user-facing result meets the §13.5 trustworthy-result
   disclosure (what data was used/absent, what is measured/declared/
   provider-derived/assumed, model versions, solver status,
   uncertainty/limitations, whether more data or professional
   inspection is required, whether any financial conclusion is
   unsupported).
6. The ledger has passed an independent review (iteration 01: 20
   findings, all patched) and a re-review (iteration 02: zero findings
   remaining), then an adversarial audit (iteration 03: 45 findings,
   all patched in the same session), a re-review (iteration 04: 12
   findings, all patched in the same session) and a re-review
   (iteration 05: 7 findings, all patched in the same session) and a
   re-review (iteration 06: 6 findings, all patched in the same
   session) and a re-review (iteration 07: 4 findings, all patched in
   the same session) and a re-review (iteration 08: 3 findings, all
   patched in the same session) and a re-review (iteration 09:
   1 finding, patched in the same session) and a re-review (iteration
   10: 1 finding, patched in the same session) and a re-review
   (iteration 11: zero findings); the FINALIZED stamp, suspended by
   the iteration-03 audit, is RESTORED on the basis of that
   zero-findings independent re-review. The reviews' findings and
   fixes are recorded in `ledger/_iterations/`.
7. Both features remain post-v1 and outside the v1.0.0 release
   critical path; no v1.0.0 gate depends on extension work.

## 8. Open items (honest, as of 21 Nov 2026)

- **§16 gates G1–G9: all OPEN.** They cannot be closed by architecture
  alone (design §16). Each has an owning subtask; until each closes,
  later waves proceed only on recorded interim positions with change
  conditions.
- **D1/D2/D4: OPEN** (carried from the v1 ledger; W0.03/W0.04 own the
  extension-facing position). D4 blocks adoption of any new code,
  schema, dataset or library.
- **AGENTS.md defect (design §1.2): OPEN.** AGENTS.md says
  docs-only while README + source tree show written-unrun code. W0.01
  owns the reconciliation; this design must not become authoritative
  until it is resolved.
- **All 12 §15 ADRs: PROPOSED, none approved.** Recorded as proposed
  in their owning subtasks.
- **Licensed standards not yet held:** ASHRAE Standard 211 (2018,
  reaffirmed 2023) + Addendum a, and IPMVP Core Concepts 2022, must be
  obtained as licensed copies with qualified/practitioner review
  before any normative workflow (W1.16, W3.02). No licensed
  table/formula is reproduced from summaries.
- **F-17 is a research candidate, not a commitment.** It stays a
  candidate until the Phase 0 prototype demonstrates real demand
  (foundations.md §8; W4.13 exit gate).
- **Independent review of this ledger: FINALIZED.** Iteration 01 (independent
  reviewer, 2026-11-21) found 20 items (0 Critical / 2 High / 14 Medium
  / 4 Low); all 20 were patched in the same session, and iteration 02
  (re-review) found zero remaining findings. Iteration 03 (adversarial
  audit, 2026-11-21) then found 45 items (3 High / 17 Medium / 25 Low);
  all 45 were patched in the same session, 4 subtasks were added
  (W0.10, W1.19, W1.20, W5.12) and W5.10 was restated as a scope review.
  Iteration 04 (independent re-review, 2026-11-21) then found 12 items
  (1 High / 4 Medium / 7 Low); all 12 were patched in the same session,
  and W3.05 was restated as a scope review (the same defect class as
  W5.10). Iteration 05 (independent re-review, 2026-11-21) then found
  7 items (0 High / 1 Medium / 6 Low); all 7 were patched in the same
  session (no subtasks added, removed or renamed). Iteration 06
  (independent re-review, 2026-11-21) then found 6 items (0 High /
  1 Medium / 5 Low); all 6 were patched in the same session (no
  subtasks added, removed or renamed; the forward-reference rule is now
  documented in README step 3 and §4). Iterations 07–10 then found
  4, 3, 1 and 1 findings respectively (all patched in the same
  session: §13.5 owners for Waves 4 and 3, ADR owners/status recording,
  the verbatim foundations definition, and the self-inclusive review
  ranges). Iteration 11 (independent re-review, 2026-11-21) returned
  zero findings. The FINALIZED stamp, suspended by the iteration-03
  audit, is RESTORED on the basis of that zero-findings review. See
  `FINALIZATION.md` and `ledger/_iterations/`.

## 9. Cross-wave risks

1. **Three audit/M&V concepts conflated.** The operational append-only
   audit log (flexcore-audit, P1.10), DR event M&V (P16.05), and
   efficiency savings M&V (W3.01/W3.02) are three separate concepts.
   Conflating any two would corrupt both an operational evidence trail
   and a savings claim. The naming discipline is restated in W0.05,
   W3.01 and W3.05.
2. **Solver-class discipline.** A single mislabelled solver (e.g.
   claiming SOCP from HiGHS, or global optimality from IPOPT) would
   invalidate feasibility or sizing evidence. HiGHS is LP/QP/MIP, not
   SOCP. Clarabel is convex conic, not MISOCP. AC power flow is not
   AC-OPF. IPOPT is local, not a global-optimality guarantee. Every
   wave restates the discipline where a solver is named.
3. **No certification claims.** Never write "ASHRAE-certified",
   "BuildingSync-certified", or any conformance claim. The design doc
   forbids it: BuildingSync standardizes exchange structure, not
   calculations; ASHRAE conformance does not follow from a checklist
   or a BuildingSync export.
4. **F-16 boundary breaches.** F-16 must not add building/household
   fields to Core Site/Asset, must not inject new AssetType enum
   values, and must not change the read-only operator console
   contract. Each breach is a named rule in W1.05, W2.05 and W1.13.
5. **F-17 boundary breaches.** F-17 must not create a FlexCore tenant,
   must not touch the FlexCore database, and must not receive dispatch
   permission. ADR-F17-01 (W4.12) is the boundary; the only exchange
   is the governed §12 enrollment handoff (W6.05/W6.06).
6. **Monthly-only data presented as hourly.** A bill-derived monthly
   profile must not be presented as an hourly load curve, and
   monthly-only inputs must never produce claims of measured hourly
   self-consumption or precise battery sizing (W1.10, W4.04, W4.07,
   W5.09).
7. **Tariff/savings overstatement.** Savings must not be calculated as
   annual PV kWh × retail price under net-billing or low export
   compensation, and payback/ROI must be suppressed where the local
   tariff/cost basis is unsupported (W4.08, W5.03, W5.04).
8. **Control authority.** Simulation-only until v1.0.0
   operator-approved control. F-16 qualification is not dispatch
   authority; F-17 never receives dispatch permission. No subtask
   grants field dispatch early.
9. **Version drift.** The design doc is a 21 Nov 2026 research
   snapshot. BuildingSync, PVWatts, REopt, SAM, Google Solar API and
   OpenEI URDB versions and terms move after the cut-off. Re-verify at
   implementation time; do not pin from memory.
10. **Gate 0 skipped.** Starting F-16 or F-17 work before Gate 0 is
    evidenced would build extensions on an unverified workspace, an
    open license audit, and undecided jurisdictions. Wave 0 is the
    entry gate for both features.
