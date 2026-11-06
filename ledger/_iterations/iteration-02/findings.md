# Iteration 02 Findings — FlexCore Ledger, Phases 0–3 (deep re-analysis)

- **Reviewer:** Iteration 2 reviewer (subagent, parent orchestrator task)
- **Date:** 7 November 2026 (research cut-off date; all web re-verification done this day)
- **Scope:** `ledger/phase-00-product-freeze/*`, `ledger/phase-01-rust-foundation/*`,
  `ledger/phase-02-asset-telemetry/*`, `ledger/phase-03-flexibility-scheduling/*`
  (EDIT scope). Phases 4–10: READ only — observations recorded, no edits.
- **Grounding:** report v1.1 (`docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`),
  foundations v1.0 (`docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.docx`),
  `AGENTS.md`, `.agent/CONTEXT.md`, `.agent/DECISIONS.md` (D1–D6 OPEN).
- **Method:** full re-read of report §§0–3, 5–12, 15–16, 17–25, 27 for scope; full
  re-read of all four scope ledgers (P0.01–P0.30, P1.01–P1.32, P2.01–P2.30,
  P3.01–P3.30) plus their PLAN/CONTEXT/RESEARCH/CHECKLIST files; every subtask
  re-checked against (i) §23 exit gate, (ii) §2 release scope, (iii) §24 slice;
  every RESEARCH.md version claim re-verified against primary sources via
  `browser.search` / `browser.open` on 7 November 2026.
- **Patch boundary honoured:** no edits to `docs/foundations/*`, the report,
  phases 4–10, `ledger/README.md`, or `MASTER_LEDGER.md`; nothing committed or pushed.

## Severity summary

| Severity | Count |
|---|---|
| Critical | 0 |
| High | 0 |
| Medium | 19 |
| Low | 11 |
| **Total findings** | **30** |

No Critical or High items were found. That is not a clean bill of health: 19
Mediums is a substantial haul, including one wrong security-fix attribution
(R1), one cross-phase input that no phase produces (H1/I1), one un-gated auth
stub (G5), and one foundations requirement with no carrier subtask at all
(H13). The absence of High/Critical reflects that the draft is structurally
sound and its counts trace correctly — not that it is finished.

## Research re-verification (7 November 2026)

Every RESEARCH.md version claim for phases 0–3 was re-checked against a
primary source. Results:

| Claim | Ledger said | Re-verification outcome |
|---|---|---|
| Rust 1.99.1 / 1.100.0 on 12 Nov 2026 | 1.99.1 current; 1.100 planned 12 Nov 2026 | Confirmed (Rust blog / Forge). NEW: 1.100 carries above-normal breakage risk (never-type inference changes, Cargo target-dir layout v2, Crater reports) → R3 |
| Tokio 1.54.1, MSRV 1.85 | 1.54.1 (21 Oct 2026) | Confirmed. NEW: CVE-2026-115567 in 1.54.0, fixed in 1.54.1 — the pin is security-motivated → R2 |
| Axum 0.8.9, MSRV 1.80 | 0.8.9 latest as of Sep 2026 | Last confirmed Sep 2026 (rustfinity); **not independently re-verified for November** — build-time re-check stands (R7) |
| SQLx 0.9.0, MSRV 1.94.0 | 0.9.0 | Confirmed |
| NATS server 2.15.2 | 2.15.2 latest patch; "fixes CVE-2026-33222" | Version confirmed; **CVE attribution WRONG** — fixed in 2.11.15 / 2.12.6 → R1 |
| NATS Apache-2.0 / CNCF | BSL move reversed 1 May 2025 (secondary, verify before D4) | Confirmed from primary announcements; **superseded by CNCF graduation 2 Nov 2026** → R4 |
| PostgreSQL 18.6 / 18 support to 14 Nov 2030 | 18.6 (13 Aug 2026); 18.5 skipped; PG 19 RC1 15 Oct 2026 | Confirmed |
| PostgreSQL `uuidv7()` built-in | Secondary, verify at build time | **CONFIRMED native in PG 18** (release announcement, RFC 9562) → closes G4 |
| highspy 1.15.1 | 1.15.1 (2 Jul 2026, PyPI) | Confirmed. Gap: no Rust HiGHS binding version researched → I2 |
| Clarabel.rs 0.11.1 | 0.11.1 | Confirmed |
| CVXPY 1.9.3 | 1.9.3 (19 Sep 2026) | Confirmed via live PyPI page crawled 6 Nov 2026 |
| cyipopt 1.7.0 | 1.7.0 (20 Apr 2026) | Confirmed |
| async-nats 0.50.0; sync `nats` deprecated | 0.50.0 (20 Jul 2026) | Confirmed |
| IPOPT 3.14.20 | 3.14.20, verified via package records | **CONFIRMED** (FreshPorts 3.14.19 → 3.14.20, 12 Sep 2026; GAMS list) → R5 |
| Rust edition 2024 default since 1.85 | P1.02 states it | **CONFIRMED** (multiple sources) → closes G9 |
| IEEE 1547-2018 Active; P1547 revision | Active; revision underway (Sandia Sep 2024) | **CONFIRMED + narrowed:** P1547 is an authorized active IEEE SA project; 1547-2018 within 10-year lifecycle; no publication date found → R6 |
| NIST SP 800-82 Rev. 4 draft, comments to 30 Nov 2026 | IPD 21 Sep 2026; Rev. 3 final | Confirmed (NIST CSRC) |
| ISO-NE FERC Order 2222 effective 1 Nov 2026 | Effective 1 Nov 2026; readiness page 4 Nov 2026 | Confirmed (ISO-NE statement 2 Nov 2026) |
| utoipa 5.5.0, tracing 0.1.44, tower 0.5.3, tower-http 0.7.0, OTel Rust 0.32.0 | Secondary-sourced, build-time re-check required | **Not independently re-verified** — re-check stands (R7) |
| FERC CAISO/NYISO/PJM/MISO/SPP dates | Secondary, open verification tasks | **Not independently re-verified** — open tasks stand (R7) |

## Findings — Phase 0 (product freeze)

### F1 — Medium — `ledger_phase_0.md`, P0.07 (+ P0.26)
**SOC representation divergence vs foundations §6.2, unrecorded.** Foundations
§6.2 permits "fraction in [0,1] or percent, but one representation is selected
per API and stated explicitly"; report §10.2 mandates fraction [0,1] at the API
boundary. P0.07 step 7 and its acceptance claimed "zero unresolved conflicts"
without naming this. The stricter report rule is compatible (selecting fraction
satisfies foundations), but a freeze that does not name the divergence cannot
prove it was considered.
**Patch applied:** P0.07 step 7 + acceptance checkbox name the divergence and
record it as the stricter permitted selection; P0.26 step 5 records it in the
cross-check table.

### F2 — Low — `ledger_phase_0.md`, P0.26
**CANCELLED/SUPERSEDED merge unrecorded.** Foundations §9.1 merges
CANCELLED/SUPERSEDED into one state-definition row; report §14.2 distinguishes
them and P0.11 correctly follows the report. The difference was nowhere in the
P0.26 table.
**Patch applied:** P0.26 step 5 records it as a compatible refinement.

### F3 — Medium — `ledger_phase_0.md`, P0.12
**Feasibility-check list is report-only.** P0.12 freezes only the report §12.5
list. Foundations §7.7 adds tap/phase-shifter limits, emergency/normal ratings,
topology/model revision, measurement freshness, and operating-state consistency.
A Phase 4 builder working from the freeze alone would drop them.
**Patch applied:** P0.12 step 7 + acceptance checkbox record the §7.7 extras as
Phase 4 carry-forwards (cross-reference, not a second list).

### F4 — Medium — `ledger_phase_0.md`, P0.08
**Estimation authorization rule has no carrier.** Foundations §6.4:
"Estimation/interpolation is explicit, tagged, bounded by policy, and
disallowed for safety-critical authorization unless approved." No subtask
carried the authorization-disallowance forward; Phase 2 has no authorization to
apply it to, so it would have died silently.
**Patch applied:** P0.08 step 6 + acceptance checkbox state the rule and hand
it to Phase 4, where command authorization first exists.

### F6 — Medium — `CONTEXT.md`, open question 3
**Mis-citation.** "Portfolio visibility is the only mandatory v0.1.0 use case
by the §23 Stage 2 exit gate's wording" is wrong on both halves: the §23 Stage
2 gate is about telemetry ingestion, not use-case mandating; and §2.1 v0.1.0
contents are canonical models, simulator, telemetry, registry, portfolio
views, and replay — portfolio visibility is one use case within that scope,
not the only mandatory one.
**Patch applied:** CONTEXT.md open question 3 rewritten with the correct
sources.

### Verified sound in Phase 0 (no patch)
P0.24 §24 step-to-stage mapping is defensible. P0.25 cut-off items (ISO-NE
readiness 4 Nov 2026, NIST Rev. 4 comment period, Rust 1.100 planned 12 Nov
2026) match RESEARCH.md. P0.21 ADR location `docs/architecture/adr/` matches
§8.1. P0.17 counts correct. P0.19 WASI 0.2 claim matches §5.4. P0.28 date
handling consistent (dates assigned in gate meeting, recorded in
DECISIONS.md). No LICENSE file exists in the repo root (P0.06 claim correct).

## Findings — Phase 1 (Rust foundation)

### G2 — Medium — `ledger_phase_1.md`, P1.03
**Internal contradiction.** Step 3 says the member list contains "only crates
created in this phase (P1.05–P1.08)" — but Phase 1 also creates
flexcore-events (P1.09), flexcore-storage + flexcore-audit (P1.10),
flexcore-authz (P1.11), adapters/postgres (P1.13), adapters/nats (P1.15),
apps/control-plane (P1.17), apps/worker (P1.19). The parenthetical is wrong
and its acceptance criterion ("lists exactly the Phase 1 member crates") would
fail against it.
**Patch applied:** P1.03 step 3 lists the full Phase 1 crate set
(P1.05–P1.11, P1.13, P1.15, P1.17, P1.19) and notes P1.12/P1.14/P1.16/P1.18
create no crates.

### G3 — Medium — `ledger_phase_1.md`, P1.10
**Unresolved home for repository traits.** P1.07 already defines repository
ports in flexcore-ports, but P1.10 step 1 said the traits "may live here or in
ports — decide once". The decision was already made by P1.07; leaving it open
risks duplicated traits drifting apart (the exact failure mode P1.10's own
risks section names).
**Patch applied:** P1.10 step 1 resolves it — repository traits live in
flexcore-ports (per P1.07); flexcore-storage holds only non-port persistence
abstractions.

### G5 — Medium — `ledger_phase_1.md`, P1.17
**Auth stub could ship.** Step 3 allowed a "documented stub/placeholder" OIDC
IdP integration; the risk note admits "auth as a stub that ships". Acceptance
criteria required nothing about gating — a recorded stub in a release build
would be an unauthenticated control plane.
**Patch applied:** P1.17 step 3 + acceptance checkbox require the stub to be
test/dev-profile-gated (cfg-gated, never compiled into release builds) and CI
to fail if it is enabled in a release profile.

### G7 — Medium — `ledger_phase_1.md`, P1.27
**No CI license gate.** §22.2 requires a "license/security review"; P1.27
wires fmt/clippy/tests/migrations/Buf/secret-scan/container-scan/SBOM but no
license scan. P1.28 only produces an inventory nobody enforces — a copyleft
dependency could land and sit until the D4 audit.
**Patch applied:** P1.27 step 1 + acceptance checkbox add a `cargo deny check
licenses` (or equivalent) required CI gate with a negative test; it feeds
P1.28/D4.

### G8 — Low — `ledger_phase_1.md`, P1.25
**Invalid image tag.** Step 1 cites `nats:2.15.x` as a "pinned" tag — `2.15.x`
is not a valid Docker tag; the Compose file as specified would not run.
**Patch applied:** P1.25 step 1 + acceptance require the exact patch tag
resolved at build time and recorded (2.15.2 at the cut-off); no `x` wildcards.

### Claims re-checked and CLOSED in Phase 1 (no ledger patch needed)
- **G4 — PostgreSQL `uuidv7()`:** P1.12 step 2 states it as a design fact
  while RESEARCH.md §6 labelled it secondary/unverified. Re-verification
  confirms it is native in PostgreSQL 18 (release announcement, RFC 9562).
  RESEARCH.md §6 and §8 task 4 updated to confirmed; build-time
  `SELECT uuidv7()` kept as a sanity check only.
- **G9 — Edition 2024 default since Rust 1.85:** P1.02 step 2 claim confirmed
  by multiple sources (Rust 1.85 release, Cargo docs). No patch.

### Verified sound in Phase 1 (no patch)
Tokio 1.54 MSRV 1.85, Axum 0.8 MSRV 1.80, SQLx 0.9.0 MSRV 1.94.0 all
consistent with RESEARCH.md. P1.19 worker-separation justification recorded
per §8.3. P1.06 serde treatment fine. P1.14 tenant-in-subject enumeration risk
flagged. P1.20 placeholder metrics recorded. P1.18 "no .proto files in Phase 1"
option recorded.

## Findings — Phase 2 (asset/telemetry)

### H1 — Medium — `ledger_phase_2.md`, P2.28 (and P3.31)
**Forecasts are consumed but never produced.** Zero "forecast" mentions exist
in the P2 ledger (grep-confirmed), yet P3.12 inputs said "forecasts from Phase
2" and P3 PLAN §5 said Phase 3 consumes "forecasts from Phase 2". No P2
subtask produces any ForecastBundle or forecast provider; §4 Forecasting has
no Phase 2 owner. §23 Stage 2 deliverables do not include forecasts, so this
is a cross-phase wiring error, not a scope gap — but left as written, Phase 3
would wait on an input that never arrives.
**Patch applied:** P2.28 step 5 + acceptance checkbox state the forecast
non-delivery explicitly; the producing subtask is P3.31 (Phase 3 synthetic
forecast fixture), not an expansion of Stage 2.

### H2 — Low — `ledger_phase_2.md`, P2.05
**Partitioning honesty note missing from the design subtask.** P2.05 step 2
defaults to monthly partitioning without recording the note from RESEARCH.md
§1: partitioning pays only when a table would otherwise be very large, and
Phase 2 synthetic size does not justify it — the shape is designed for the
volume trajectory, not measured.
**Patch applied:** P2.05 step 2 + acceptance checkbox record the honesty note.

### H4 — Medium — `ledger_phase_2.md`, P2.06
**Vague migration scope.** P2.06 step 1 says registry migrations "if not
already present from Phase 1" — but P1.12/P1.13 already design and implement
tenants/portfolios/sites/assets tables. The wording risks duplicated
migrations creating a second registry.
**Patch applied:** P2.06 step 1 + acceptance checkbox state the
extend-not-recreate rule (Phase 2 extends with asset-type enum,
capability-profile reference, PCC/grid-model reference).

### H5 — Low — `ledger_phase_2.md`, P2.12
**DELETE without a rule.** P2.12 step 1 adds "full CRUD" (incl. DELETE) for
registry resources but does not reference the P2.02 step 5 deletion rule
(cascade vs restrict).
**Patch applied:** P2.12 step 1 + acceptance checkbox cite P2.02 step 5.

### H7 — Medium — `ledger_phase_2.md`, P2.20
**Undefined no-retention behaviour.** P2.20 step 2 makes retention
manual/approved but defines no behaviour when no retention value is
configured — partitions would accumulate unboundedly with no signal.
**Patch applied:** P2.20 step 5 + acceptance checkbox define it: no deletion;
accumulation visible via metric/alert (or a configured default) — decided
once, written down.

### H13 — Medium — NEW `ledger_phase_2.md`, P2.31
**Foundations §6.4 requirement with no carrier subtask.** "Telemetry
correction and late arrival must be append-only or versioned so historical
decisions can be replayed using the data available at decision time." No P2
subtask requires versioned corrections or as-of (point-in-time) replay: P2.24
replays by re-running the simulator and comparing stored records — a
different thing. Without this, a correction silently rewrites history and a
past decision can no longer be explained from its inputs.
**Patch applied:** new subtask P2.31 (versioned telemetry corrections and
as-of replay), full subtask format; P2.28 handover list and P2.29 review scope
updated to include it; traceability table + count line updated; CHECKLIST.md
item added.

### Verified sound in Phase 2 (no patch)
P2.13/P2.14 determinism scope (`ingest_time` excluded from replay
comparison). P2.08/P2.18 freshness limits as configuration. P2.17
`simulated: true` flag + honest edge-connectivity status. P2.24 sim-runner
decision deferred with default recorded. P2.02 entity graph matches §10.1.
P2.04 per-asset quantities reasonable. P2.16 batch-vs-per-record event
granularity decided and recorded.

## Findings — Phase 3 (flexibility/scheduling)

### I1 — Medium — NEW `ledger_phase_3.md`, P3.31
**Same forecast gap as H1, Phase 3 side.** P3.12 consumed forecasts that no
phase produced; P3.13's "forecast update" staleness trigger and P3.21's
"forecast version" snapshot field likewise pointed at nothing.
**Patch applied:** new subtask P3.31 (synthetic forecast fixture owned by
Phase 3: versioned, deterministic from the P2.13 scenario seed, labelled
synthetic, `ForecastPublished` emission); P3.12 inputs corrected; P3.30
handover updated to include the fixture; P3 PLAN §5 corrected; cross-phase
references corrected; CHECKLIST.md G3.31 added.

### I2 — Medium — `ledger_phase_3.md`, P3.17 (+ RESEARCH.md §1, §6)
**Rust HiGHS binding version never researched.** P3.17 step 2 says Tier 1 is
"via highspy or the Rust binding, per the pinned version in RESEARCH.md" —
but RESEARCH.md pins only highspy 1.15.1 (Python). No Rust binding version was
researched anywhere in the ledger.
**Patch applied:** P3.17 step 2 + acceptance require the binding choice
recorded with a version pin, and the Rust binding's version researched first
if chosen; RESEARCH.md §1 notes the gap and §6 gains open task 6.

### I3 — Medium — `ledger_phase_3.md`, P3.05
**BESS balance omits auxiliary load.** Foundations §7.2 includes an
auxiliary-load term (−P_aux·Δt) and an explicit binary formulation; report
§11.2 omits P_aux. P3.05 implemented the report balance silently, so the
divergence would never surface.
**Patch applied:** P3.05 step 1 + acceptance include P_aux where data exists,
else record it as unknown per P3.02 — never silently zero without a note.

### I4 — Medium — `ledger_phase_3.md`, P3.18
**SOCP relaxation residual not recorded.** Foundations §7.8 requires recording
per-branch SOCP relaxation residual g_ij = v_i·l_ij − (P_ij² + Q_ij²) with a
configured tolerance. P3.18 states the exactness caveat but has no residual
recording — the caveat would be unverifiable per run.
**Patch applied:** P3.18 step 5 + acceptance record the residual and tolerance
per run; the residual does not replace Phase 4 AC verification.

### I6 — Low — `ledger_phase_3.md`, P3.14
**Foundations §7.5 objective terms unaddressed.** Foundations §7.5 lists extra
objective terms (flex discomfort, commitment deviation, reserve shortfall,
switching costs) vs report §12.1. Foundations wording is illustrative ("may
minimize"), not mandatory — but the ledger never said so, leaving a later
reader to wonder whether they were missed.
**Patch applied:** P3.14 step 5 + acceptance record the extras as not in the
v0.2.0 objective, with the reason.

### I8 — Low — `ledger_phase_3.md`, P3.21
**Grid model version has no source.** P3.21 requires "grid model version" in
the §12.6 result snapshot but never says where it comes from — Phase 3 has no
grid model.
**Patch applied:** P3.21 step 2 + acceptance source it from the Phase 2 Site
grid-model reference (P2.02).

### I12 — Medium — `ledger_phase_3.md`, P3.28
**`ScheduleValidated` undefined in Phase 3.** P3.28 step 2 emits
`ScheduleValidated`, but nothing defines what "validated" means here — risking
confusion with Phase 4 network-feasibility validation. A Phase 4 reader could
mistake a Phase 3 `ScheduleValidated` for a feasibility verdict.
**Patch applied:** P3.28 step 2 + acceptance define Phase 3 ScheduleValidated
as asset/time-coupled constraint validation (P3.26), explicitly not network
feasibility.

### Verified sound in Phase 3 (no patch)
P3.03 quality-aggregation acceptance is vague but the definition is the
subtask's own deliverable. P3.26's three scenarios cover the §23 Stage 3 exit
gate. P3.20 IPOPT boundary is honest. P3.30 verification-tool step is correct
(both tools run and pass — see below).

## Findings — research (stale / incorrect)

### R1 — Medium — `phase-01-rust-foundation/RESEARCH.md` §5; `phase-02-asset-telemetry/RESEARCH.md` §3
**CVE-2026-33222 fix-version attribution is wrong.** Both files said v2.15.2
"fixes CVE-2026-33222". Reality, re-verified 7 Nov 2026 (Wiz, Vulert, Feedly,
Ubuntu CVE records, all agreeing): CVE-2026-33222 (JetStream stream-restore
endpoint auth bypass, GHSA-9983-vrx2-fg9c, published 24 March 2026, CVSS 4.9)
was fixed in **2.11.15 and 2.12.6**; every 2.15.x release merely includes the
fix. The v2.15.2 changelog item is CVE-2026-111943 (eMudhra certificate
distrust, Go 1.27.3). The 2.15.x pin itself is safe — the error is in a
security-relevant note, which is exactly where a wrong version is dangerous.
**Patch applied:** corrected wording in both RESEARCH files, with the wrong
wording quoted as wrong; P1 RESEARCH §8 task 2 and P2 RESEARCH §7 task 2
marked resolved with the finding.

### R2 — Low — `phase-01-rust-foundation/RESEARCH.md` §2
**New CVE note missing.** CVE-2026-115567 (disclosed 23 October 2026): race
between `tokio::time::Interval::poll_tick` and the multi-threaded runtime's
time driver in Tokio 1.54.0, use-after-free in safe Rust (CWE-362/CWE-416),
fixed in 1.54.1. The 1.54.1 pin is security-motivated, not just "latest".
**Patch applied:** security note added to §2 (severity labels differ by
source — CNA advisory: critical; CVSS 4.0 base 5.8 per Rapid7 — no single
score asserted).

### R3 — Low — `phase-01-rust-foundation/RESEARCH.md` §8 task 6
**Rust 1.100 bump risk unstated.** 1.100 (planned 12 Nov 2026) carries
above-normal breakage risk: never-type stabilization changing type inference,
Cargo target-directory layout v2 (Crater reports). A bump on the planned date
alone could break the workspace.
**Patch applied:** task 6 now requires a `cargo +beta test` pass on the full
workspace before any 1.100 bump.

### R4 — Medium — `phase-01-rust-foundation/RESEARCH.md` §5, §8; `ledger_phase_1.md` P1.28
**NATS license story superseded by graduation.** The ledger correctly recorded
the 1 May 2025 CNCF/Synadia agreement reversing the BSL move (verified this
pass from the joint announcement), but the episode has a decisive close the
ledger predates: NATS **graduated in CNCF on 2 November 2026** (CNCF project
page, checked 7 Nov 2026). The "verify before the D4 review" qualifier is
retired.
**Patch applied:** §5 License row updated with primary sources + graduation;
§8 task 3 marked resolved; P1.28 step 3 cites graduation as the decisive close.

### R5 — Low (info) — `phase-03-flexibility-scheduling/RESEARCH.md` §4, §6
**IPOPT 3.14.20 confirmed.** Existence corroborated this pass via the
FreshPorts commit record (3.14.19 → 3.14.20, 12 Sep 2026) and the GAMS
third-party list. The GitHub release-notes read remains a build-time check,
not an open version question.
**Patch applied:** §4 + §6 task 4 updated.

### R6 — Low (info) — `phase-00-product-freeze/RESEARCH.md` §2
**IEEE 1547 / P1547 status narrowed.** IEEE SA project page (checked 7 Nov
2026): P1547 is a currently authorized project; 1547-2018 is within its
10-year lifecycle (Active). No expected publication date was found — none is
asserted.
**Patch applied:** §2 open task updated (status closed; publication date
remains open).

### R7 — Low (info) — carried forward, not a fix
**Still secondary / unverified by design:** utoipa 5.5.0, tracing 0.1.44,
tracing-subscriber 0.3.23, tower 0.5.3, tower-http 0.7.0, OpenTelemetry Rust
0.32.0 (all already labelled secondary with build-time re-check required);
FERC CAISO/NYISO/PJM/MISO/SPP dates (already flagged as open verification
tasks); Axum 0.8.9 as of November 2026 (last confirmed September 2026).
No patch: the existing build-time re-check requirements already cover these,
and inventing a fresher confirmation without a primary source would violate
AGENTS.md rule 1. Deferred to build time, with reasons recorded here.

## Phases 4–10 — read-only observations (no edits made)

- CHECKLIST checkbox counts are lower than subtask counts for P6 (20 items vs
  30 subtasks), P7 (22 vs 32), P8 (19 vs 30). Checklist items group multiple
  subtasks; this is not necessarily a defect. Flagged for the parallel
  reviewers who own those phases — no action taken here.
- No Phase 4–10 content was changed. Any finding touching those phases from
  the Phase 0–3 side is recorded above as a cross-reference (P0.08→Phase 4,
  P0.12→Phase 4, P3.28→Phase 4), not as an edit to their files.

## Cross-checks that held (what was re-verified and found correct)

Counts all check out against the report: 26 §15.3 events; 16 §15.2 REST
resources; 14 §10.1 entities; 14 command states (8 forward + 6
terminal/fallback); 11 §5.3 plugin categories; 10 §17.4 threats; 10 §17.5
safety invariants; 16 §17.3 controls; 7 §16.2 tenancy rules; 6 §16.1 storage
rows; 12 §20.1 workspaces; 9 §20.2 display rules; 6 §19.1 tracks; 10 §19.2
test layers; 4 control-authority modes (§21.4); §5.7 permission table incl.
never-granted `dispatch.authorize`, `secret.read`, `network.any` prohibited;
WASI 0.2 baseline (§5.4); §21.2 nine recovery capabilities; §21.4 six-row
distribution table. P0.24 §24 step-to-stage mapping defensible. P0.28
gate/escalation logic consistent with §23 Stage 0 exit gate. No solver-class
conflations found in the scope ledgers (HiGHS LP/QP/MIP, Clarabel convex conic,
IPOPT local, AC power flow ≠ AC-OPF all stated correctly). No safety,
certification, or protection-grade claims found. No hallucinated version
claims beyond the two corrected here (R1, and the I2 unresearched binding).

## Applied vs deferred log

**Applied in this pass (all 30 findings have a patch or a recorded closure):**
F1, F2, F3, F4, F6, G2, G3, G5, G7, G8, H1, H2, H4, H5, H7, H13, I1, I2, I3,
I4, I6, I8, I12, R1, R2, R3, R4, R5, R6. (G4 and G9 are verification closures,
not patches — the ledger text was already correct and the RESEARCH qualifier
is what changed.) Two new subtasks added: P2.31, P3.31. No finding was
deferred as "fix later" — R7 items are pre-existing build-time re-checks, not
deferred fixes, and are logged above with reasons.

**Decisions taken during the review (for the parent):**
- The foundations §6.4 as-of replay gap became a new subtask (P2.31), not a
  P2.24 amendment — P2.24 is simulator replay; versioned corrections + as-of
  query semantics are a distinct storage requirement.
- The P1.27 license gate was added now (cargo-deny in CI), not recorded as a
  P1.28 follow-up — an inventory without an enforcing gate does not satisfy
  §22.2.
- No new P0 or P1 subtask was needed; all P0/P1 items are amendments to
  existing subtasks.

## Before / after counts (grep-verified)

Subtask headings: `^##+ P[0-9]+\.[0-9]+`. Checkboxes: `- [ ]` (P3 uses one
checkbox per subtask, on the Objective line; P0–P2 use multiple acceptance
checkboxes per subtask).

| File | Subtasks before | Subtasks after | Checkboxes before | Checkboxes after |
|---|---|---|---|---|
| `ledger_phase_0.md` | 30 | 30 | 114 | 118 (+4: P0.07, P0.08, P0.12, P0.26) |
| `ledger_phase_1.md` | 32 | 32 | 121 | 123 (+2: P1.17, P1.27) |
| `ledger_phase_2.md` | 30 | **31** | 124 | 133 (+9: P2.05, P2.06, P2.12, P2.20, P2.28, P2.31×4) |
| `ledger_phase_3.md` | 30 | **31** | 30 | 31 (+1: P3.31 objective checkbox) |
| **Total P0–P3** | **122** | **124** | **389** | **405** |

Files changed (scope only — `git status` shows nothing outside `ledger/`):
`phase-00-product-freeze/ledger_phase_0.md`, `CONTEXT.md`, `RESEARCH.md`;
`phase-01-rust-foundation/ledger_phase_1.md`, `RESEARCH.md`;
`phase-02-asset-telemetry/ledger_phase_2.md`, `RESEARCH.md`, `CHECKLIST.md`,
`PLAN.md`; `phase-03-flexibility-scheduling/ledger_phase_3.md`,
`RESEARCH.md`, `CHECKLIST.md`, `PLAN.md`.

## Verification tools (AGENTS.md §6)

Run after all patches, from `~/workspace/energy-flexcore` with `python3`
(`python` is not on PATH in this VM):

```
$ python3 tools/verify_docs.py
report: docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
(exit 0)

$ python3 tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
(exit 0)
```

## Remaining Critical / High items

None. Highest remaining severity is Medium, and every Medium in this review
has a patch applied in this pass. What remains open by design (not by
deferral): the R7 build-time re-checks; D1–D6 (still OPEN per DECISIONS.md);
the Rust HiGHS binding version (P3.17 / RESEARCH §6 task 6, to be resolved at
implementation time); the P1547 publication date (P0 RESEARCH §2).
