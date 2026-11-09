# CHECKLIST — Phase 0: Product and Domain Freeze (Stage 0 exit gate)

Exit gate (§23 Stage 0): *engineering, product, and power-systems reviewers
agree on the first workflow and protected invariants, and every §25.2 gate has
an owner, a decision venue, and a date. Work on physical control remains
blocked until the relevant gates close.*

Each item needs **evidence** (a named artefact + location), not assertion.
Tick an item only when the evidence exists and is linked.

## Scope and product

- [x] Scope/non-goals note exists, contains the full §0.3 list verbatim, the four primary audiences, the five "not the initial target" groups, and the §1.6 responsibility boundary — Evidence: `ledger/phase-00-product-freeze/01_scope_and_non_goals.md` (P0.01) — implemented, awaiting human build/test
- [x] First buyer named (or deferred to D2 with owner/venue/date) — Evidence: `02_first_buyer_use_cases.md` (P0.02) — implemented, awaiting human build/test
- [x] Every §1.5 first-generation use case has a disposition (in-scope for v0.1.0 / staged / out of scope); all eight "later use cases" listed as deferred — Evidence: `02_first_buyer_use_cases.md` (P0.02) — implemented, awaiting human build/test
- [x] Release ladder (§2.1) reproduced with simulation-only 0.x and v1.0.0 operator-approved default; no 0.x field-control promise anywhere — Evidence: `01_scope_and_non_goals.md` + `18_control_authority.md` (P0.01, P0.18) — implemented, awaiting human build/test

## Decisions D1–D6

- [x] D1 (jurisdiction/market): decided with ADR, or scheduled with owner + venue + date — Evidence: `.agent/DECISIONS.md` D1 row (P0.03) — implemented, awaiting human build/test
- [x] D2 (first portfolio/service): decided with ADR, or scheduled with owner + venue + date; first-portfolio sheet names asset mix, sites, service, acceptance criteria — Evidence: `02_first_buyer_use_cases.md` + `03_decision_records.md` §2 (P0.04) — implemented, awaiting human build/test
- [x] D3 (network class/fidelity): decided with ADR, or scheduled with owner + venue + date; 0.x default and revisit trigger recorded as a recommendation — Evidence: `03_decision_records.md` §3 (P0.05) — implemented, awaiting human build/test
- [x] D4 (license/dependency audit): owner + venue + date recorded; gate "before v0.1.0" stated — Evidence: `.agent/DECISIONS.md` D4 row (P0.06) — implemented, awaiting human build/test
- [x] D5 (first protocol/asset mix): owner + venue + date recorded; gate "before v0.4.0" stated — Evidence: `.agent/DECISIONS.md` D5 row (P0.06) — implemented, awaiting human build/test
- [x] D6 (cyber/OT boundary review): owner + venue + date recorded; gate "before any read-only pilot" stated — Evidence: `.agent/DECISIONS.md` D6 row (P0.06) — implemented, awaiting human build/test
- [ ] Gate table (P0.28) complete: all six gates have a named owner (person), a venue, and a target date; escalation rule stated per gate — Evidence: ______________________ (P0.28) — NOT MET: named persons and target dates are assigned at the Stage 0 gate meeting (ledger forbids inventing them); skeleton with owner roles + escalation rule in `03_decision_records.md` §7

## Frozen contracts

- [x] Canonical units/sign contract (§10.2): MW/MVAr/MVA/MWh; P > 0 = export; Q declared per model/profile; SOC [0,1]; foundations §6 cross-check recorded with zero unresolved conflicts — Evidence: `07_units_sign_conventions.md` + `26_foundations_crosscheck.md` (P0.07, P0.26) — implemented, awaiting human build/test
- [x] Telemetry quality vocabulary: exactly six states with §10.4 meanings; exactly-one rule; source-mapping rule; non-silencing rule — Evidence: `08_telemetry_quality_vocabulary.md` (P0.08) — implemented, awaiting human build/test
- [x] Time model (§10.3): event_time vs ingest_time, UTC-internal, DST rule, clock-quality field — Evidence: `09_time_model.md` (P0.09) — implemented, awaiting human build/test
- [x] Asset/capability vocabulary: all 14 §10.1 entities; §2.2 table intact; "not a single max_mw" rule; measured/configured/inferred/unknown rule — Evidence: `10_asset_capability_vocabulary.md` (P0.10) — implemented, awaiting human build/test
- [x] Command state machine (§14): all 14 states; every §14.1 transition in the table, none added; SUPERSEDED ≠ CANCELLED; all §14.3 invariants listed; three-gate distinction; acknowledgement ≠ delivery; no universal fallback — Evidence: `11_command_state_machine.md` (+ code oracle `crates/flexcore-domain/src/command_state.rs`) (P0.11) — implemented, awaiting human build/test
- [x] Grid-model contract (§12.4–§12.5): versioned-model rule; AC power flow ≠ AC-OPF; SOCP-relaxation warning; stale/incomplete model is not a pass; no provider selected — Evidence: `12_grid_model_contract.md` (P0.12) — implemented, awaiting human build/test
- [x] Solver discipline (§7.4, §12.3): HiGHS LP/QP/MIP only; Clarabel convex conic, not MISOCP; IPOPT local only; four tiers; provider-contract declaration list — Evidence: `13_solver_class_discipline.md` (P0.13) — implemented, awaiting human build/test
- [x] Event vocabulary: all 26 §15.3 events spelled exactly; events-are-facts rule; at-least-once/idempotency rule; four-step outbox sequence — Evidence: `14_event_vocabulary.md` (+ `crates/flexcore-domain/src/events.rs`) (P0.14) — implemented, awaiting human build/test
- [x] API surface: all 16 §15.2 resources under `/api/v1`; five §15.1 styles; no-arbitrary-command-endpoint prohibition; SemVer/migration-note rule — Evidence: `15_api_surface.md` (+ `crates/flexcore-domain/src/api_paths.rs`) (P0.15) — implemented, awaiting human build/test
- [x] Tenancy/storage principles: §16.1 storage rows; all seven §16.2 tenancy rules incl. RLS table-owner caveat; per-data-class retention, no universal period; selective event sourcing (§6.5) — Evidence: `16_tenancy_storage_principles.md` (P0.16) — implemented, awaiting human build/test

## Security and authority

- [x] Threat model (§17): Rev. 3 final / Rev. 4 draft (21 Sep 2026, comments due 30 Nov 2026); five zones + flow rule; all §17.3 controls; all 10 §17.4 threats; all 10 §17.5 invariants; "not a certification claim" qualifier — Evidence: `17_threat_model.md` (P0.17) — implemented, awaiting human build/test
- [x] Control-authority modes (§21.4): all four modes tabulated; mode-is-a-deployment-setting rule; mixed-modes rule; 0.x simulation-only — Evidence: `18_control_authority.md` (P0.18) — implemented, awaiting human build/test
- [x] Plugin model (§5): protected kernel named; three execution classes; dylib rejection; §5.7 permission table incl. never-granted rows; v0.1.0 = static built-ins + `sim-lab` only — Evidence: `19_plugin_model.md` (P0.19) — implemented, awaiting human build/test
- [x] Deployment/build form (§21): five hosting modes; Compose-first / Helm-later; §21.2 recovery capabilities + central-outage rule; §21.3 correlation list + no-secrets-in-logs; §21.4 distribution table — Evidence: `20_deployment_build_form.md` (P0.20) — implemented, awaiting human build/test

## Governance

- [x] ADRs ADR-001…ADR-012 exist, each with context, decision, alternatives, consequences, revisit conditions; status "proposed"; none claims a D1–D6 decision — Evidence: `docs/architecture/adr/ADR-001.md` … `ADR-012.md` (P0.21) — implemented, awaiting human build/test
- [x] Console scope: twelve §20.1 workspaces; all nine §20.2 display rules; console marked as Phase 5 work — Evidence: `22_console_scope.md` (P0.22) — implemented, awaiting human build/test
- [x] Test strategy: six §19.1 tracks, ten §19.2 layers, §19.3 benchmark cases, §19.4 metrics; no-arbitrary-targets rule — Evidence: `23_simulation_test_strategy.md` (P0.23) — implemented, awaiting human build/test
- [x] Vertical-slice map: all ten §24 steps mapped to §23 stages; "not all in v0.1.0" caveat stated — Evidence: `24_vertical_slice_map.md` (P0.24) — implemented, awaiting human build/test
- [x] Research cut-off check: every Phase 0 external claim sourced + dated in `RESEARCH.md`; post-cut-off changes listed or "none affecting Phase 0" recorded — Evidence: `RESEARCH.md` §5 (P0.25) — implemented, awaiting human build/test
- [x] Foundations cross-check: every foundations §6 convention in the table; zero unresolved conflicts; silent areas listed as silent — Evidence: `26_foundations_crosscheck.md` (P0.26) — implemented, awaiting human build/test

## Handover and gate

- [x] Handover pack index lists every P0.01–P0.26 deliverable with location + status; open questions listed with owning gates; Phase 1 entry condition stated — Evidence: `27_handover_pack.md` (P0.27) — implemented, awaiting human build/test
- [ ] Exit-gate review held with engineering, product, and power-systems reviewers present (or recorded delegates); agreement on first workflow and protected invariants recorded; outcome (pass / pass with conditions / fail) recorded with conditions or blocking items; dissent recorded — Evidence: ______________________ (P0.29) — NOT MET: review NOT HELD; requires human reviewers — see `29_exit_gate_review.md`
- [ ] Anti-scope audit filed: no code/decisions/claims beyond the freeze; repo state verified (`docs/foundations/*` untouched, report untouched, nothing committed/pushed) — Evidence: `git status` output (P0.30) — NOT MET AS WRITTEN: audit filed (see `30_anti_scope_audit.md`) but records the flexcore-domain code-contracts deviation made per the coordinator loop rules; foundations/report untouched, nothing committed/pushed

## Gate outcome

- Outcome: ☐ Pass ☐ Pass with conditions ☐ Fail → **NOT HELD by the implementation agent — requires the three human reviewer groups; see `29_exit_gate_review.md`**
- Conditions / blocking items: Phase 1 implementation proceeds on the frozen contracts while the gate is scheduled; the gate must be recorded before any release claim (see `29_exit_gate_review.md`)
- Reviewers (names + groups): ________ (to be recorded at the gate meeting)
- Date: ________ (date of the gate meeting)
