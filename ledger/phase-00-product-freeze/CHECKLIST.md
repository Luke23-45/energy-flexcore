# CHECKLIST — Phase 0: Product and Domain Freeze (Stage 0 exit gate)

Exit gate (§23 Stage 0): *engineering, product, and power-systems reviewers
agree on the first workflow and protected invariants, and every §25.2 gate has
an owner, a decision venue, and a date. Work on physical control remains
blocked until the relevant gates close.*

Each item needs **evidence** (a named artefact + location), not assertion.
Tick an item only when the evidence exists and is linked.

## Scope and product

- [ ] Scope/non-goals note exists, contains the full §0.3 list verbatim, the four primary audiences, the five "not the initial target" groups, and the §1.6 responsibility boundary — Evidence: ______________________ (P0.01)
- [ ] First buyer named (or deferred to D2 with owner/venue/date) — Evidence: ______________________ (P0.02)
- [ ] Every §1.5 first-generation use case has a disposition (in-scope for v0.1.0 / staged / out of scope); all eight "later use cases" listed as deferred — Evidence: ______________________ (P0.02)
- [ ] Release ladder (§2.1) reproduced with simulation-only 0.x and v1.0.0 operator-approved default; no 0.x field-control promise anywhere — Evidence: ______________________ (P0.01, P0.18)

## Decisions D1–D6

- [ ] D1 (jurisdiction/market): decided with ADR, or scheduled with owner + venue + date — Evidence: `.agent/DECISIONS.md` D1 row (P0.03)
- [ ] D2 (first portfolio/service): decided with ADR, or scheduled with owner + venue + date; first-portfolio sheet names asset mix, sites, service, acceptance criteria — Evidence: ______________________ (P0.04)
- [ ] D3 (network class/fidelity): decided with ADR, or scheduled with owner + venue + date; 0.x default and revisit trigger recorded as a recommendation — Evidence: ______________________ (P0.05)
- [ ] D4 (license/dependency audit): owner + venue + date recorded; gate "before v0.1.0" stated — Evidence: `.agent/DECISIONS.md` D4 row (P0.06)
- [ ] D5 (first protocol/asset mix): owner + venue + date recorded; gate "before v0.4.0" stated — Evidence: `.agent/DECISIONS.md` D5 row (P0.06)
- [ ] D6 (cyber/OT boundary review): owner + venue + date recorded; gate "before any read-only pilot" stated — Evidence: `.agent/DECISIONS.md` D6 row (P0.06)
- [ ] Gate table (P0.28) complete: all six gates have a named owner (person), a venue, and a target date; escalation rule stated per gate — Evidence: ______________________ (P0.28)

## Frozen contracts

- [ ] Canonical units/sign contract (§10.2): MW/MVAr/MVA/MWh; P > 0 = export; Q declared per model/profile; SOC [0,1]; foundations §6 cross-check recorded with zero unresolved conflicts — Evidence: ______________________ (P0.07, P0.26)
- [ ] Telemetry quality vocabulary: exactly six states with §10.4 meanings; exactly-one rule; source-mapping rule; non-silencing rule — Evidence: ______________________ (P0.08)
- [ ] Time model (§10.3): event_time vs ingest_time, UTC-internal, DST rule, clock-quality field — Evidence: ______________________ (P0.09)
- [ ] Asset/capability vocabulary: all 14 §10.1 entities; §2.2 table intact; "not a single max_mw" rule; measured/configured/inferred/unknown rule — Evidence: ______________________ (P0.10)
- [ ] Command state machine (§14): all 14 states; every §14.1 transition in the table, none added; SUPERSEDED ≠ CANCELLED; all §14.3 invariants listed; three-gate distinction; acknowledgement ≠ delivery; no universal fallback — Evidence: ______________________ (P0.11)
- [ ] Grid-model contract (§12.4–§12.5): versioned-model rule; AC power flow ≠ AC-OPF; SOCP-relaxation warning; stale/incomplete model is not a pass; no provider selected — Evidence: ______________________ (P0.12)
- [ ] Solver discipline (§7.4, §12.3): HiGHS LP/QP/MIP only; Clarabel convex conic, not MISOCP; IPOPT local only; four tiers; provider-contract declaration list — Evidence: ______________________ (P0.13)
- [ ] Event vocabulary: all 26 §15.3 events spelled exactly; events-are-facts rule; at-least-once/idempotency rule; four-step outbox sequence — Evidence: ______________________ (P0.14)
- [ ] API surface: all 16 §15.2 resources under `/api/v1`; five §15.1 styles; no-arbitrary-command-endpoint prohibition; SemVer/migration-note rule — Evidence: ______________________ (P0.15)
- [ ] Tenancy/storage principles: §16.1 storage rows; all seven §16.2 tenancy rules incl. RLS table-owner caveat; per-data-class retention, no universal period; selective event sourcing (§6.5) — Evidence: ______________________ (P0.16)

## Security and authority

- [ ] Threat model (§17): Rev. 3 final / Rev. 4 draft (21 Sep 2026, comments due 30 Nov 2026); five zones + flow rule; all §17.3 controls; all 10 §17.4 threats; all 10 §17.5 invariants; "not a certification claim" qualifier — Evidence: ______________________ (P0.17)
- [ ] Control-authority modes (§21.4): all four modes tabulated; mode-is-a-deployment-setting rule; mixed-modes rule; 0.x simulation-only — Evidence: ______________________ (P0.18)
- [ ] Plugin model (§5): protected kernel named; three execution classes; dylib rejection; §5.7 permission table incl. never-granted rows; v0.1.0 = static built-ins + `sim-lab` only — Evidence: ______________________ (P0.19)
- [ ] Deployment/build form (§21): five hosting modes; Compose-first / Helm-later; §21.2 recovery capabilities + central-outage rule; §21.3 correlation list + no-secrets-in-logs; §21.4 distribution table — Evidence: ______________________ (P0.20)

## Governance

- [ ] ADRs ADR-001…ADR-012 exist, each with context, decision, alternatives, consequences, revisit conditions; status "proposed"; none claims a D1–D6 decision — Evidence: ______________________ (P0.21)
- [ ] Console scope: twelve §20.1 workspaces; all nine §20.2 display rules; console marked as Phase 5 work — Evidence: ______________________ (P0.22)
- [ ] Test strategy: six §19.1 tracks, ten §19.2 layers, §19.3 benchmark cases, §19.4 metrics; no-arbitrary-targets rule — Evidence: ______________________ (P0.23)
- [ ] Vertical-slice map: all ten §24 steps mapped to §23 stages; "not all in v0.1.0" caveat stated — Evidence: ______________________ (P0.24)
- [ ] Research cut-off check: every Phase 0 external claim sourced + dated in `RESEARCH.md`; post-cut-off changes listed or "none affecting Phase 0" recorded — Evidence: `RESEARCH.md` §5 (P0.25)
- [ ] Foundations cross-check: every foundations §6 convention in the table; zero unresolved conflicts; silent areas listed as silent — Evidence: ______________________ (P0.26)

## Handover and gate

- [ ] Handover pack index lists every P0.01–P0.26 deliverable with location + status; open questions listed with owning gates; Phase 1 entry condition stated — Evidence: ______________________ (P0.27)
- [ ] Exit-gate review held with engineering, product, and power-systems reviewers present (or recorded delegates); agreement on first workflow and protected invariants recorded; outcome (pass / pass with conditions / fail) recorded with conditions or blocking items; dissent recorded — Evidence: ______________________ (P0.29)
- [ ] Anti-scope audit filed: no code/decisions/claims beyond the freeze; repo state verified (`docs/foundations/*` untouched, report untouched, nothing committed/pushed) — Evidence: `git status` output (P0.30)

## Gate outcome

- Outcome: ☐ Pass ☐ Pass with conditions ☐ Fail
- Conditions / blocking items: ________________________________________________
- Reviewers (names + groups): ________________________________________________
- Date: ______________________
