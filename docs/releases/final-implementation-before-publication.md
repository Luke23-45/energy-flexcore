<!-- Final pre-publication implementation record for Neryva FlexCore v1.0.0.
     STATUS: DRAFT-PROPOSED — written 2026-11-19 at HEAD b8012e7. NOT a
     release, NOT published, NOT approved. Every status claim below cites
     its source; where the source is a test log, the batch and log name
     are given. Nothing in §5 is decided: every feature there is PROPOSED
     and needs Krishna's approval before any implementation starts.
     Companion: docs/releases/v1.0.0-release-notes-draft.md (P10.26,
     2026-11-14) — this document records what is implemented and what
     remains to be done; the release-notes draft is the public-facing
     wording and its known-limitations sections still govern. Where the
     two disagree, the release-notes draft's NOT-QUALIFIED verdicts
     stand unless new evidence in §3 closes them. -->

# Final Implementation Before Publication — Neryva FlexCore v1.0.0

**Document:** `docs/releases/final-implementation-before-publication.md`
**Version:** draft-proposed, 2026-11-19. **Head:** `b8012e7` (pushed
2026-11-19; working tree clean). **Test run:** STOPPED by Krishna's
order 2026-11-19 00:32 NPT ("push the changes. stop the things.").
**Ledger:** v1.0 FINALIZED 2026-11-07
(`ledger/MASTER_LEDGER.md`, `ledger/FINALIZATION.md`).

---

## 1. Purpose and scope

### 1.1 What "final before publication" means

This document is the last implementation record before the v1.0.0
publication decision (ledger P10.27/P10.28). It answers three
questions, in order:

1. **What is implemented?** (§2 — the ledger, phase by phase, with
   evidence.)
2. **What verification is still missing?** (§3 — each item with its
   exact unblocker.)
3. **What else should we build before publishing?** (§5 — proposed
   features, each PROPOSED, none decided.)

The publication gate itself is §6: an ordered checklist. Nothing in
this document publishes anything. Publication executes only on a go
decision (P10.27, decision NOT MADE), exactly as the release-notes
draft §9 records.

### 1.2 In scope

- All 381 ledger subtasks (Phases 0–10) and their implementation state
  at HEAD `b8012e7`.
- Patch A (Postgres `TelemetryReader`) and Patch B (Ed25519 signed
  envelopes), implemented 2026-11-18.
- The 2026-11-18/19 test run (batches 0–66), logged in
  `~/workspace/flexcore-test-debug/BATCH_LOG.md` (outside this repo).
- Decisions D1–D6, the verified cursor collision, and the `fmt` drift.
- Proposed pre-publication features (§5), each traced to a verified
  gap or finding.

### 1.3 Explicitly NOT in scope

- **Phases 11–20** (`docs/phases-11-20/`) are post-v1 by design.
  The gap analysis verdict is in §5.6: none of it is
  publication-blocking for a simulation-only v1.0.0, and its own §10
  rule forbids starting implementation before a green build without
  the owner's explicit exception record. Market participation,
  EV/fleet, protocol stacks, forecasting, certification, and scale
  architecture all stay post-v1.
- **Any field deployment, pilot, or operator-approved control.**
  Simulation-only until v1.0.0 operator-approved control (AGENTS.md
  §3 rule 5). No command has ever been issued to a device by this
  platform (release-notes draft §2).
- **Anything that contradicts the foundations spec.**
  `docs/foundations/` is canonical and wins on conflict (AGENTS.md
  §2). This document edits nothing under `docs/foundations/`.

### 1.4 How to read the status words in this document

| Word | Meaning here |
|---|---|
| **Written** | Code exists in the tree at `b8012e7`. Nothing more is claimed. |
| **Test-verified** | A named batch in `BATCH_LOG.md` ran the tests green (counts given). |
| **Diagnosed, fix applied, re-run pending** | The failure was root-caused and patched, but the proving run has not happened. This is NOT "passing". |
| **PROPOSED** | §5 only. Needs Krishna's approval. Not decided, not started. |
| **OPEN / SCHEDULED** | A decision or gate owned by a human, not by code. |

---

## 2. The ledger — phase by phase, with evidence

Ledger v1.0: 11 phases, **381 subtasks** (P0 30, P1 32, P2 31, P3 31,
P4 37, P5 33, P6 34, P7 43, P8 39, P9 41, P10 30), finalized
2026-11-07 with **170 findings — 0 Critical / 24 High / 95 Medium /
51 Low** (`ledger/MASTER_LEDGER.md`). Implementation was written
across commits `307fcfb`, `316a034`, `74d56f4`, `b8012e7`
(`b8012e7`: 452 files, pushed 2026-11-19).

**Reading note — test batches verify crates, not ledger phases.**
The test run (batches 44–66) runs `cargo test` per crate. The
"nearest test evidence" column below maps each phase to the crate
batches that exercise its code; it is NOT a phase-level pass. No
ledger phase has a phase-level test verdict, and none is claimed
here.

| Phase | Subtasks | Ledger ticks | Implementation at `b8012e7` | Nearest test evidence |
|---|---|---|---|---|
| 0 — Product freeze | 30 | 31/34 lines ticked in `CHECKLIST.md`; **3 open** | Written. Scope, vocabulary, contracts, threat model, ADRs recorded. D1–D3 still open (§4). | No code to test. Evidence is the decision records (`ledger/phase-00-product-freeze/03_decision_records.md`). |
| 1 — Rust foundation | 32 | 30/32 ticked; **2 open** | Written. Cargo workspace, DDD/hexagonal separation, Axum API, Postgres + tenant context, NATS JetStream + outbox. | `cargo check --workspace` GREEN, Batch 31 (`check-41.log`, exit 0). Workspace compiles; every crate below ran. |
| 2 — Asset + telemetry | 31 | **31/31 ticked** — see the honesty note below | Written, **plus Patch A and Patch B** (see §2.1). Registry, canonical telemetry model, simulator source, quality/stale detection, current-state + historical views, deterministic replay. | B45 ports 70; B62 postgres non-gated 120 (48 lib + 72 int); B66 gated 48 lib green, 68 int failures diagnosed + fixed, **re-run pending** (§3). |
| 3 — Flexibility + scheduling | 31 | 31/31 ticked | Written. BESS/load/EVSE capability models, flexibility envelopes, LP/MILP/QP scheduler, solver result/diagnostics contract, schedule comparison. Solver classes as specified: HiGHS LP/QP/MIP, Clarabel convex conic — never conflated (AGENTS.md §3 rule 4). | B46 domain 484; B53 optimization-api 148. |
| 4 — Grid feasibility + dispatch | 37 | 37/37 ticked | Written. Grid-model provider, AC power-flow validation (AC-PF, not AC-OPF), voltage/thermal/transformer constraints, command lifecycle + audit, human approval + signed commands, simulation-only dispatch adapter. | B51 grid-api 160; B52 kernel 283; B64 control-plane 337. |
| 5 — Operator console + simulator | 33 | 33/33 ticked | Written. Fleet/sites/assets/grid/schedule/dispatch workspaces, simulator adapters + scenario replay, failure injection, operator audit + reports. **Console scaffold never built** — the release-notes draft §1 records "no candidate console exists to walk". | B56 simulator 133; B65 sim-runner 165; B64 control-plane 337. Console: no test evidence exists. |
| 6 — Edge + protocol | 34 | 34/34 ticked | Written. Rust edge agent, outbound mTLS enrollment, OpenEMS integration, telemetry buffering/recovery, protocol adapters, conformance tests. | B61 edge-agent 565; B59 protocol adapters 357; B60 NATS adapter 21. |
| 7 — Plugin SDK | 43 | 43/43 ticked | Written. Manifests + schema, capability seams, built-in Rust provider registry, profiles/bundles, Wasmtime/WIT sandbox, Python gRPC worker template, conformance suite. | B58 plugin-host 170; B54 protocol-ports 17. |
| 8 — Advanced coordination | 39 | 39/39 ticked | Written. ADMM/DMPC provider, GNN warm-start/screening, MARL simulation/shadow worker, dynamic simulator integration, benchmark paper/report. Research-track only; IPOPT local, never a global-optimality claim. | B55 research 186. **P8 F-1 (HIGH) still OPEN**: unguarded `torch.load(...)` at the GNN artifact load blocks with-framework activation (release-notes draft §5; unreachable today — guarded import, no framework installed, no artifact exists). |
| 9 — Pilot qualification | 41 | 41/41 ticked | Written. Customer-specific integration + network design, security assessment, OT/IT boundary review, commissioning plan, HIL + field acceptance tests, operational procedures. | B48 audit 47; B49 authz 94; B61 edge-agent p9_* tests (incl. `pilot_rejection_proof_p9_31`). **No pilot exists — there is no field evidence of any kind** (release-notes draft §4, LR-001..LR-006). Ticks record written preparation, not qualification. |
| 10 — v1.0.0 release hardening | 30 | 30/30 ticked | Written. Release-notes draft exists (P10.26, 2026-11-14 — see §2.2, now partly stale). Deployment/upgrade model drafted with every site value PLACEHOLDER. | **No P10 gate has passed.** CI gate UNRUN (P10.18); benchmark pack EMPTY, no baseline run (P10.09/P10.10); security review NOT APPROVED (P10.25); operator mode NOT QUALIFIED (§6 item 8). The test run in §3 does not close any P10 human gate. |
| **Total** | **381** | **P0 3 open, P1 2 open, P2–P10 fully ticked** | | **Non-gated total: 4,453 tests passed, 0 failed (batches 44–65)** — summed from the batch headers below. |

### 2.1 The two patches (post-ledger, pre-publication)

The 2026-11-12 review of `d6eeddc` found two HIGH gaps; both are
fixed in the tree at `b8012e7`. Plan:
`ledger/PATCH_PLAN_HIGH_GAPS.md`. Handovers:
`ledger/phase-02-asset-telemetry/HANDOVER_PATCH_A.md`,
`HANDOVER_PATCH_B.md`.

- **Patch A — Postgres `TelemetryReader`.**
  `crates/adapters/postgres/src/telemetry_reader.rs` (new),
  keyset pagination on the unique sort key, RLS defence-in-depth
  (`FORCE ROW LEVEL SECURITY`), `DISTINCT ON` latest-state.
  Test evidence: B62 non-gated 120 green; gated RLS tests exist in
  `crates/adapters/postgres/tests/telemetry_reader_rls.rs`
  (queries run **as the `flexcore_query` role**, including the case
  with the tenant predicate deliberately removed — the RLS policy,
  not the predicate, is the backstop). Gated re-run pending (§3).
- **Patch B — Ed25519 signed envelopes.**
  `crates/flexcore-ports/src/source_auth.rs` (new),
  `crates/adapters/postgres/src/source_keys.rs` (new), migration
  `0026_source_signing_keys_p2_07.{up,down}.sql` (new),
  `deploy/compose/nats-server.conf` + `.env.example` (new), worker
  config `FLEXCORE_SOURCE_AUTH_MODE` (default `enforced`) and
  `FLEXCORE_SOURCE_AUTH_WINDOW_SECS` (default 300). Domain-separated
  canonical bytes `flexcore-telemetry-v1`, SHA-256 digest,
  per-source monotonic sequence + persisted watermark, ±5-minute
  window. Test evidence: B63 worker non-gated 101 green (signed
  envelope path exercised in `source_auth_pipeline` /
  `source_auth_transport`); gated replay/keys/envelope tests in
  `crates/adapters/postgres/tests/source_auth_*.rs`, re-run pending
  (§3). `trusted_source_id` remains an allowlist only, explicitly
  not authentication. D6 stays OPEN.

### 2.2 Honesty note — what the ticks do and do not mean

1. **P2 was ticked against the wrong code.** The Phase 2 checklist
   reads 31/31, but the ticks were made by the wave implementation
   against a `FakeTelemetryReader` and an `authenticate_source`
   recorded as STUB (`SOURCE_AUTH_STATUS` in `apps/worker/src/
   ingestion.rs`, pre-patch). Patch A and Patch B replaced the fake
   and the stub with real code **without re-ticking any box** —
   there was nothing left to tick. A ticked P2 box therefore proves
   the subtask was *attempted in writing*, not that the real
   Postgres reader or the real authenticator was verified. The
   verification for P2 is the Batch 62/63/66 evidence in the table
   above, not the ticks.
2. **P0 and P1 are not fully ticked.** 3 of 34 (P0) and 2 of 32 (P1)
   checklist lines remain open. They are the Phase 0 decision
   items (§4), not code.
3. **No new ledger boxes were ticked for the patches.** The patches
   are recorded by dated handovers, not by ledger ticks. If a future
   ledger revision re-ticks P2, it must cite the Batch 66 gated
   re-run (§3 item 1), not the patches' existence.
4. **The release-notes draft is partly stale.** Written 2026-11-14,
   before the test run, it states "no Cargo.lock" and "no test suite
   in this repository has been executed". Both are now false:
   `Cargo.lock` exists (generated 2026-11-18, committed in
   `b8012e7`), and 4,453 non-gated tests have been executed green.
   The draft must be revised before publication (§6 item 7). Its
   NOT-QUALIFIED verdicts (operator mode, benchmark, security
   review) are **not** superseded — no new evidence closes them.

### 2.3 Test evidence — batch table (batches 44–66)

Source: `~/workspace/flexcore-test-debug/BATCH_LOG.md`. Counts are
lib + integration as recorded in each batch header. `cargo check
--workspace` first went green at Batch 31 (`check-41.log`, exit 0).

| Batch | Crate / scope | Passed | Failed |
|---|---|---|---|
| 31 | `cargo check --workspace` | 0 errors | 0 |
| 44 | flexcore-contracts | 104 | 0 |
| 45 | flexcore-ports | 70 | 0 |
| 46 | flexcore-domain | 484 | 0 |
| 47 | flexcore-application | 770 | 0 |
| 48 | flexcore-audit | 47 | 0 |
| 49 | flexcore-authz | 94 | 0 |
| 50 | flexcore-events | 41 | 0 |
| 51 | flexcore-grid-api | 160 | 0 |
| 52 | flexcore-kernel | 283 | 0 |
| 53 | flexcore-optimization-api | 148 | 0 |
| 54 | flexcore-protocol-ports | 17 | 0 |
| 55 | flexcore-research | 186 | 0 |
| 56 | flexcore-simulator | 133 | 0 |
| 57 | flexcore-storage | 80 | 0 |
| 58 | plugin-host | 170 | 0 |
| 59 | protocol adapters (8) | 357 | 0 |
| 60 | adapters/nats | 21 | 0 |
| 61 | edge-agent | 565 | 0 |
| 62 | postgres adapter (non-gated) | 120 | 0 (41 gated blocked) |
| 63 | worker (non-gated) | 101 | 0 (11 gated blocked) |
| 64 | control-plane | 337 | 0 |
| 65 | sim-runner | 165 | 0 |
| **44–65 total** | | **4,453** | **0** |
| 66 | postgres adapter (gated) | lib 48/48; int 72 passed, **68 failed in run 2** | 68 — all diagnosed, 13 fixes A–M applied, **re-run NOT done** |

Notable implementation bugs the test run caught and fixed (not
test-only fixes): OCPP empty `[]` frame refused under the wrong
name (Batch 59, `framing.rs`); NATS DLQ off-by-one token index
sending quarantine copies to the catch-all stream (Batch 60,
`topology.rs`); edge `commit_uploaded` checking the wrong queue
front so the drain→commit flow could never commit (Batch 61,
`audit_buffer.rs`); edge clock-quality median truncating an
even-count mean toward zero (Batch 61, `clock_quality.rs`) — this
last one **closes recorded finding F-P10.07-5** (release-notes
draft §5), which asserted a `Degraded` outcome the old median
arithmetic could not produce.

---

## 3. Remaining verification before publication

Each item states its exact unblocker. Nothing here is "almost
done" — an item is done only when its proving run is green.

| # | Item | Status | Exact unblocker |
|---|---|---|---|
| V-1 | **Gated postgres re-run** — `cargo test -p flexcore-postgres-adapter` on a fresh DB, all 21 integration targets (140 int tests: 72 passed in run 2, 68 failed, 13 fixes A–M applied). Fixes compile (`cargo check -p flexcore-postgres-adapter --all-targets` exit 0); lib 48/48 green. | BLOCKED | **Postgres 18 re-provision, needs Krishna's approval for `apt-get install postgresql-18`.** The VM reset wiped Postgres 18 (user, cluster, package, servers). The approval prompt timed out twice; the PGDG index also failed to fetch via the proxy twice ("Could not wait for server fd") — the install may fail even with approval. PG 16 is NOT a substitute (schema uses PG 18 `uuidv7()`, migration 0002). Re-provision commands are in `BATCH_LOG.md` Batch 66. |
| V-2 | **Worker gated tests (11)** — `source_auth_pipeline` (8) + `source_auth_transport` (3), with `FLEXCORE_TEST_NATS_URL=nats://127.0.0.1:4222` added. | BLOCKED | Same as V-1 (needs a live Postgres). NATS 2.15.2 was restored and running (PID 14465, JetStream up, port 4222) but was stopped with the rest of the test run on Krishna's order; restart from `~/workspace/flexcore-test-debug/gated/nats/`. |
| V-3 | **NATS spelling probes** — JetStream encryption key spelling; TLS 1.3 minimum-version key spelling. | **CLOSED 2026-11-19** | None. Runtime probes against nats-server 2.15.2 (`BATCH_LOG.md` tail): `key:` and `encryption_key:` **both** enable ChaCha20-Poly1305 — the config's `key:` form is correct. `min_version: "1.3"` in the `tls {}` block is valid and enforced (TLS 1.2 gets a `protocol_version` alert; TLS 1.3 handshakes fine). Note for future probes: NATS sends plaintext `INFO` before the TLS handshake — a raw TLS client that skips INFO fails with `WRONG_VERSION_NUMBER`, which is normal, not a broken server. |
| V-4 | **Finish step** — `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `python3 tools/check_dependency_rule.py`, `python3 tools/verify_docs.py`. | NOT RUN | V-1/V-2 first (fixes from the gated re-run may change the results). A static-checks run was started and then cancelled by Krishna's stop order; its partial `fmt --check` result (exit 1, pre-existing drift) is not a completed check. |
| V-5 | **Warning triage** — clippy warnings pending the final clippy step. | NOT RUN | V-4. Counts differ by scope and are **unverified as a single number** until clippy runs: 68 warnings at the Batch 31 workspace check; 95 pre-existing warnings in the postgres-adapter scope at Batch 66; 467 cited from `check-164`. Do not quote any one number as "the warning count" before V-4. |
| V-6 | **E_STORE_UNAVAILABLE trio** — 3 gated tests failed with opaque backend-unavailable (`duplicate_primary_key_is_classified_exact_or_conflicting`, `tenant_scoping_rows_of_tenant_b_never_appear_in_tenant_a_query`, `rls_table_owner_with_no_tenant_set_sees_zero_rows`). No fix applied. | OPEN, pending evidence | The V-1 re-run decides. Hypothesis: role-race fallout or connection pressure (default `max_connections=100` vs ~21 binaries × several 10-connection pools). If it persists after V-1, check `max_connections` and the server log. |
| V-7 | **Fix M verification** — the partition-PK-name matcher (`telemetry_pkey` OR `telemetry_y*_pkey` in `crates/adapters/postgres/src/telemetry.rs`) is verified only by the re-run. | OPEN, pending evidence | V-1. If the corrections extended-PK test still returns `E_STORE_UNIQUE`, the reported name differs and the matcher must be widened (telemetry has no other unique constraints, so any 23505 on that insert is the PK). |
| V-8 | **Same-sequence race regression test (moved from the first draft's §5 F-01)** — the behaviour already exists: `advance_source_sequence` (`crates/adapters/postgres/src/source_keys.rs:482`) serializes per source with `pg_advisory_xact_lock` and returns `Stale` when `sequence <= MAX(sequence)`, so concurrent same-sequence submissions resolve to exactly one `Advanced`. No test proves it. | OPEN | Write the regression test (`crates/adapters/postgres/tests/source_auth_replay.rs`, gated): two concurrent submissions of the same (`source_id`, `sequence`) produce exactly one `Advanced` and the rest `Stale`. Run it in the V-1 re-run. |
| V-9 | **Postgres `audit_records` chain upgrade (moved from the first draft's §5 F-04)** — the audit-crate chain is already HMAC-SHA-256 (`crates/flexcore-audit/src/chain.rs`, `AUDIT_CHAIN_ALGORITHM = "hmac-sha256-chain-v1"`); the postgres adapter's `audit_records` chain is still test-grade FNV-1a (`crates/adapters/postgres/src/audit.rs:33`, LR-203). This is a hardening upgrade, not a new capability. | OPEN | Krishna's call (P11.15 deployment requirement): wire the adapter chain to the audit-crate HMAC-SHA-256 construction, with a migration note for how pre-upgrade chains verify (old chains keep their recorded digest algorithm — no silent re-hash). |
| V-10 | **Remove the remaining `.expect()` fail-fast (moved from the first draft's §5 F-03)** — `crates/flexcore-ports/src/source_auth.rs:161`, inside `push_length_prefixed` on the signing path. | OPEN | Implementation time; no decision needed. `push_length_prefixed` returns a typed error on over-long input; callers handle it as a refusal; crate-wide grep confirms no `.expect()` / `.unwrap()` remains in non-test production code in `flexcore-ports`. |

V-8–V-10 are open pre-publication items carried from the §5 revision (a missing test, a hardening upgrade, a cleanup) — not verification runs, but they gate publication the same way.

Watch items carried from the test run (not blockers, but a future
change must respect them): the advisory-lock key `20261118` is
shared by every `ensure_test_roles` copy and every partition setup
— a new copy must use the same key; `with_tenant` in
`migration_compat_p10_13.rs` now commits on success (a test
relying on rollback would silently persist — none does today).

---

## 4. Decisions required before publication

Each decision is stated as an exact question with its options and
the consequence of each option. None is decided in this document.

### 4.1 Product decisions D1–D6

Source: `.agent/DECISIONS.md`; owners and venues recorded in
`ledger/phase-00-product-freeze/03_decision_records.md` §1–§6. All
six are **SCHEDULED** (owner + venue recorded; named person + date
assigned at the Stage 0 gate meeting — P0.28). D1–D3 are the Phase 0
blocking decisions (ledger FINALIZATION.md).

| Decision | Exact question | Options and consequence |
|---|---|---|
| **D1** — Target jurisdiction and market | Which jurisdiction and market is v1.0.0 built for? | Decides interconnection, market adapter, data residency, and liability. No default exists; the 0.x line cannot be published for a market without this. |
| **D2** — First portfolio and service | Which first portfolio and service does v1.0.0 serve? | Determines asset mix, baseline method, and acceptance criteria. Baseline-method question is OPEN pending D1 input. |
| **D3** — Network class and model fidelity | Which network class does the grid model cover? | Balanced radial, unbalanced three-phase, and meshed transmission need different models and solvers. The 0.x balanced-radial default is a RECOMMENDATION, not a decision. |
| **D4** — License and dependency audit | Under which license is v1.0.0 released, and are all dependencies compatible? | Core license OPEN; Apache-2.0 recommended only. Re-closure for the release artifact set is NOT CLOSED (release-notes draft §4; P10.19 F-1..F-7: no SBOM, no license scan has ever run). Publication without D4 ships code with an undecided license. |
| **D5** — First protocol and asset mix | Which protocol adapter and asset mix is the pilot's? | Selects the pilot adapter and conformance tests. OpenEMS priority is not a decision. |
| **D6** — Cyber and OT boundary review | What is the cyber/OT boundary for the first site? | Site-specific; not derivable from the architecture alone. Required before any read-only pilot. Every scope addition beyond a signed D6 is a go blocker (release-notes draft §4). |

### 4.2 Same-millisecond telemetry cursor collision

**Exact question:** two legal telemetry rows sharing
(`event_time`, `asset`, `quantity`) with different
`source_sequence` produce the identical frozen cursor
`v1:{millis}:{asset}:{quantity}` (port contract
`crates/flexcore-ports/src/telemetry_read.rs`, "DECIDED ONCE"). A
page boundary between them would lose one row. Do we (A) extend
the cursor/sort key with `source_sequence`, or (B) declare
same-millisecond same-asset same-quantity pairs out of scope?

- **Option A** — extend the cursor with `source_sequence`. This is
  a **frozen-contract change**: the cursor format, the sort key,
  the reader, and the tests change together. Cost: a contract
  revision and re-verification of the pagination tests. Benefit:
  pagination is correct for all legal rows.
- **Option B** — declare such pairs out of scope. Cost: none in
  code. Consequence: the contract must record the exclusion
  explicitly, and any source that can emit two rows with the same
  millisecond, asset, and quantity is outside the supported
  envelope. The collision is VERIFIED, not hypothetical: the
  Batch 66 reader test data had to be moved +1h to avoid it
  (BATCH_LOG.md fix 12).

### 4.3 `cargo fmt` drift

**Exact question:** `cargo fmt --check` fails workspace-wide on
pre-existing drift in untouched files. Do we (A) run `cargo fmt`
repo-wide before publication, or (B) leave the drift?

- **Option A** — format repo-wide. Cost: a large formatting diff
  across files the test run never touched; every open patch and
  review must rebase over it. Benefit: `fmt --check` passes and
  the finish step is clean.
- **Option B** — leave the drift. Cost: the finish step cannot
  claim `fmt --check` green; the drift must be recorded as a known
  deviation. Benefit: no churn in untouched files.

---

## 5. New features proposed before publication

Every feature below is **PROPOSED** — it needs Krishna's approval
before any implementation starts. Nothing here is decided, and
nothing here starts before the §10 gate: gap analysis §10.1
forbids phases 11–20 implementation before a green build without
the owner's explicit exception, so these features queue behind
the gated suite (V-1) unless Krishna grants that exception in
writing.

Read this section with one fact in mind: the capabilities in
`docs/phases-11-20/` already exist in this repo as **written-unrun
designs** — the phase checklists are closed on written artifacts
(`docs/phases-11-20/phase-*/CHECKLIST.md`; protocol crates in
`crates/adapters/protocol-*`; `apps/control-plane/src/oidc.rs`,
`stepup.rs`, `webhook_service.rs`, `idempotency.rs`). What §5
proposes is the **live half** each design prepares for: the wire
transport, the live delivery, the real IdP, the measured model —
the capability an operator or integrator actually uses. For every
feature, the existing design is cited alongside what is missing,
so nothing already built is re-proposed as new. No item in §5 is
a test, a build step, a checklist, a fix, or a cleanup; those live
in §3/§4/§6.

Each feature is independently shippable: one capability, one
owning phase, its own acceptance criteria. A feature is done only
when all of its criteria hold; there is no partial credit across
features.

### F-01 — Live IEEE 2030.5 server transport (PROPOSED)

- **Source:** `crates/adapters/protocol-20305/src/lib.rs:16`
  (the `AggregatorTransport` seam — "UNRUN by design — no
  transport ships in this repo"); P14.02; gap analysis §4 (IEEE
  2030.5 server: Absent).
- **What is new:** the 2030.5 AggregatorServer answers real
  utility requests over HTTPS with mTLS client-certificate
  validation. Today the server skeleton exists and no request has
  ever reached it.
- **Owning phase:** Phase 14 (P14.02).
- **Acceptance criteria:** (1) a 2030.5 client completes a
  DERControl flow against the live server with mTLS; (2) a
  request without a valid client certificate is refused,
  fail-closed.
- **Status:** PROPOSED.

### F-02 — Live OCPP CSMS transport (PROPOSED)

- **Source:** `crates/adapters/protocol-ocpp/src/lib.rs:20`
  (the `CsmsTransport` seam — "UNRUN by design — no transport
  ships in this repo"); P17.01/P17.02; P17-E-2 DEFAULT
  edge-agent-hosted depot shape
  (`CsmsPlacement::depot_default`).
- **What is new:** the OCPP CSMS accepts real charge-point
  WebSocket connections. Today the session state machine and
  composer exist and no charge point has ever connected.
- **Owning phase:** Phase 17 (P17.01/P17.02).
- **Acceptance criteria:** (1) a simulated charge point
  (P17.15) completes boot + heartbeat + a charging-profile relay
  against the live CSMS; (2) a charge point offering only
  `ocpp1.6` is refused with the P17-E-1 reason.
- **Status:** PROPOSED.

### F-03 — Live Modbus TCP client (PROPOSED)

- **Source:** `crates/adapters/protocol-modbus/src/lib.rs:25`
  (the transport is a port trait + config types; "NO Modbus stack
  ships"); P14.07; gap analysis §4 (SunSpec Modbus client:
  Absent).
- **What is new:** the Modbus client polls real inverters and
  meters over Modbus TCP. Today the register model and the
  backpressure discipline exist and no device has ever been
  polled.
- **Owning phase:** Phase 14 (P14.07).
- **Acceptance criteria:** (1) a Modbus TCP device answers a
  SunSpec model read through the live client; (2) an unreachable
  device surfaces a named refusal, never a hang.
- **Status:** PROPOSED.

### F-04 — Live OpenADR VTN (PROPOSED)

- **Source:** `crates/adapters/protocol-openadr/src/lib.rs:17`
  ("no server loop"); P14.05; gap analysis §4 (OpenADR VTN:
  Absent).
- **What is new:** the OpenADR VTN serves its REST endpoints —
  a VEN registers, polls events, and reports against the live
  server. Today the event/report/registration models exist and
  no endpoint has ever served.
- **Owning phase:** Phase 14 (P14.05).
- **Acceptance criteria:** (1) a VEN registers and polls an
  event from the live VTN endpoints; (2) event delivery is
  recorded with the event version in force.
- **Status:** PROPOSED.

### F-05 — Live MQTT 5 + Sparkplug B edge uplink (PROPOSED)

- **Source:** `crates/adapters/protocol-mqtt/src/lib.rs:40`
  ("Not an MQTT client. No connect/subscribe/publish I/O, no
  broker"); `crates/adapters/protocol-mqtt/src/transport.rs:182`
  (`MqttUplinkTransport` trait) with `UnwiredUplinkTransport`
  (:202) as the only implementation;
  `crates/edge-agent/src/adapters/edge_to_cloud.rs` composes the
  adapter through the injected transport seam. P14.11.
- **What is new:** the edge agent uplinks telemetry to a real
  broker over MQTT 5 with Sparkplug B birth/data/death
  semantics. Today the topic model, birth/death handling, and
  backpressure exist and no message has ever left the edge.
- **Owning phase:** Phase 14 (P14.11).
- **Acceptance criteria:** (1) the edge agent connects to a
  broker and publishes a Sparkplug B birth certificate plus
  telemetry; (2) a command published to the subscribed topic
  reaches the edge agent; (3) an unclean disconnect publishes
  the death certificate; (4) no secret material is held by the
  client (the never-granted `secret.read` rule — client-id /
  username bindings only).
- **Status:** PROPOSED.

### F-06 — Live CIM/CGMES network-model import (PROPOSED)

- **Source:** P14.12;
  `docs/phases-11-20/phase-14-protocol-device-connectivity/records/44_p14_12_cim_cgmes_import.md`
  ("No profile file was read, no DSO boundary touched, no model
  exchanged");
  `crates/adapters/protocol-cim/src/lib.rs:39-40` ("Not an
  RDF/XML parser. No XML crate, no CGMES profile conformance;
  the parsing is a designed, UNWIRED seam").
- **What is new:** a real CGMES profile file is read and
  imported into a `VersionedNetworkModel` — the grid model the
  optimizer and feasibility checks consume comes from the DSO's
  network model instead of hand-built fixtures. Today the class
  mapping, the completeness gate, and the DER-group registry
  exist and no profile file has ever been read.
- **Owning phase:** Phase 14 (P14.12).
- **Acceptance criteria:** (1) a real CGMES profile file parses
  into a `VersionedNetworkModel` through the grid-api IR; (2)
  the completeness gate refuses the import while any AMBIGUOUS
  object remains; (3) every unmapped class is
  EXPLICITLY_UNMAPPED, never guessed; (4) the 15 declared class
  spellings and attribute names are verified against the CGMES
  profile texts at implementation time — all are currently
  UNVERIFIED.
- **Status:** PROPOSED.

### F-07 — Live enterprise identity: real IdP login, SCIM provisioning, enforced step-up (PROPOSED)

- **Source:** `apps/control-plane/src/oidc.rs` ("no token has ever
  been validated, no JWKS document has been fetched ... no IdP is
  deployed ... `DenyAllResolver` still the SHIPPED DEFAULT ...
  `main.rs` is untouched by this subtask");
  `apps/control-plane/src/stepup.rs` ("no challenge has ever been
  issued or verified, no IdP is deployed (P13-E-3 stays OPEN)");
  P13.01–P13.04; LR-047 (no IdP), LR-048 (step-up UI-only).
  No SCIM service exists in `apps/` or `crates/` — no endpoints,
  no provisioning flow. What exists is the design record (P13.03)
  and a conformance-scenario test slice
  (`apps/control-plane/tests/phase13_scim_role_mapping_tests.rs`,
  written-unrun).
- **What is new:** operators log in with their own IdP (OIDC,
  Keycloak DEFAULT per P13-E-1) instead of dev-auth; the
  `OidcResolver` becomes the composed production path; step-up
  challenges are answered with fresh IdP evidence at the APPROVED
  gate; users and groups are provisioned over live SCIM 2.0
  endpoints. Today the resolver and the step-up protocol exist
  written-unrun and nothing is wired to a real identity provider.
- **Owning phase:** Phase 13 (P13.01–P13.04).
- **Acceptance criteria:** (1) a login with a real IdP token
  yields a `TenantContext` through the composed resolver;
  (2) a token from the wrong issuer/audience is refused;
  (3) a high-impact approval without fresh step-up evidence is
  refused server-side; (4) a SCIM create/deactivate round-trips
  a user and its group→role mapping; (5) dev-auth remains
  non-default and feature-gated.
- **Status:** PROPOSED.

### F-08 — Live integration platform: machine identity, API keys, webhook delivery (PROPOSED)

- **Source:** P13.07 (machine identity), P13.08 (API keys),
  P13.09 (webhooks); `apps/control-plane/src/webhook_service.rs`
  ("The actual HTTP transmission is a PORT ... this batch adds no
  HTTP-client dependency and performs no I/O"). No
  machine-identity or API-key issuance service exists in code
  (the only hits are a policy-file comment and the P13.08
  secret-format record).
- **What is new:** external systems integrate without a human
  login: OAuth2 client-credentials JWTs issued and validated for
  grid-operator and market-adapter clients; per-tenant API keys
  with rotation and last-used evidence; outbound webhooks actually
  delivered over HTTP with HMAC-SHA256 signing, 5-minute
  tolerance, retries, and a dead-letter table. Today the webhook
  bookkeeping and signing exist and nothing is ever sent.
- **Owning phase:** Phase 13 (P13.07–P13.09).
- **Acceptance criteria:** (1) a client-credentials token
  authorizes a scoped call and a revoked client is refused;
  (2) an API key rotates without dropping in-flight callers and
  last-used evidence is recorded; (3) a webhook attempt is
  delivered, signed, and verifiable by the receiver's HMAC check;
  (4) a failing endpoint lands in the dead-letter table after
  the retry budget; (5) webhook secrets never enter the database
  (P11.16 posture).
- **Status:** PROPOSED.

### F-09 — Live observability pipeline: a running Collector, Python instrumentation, firing SLO alerts (PROPOSED)

- **Source:** P11.01–P11.09, P11.20;
  `apps/control-plane/Cargo.toml:42` pins `opentelemetry 0.33.1`.
  Correction to the earlier draft: the OTLP exporter
  construction **is** composed in both mains —
  `apps/control-plane/src/main.rs:67-75` calls
  `otel_telemetry::init_from_env`, which builds the Span and Log
  exporters when `FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT` is set
  (`apps/control-plane/src/otel_telemetry.rs:209-246`), and
  `apps/worker/src/main.rs:143-144` does the same. What has never
  happened is the live half: no Collector has ever run, no
  endpoint has ever been configured, and no span has ever been
  exported (WRITTEN-UNRUN throughout). No OpenTelemetry exists
  in `apps/worker/python/`; the Collector config, alert rules,
  and dashboards are checked in and never executed (Phase 11
  checklist criterion 2).
- **What is new:** the pipeline actually runs — a Collector
  gateway receives the three signals, traces flow end to end on
  a shared `trace_id` (including the Python workers, which need
  OTEL instrumentation added — none exists today), and the
  golden-signal SLOs actually page and ticket when burn rates
  breach. The Rust exporter wiring exists; the live half is the
  running Collector, the configured endpoint, the Python
  instrumentation, and the firing alerts.
- **Owning phase:** Phase 11 (P11.01–P11.09, P11.20).
- **Acceptance criteria:** (1) a command's `trace_id` appears in
  the control-plane spans, the JetStream message headers, and
  the worker logs — including the Python workers (new OTEL
  instrumentation); (2) the Collector gateway receives and
  exports all three signals; (3) a breaching SLO fires the
  recorded multi-burn-rate alert; (4) a stuck-state command
  surfaces on the stuck-state gauge and alarm; (5) no emitted
  log line carries PII or a secret (P11.08 redaction at emit).
- **Status:** PROPOSED.

### F-10 — Live forecasting: vendor weather and price ingestion, evaluated baselines feeding scheduling (PROPOSED)

- **Source:** P15.01–P15.06, P15.16; ports
  `crates/flexcore-ports/src/weather_data.rs` and
  `price_signal.rs` exist; `apps/worker/python/forecast_worker/`
  serves statistical baselines (written-unrun);
  `ForecastPublished` trigger exists in
  `crates/flexcore-application/src/envelope_staleness.rs:123`.
  No vendor integration exists (P15-E-1 weather build-vs-buy and
  P15-E-2 price vendor are OPEN; P15.04: the API token is a human
  gate); no evaluation run exists (P15.05: every result cell
  EMPTY, no skill claimed).
- **What is new:** real forecasts become the scheduler's primary
  input: vendor irradiance/power and day-ahead prices arrive
  with provenance, the statistical baselines are evaluated
  against the persistence baseline, and real forecasts feed the
  flexibility envelopes and rolling horizon — with staleness
  fallback to the persistence baseline. The synthetic fixture
  stays only as the labeled fallback source (P15.16).
- **Owning phase:** Phase 15 (P15.01–P15.06, P15.16).
- **Acceptance criteria:** (1) a vendor weather feed and a vendor
  price feed ingest with provenance; (2) baselines carry MAE /
  RMSE / skill scores from a backtesting run — no skill claimed
  without it; (3) envelopes and the rolling horizon consume a
  real forecast and fall back to the persistence baseline past
  max-age; (4) a forecast artifact is versioned, digest-chained,
  and replayable (P15.06).
- **Status:** PROPOSED. Depends on P15-E-1/P15-E-2 decisions and
  a human-registered vendor token.

### F-11 — Measured BESS degradation and SoH: rainflow counting, cost curve, SoH estimation (PROPOSED)

- **Source:** P15.07–P15.11; `crates/flexcore-domain/src/
  bess_degradation.rs` ("Rainflow extraction is NOT implemented
  — the histogram stores [a] RECORDED SEAM");
  `crates/flexcore-domain/src/bess_model.rs` ("no verified
  degradation model is on record. The cost is a configurable
  LINEAR throughput term, default ZERO ... PLACEHOLDER, not a
  measured degradation law");
  `crates/flexcore-domain/src/bess_soh.rs`
  (`SOH_ESTIMATION_DEFERRAL_STATEMENT`: "No estimation method is
  implemented"). The optimizer already accepts curve data as the
  `C_degradation` input shape
  (`crates/flexcore-application/src/scheduling_objective.rs:154`),
  and the warranty-headroom computation exists
  (`crates/flexcore-domain/src/bess_warranty.rs`) — it has no
  measured counters to compute against.
- **What is new:** dispatch stops pricing battery wear at zero:
  rainflow counting turns SOC series into DOD × mean-SOC
  histograms, a chemistry-parameterized cost curve prices
  throughput into the optimizer, the warranty-headroom
  computation finally receives measured counters, and SoH is
  estimated on-platform instead of only ingested from the BMS.
- **Owning phase:** Phase 15 (P15.07–P15.11).
- **Acceptance criteria:** (1) a recorded SOC series yields the
  pinned rainflow histogram; (2) the cost curve changes the
  optimizer's `C_degradation` total vs the zero placeholder on
  the same scenario; (3) warranty headroom computed from measured
  counters falls as throughput consumes the allowance;
  (4) an SoH estimate carries provenance and a staleness bound,
  and never overrides a BMS-reported SoH without recording the
  disagreement (weakest-provenance merge,
  `crates/flexcore-domain/src/bess_degradation.rs:244`).
- **Status:** PROPOSED. Depends on P15-E-3 (chemistry coverage:
  LFP/NMC first).

### F-12 — Secure OTA update distribution: live bundle producer and staged rollout (PROPOSED)

- **Source:** P18.02–P18.06; `crates/edge-agent/src/
  update_bundle.rs` ("An update bundle has never been produced,
  signed..."); the P9.34 update seam and verifier exist in
  `crates/edge-agent/src/update_verification.rs`, with nothing
  behind them. Gap analysis §7 (TUF/SUIT).
- **What is new:** edge devices can actually be updated: signed
  update bundles are produced from a TUF-shaped metadata
  repository, the control plane rolls them out in cohorts with
  abort thresholds, and devices report download → verify →
  install → status with A/B auto-rollback. Today the edge agent
  can verify an update and no update has ever existed to verify.
- **Owning phase:** Phase 18 (P18.02–P18.06).
- **Acceptance criteria:** (1) a bundle is produced, signed, and
  verified by the edge agent's real verifier; (2) a tampered
  bundle is refused; (3) a canary cohort receives the update and
  a failing cohort aborts the rollout; (4) a failed install
  rolls back to the previous slot and reports it; (5) every
  attempt lands in the audit trail (P18.07).
- **Status:** PROPOSED. Key ceremonies are human gates (P18.03).

### F-13 — ISO 15118 Plug & Charge certificate management (PROPOSED)

- **Source:** P17.12 (phase-gated design only — no
  implementation);
  `docs/phases-11-20/phase-17-ev-fleet-charging/records/41_p17_12_pnc_certificate_management_design.md`
  §3 (the five-condition ungate checklist, all NOT-STARTED);
  gap analysis §7 ("phase-gated because PKI is the last
  commercial barrier [S-128][S-130][S-125]").
- **What is new:** an EV can plug in and charge without a card
  or app: the V2G Root CA trust store, OCPP certificate flows
  (Get15118EVCertificate, CertificateSigned, InstallCertificate),
  and OCSP validation let the charge point prove the vehicle and
  bill the contract behind it.
- **Owning phase:** Phase 17 (P17.12).
- **Acceptance criteria:** (1) a vehicle certificate is issued,
  installed, and validated against the trust store; (2) a revoked
  certificate fails OCSP and charging is refused; (3) ALL FIVE
  ungate conditions in record 41 §3 are met before
  implementation starts — a named PKI operator; trust-anchor
  distribution agreed; an OCSP service available at fail-closed
  reliability; the OCA-text verification pass over every
  UNVERIFIED item; D5 closed naming OCPP and the edition. None
  may be skipped. (P14.03 is the design + operations record that
  frames these conditions — it explicitly forbids standing up a
  CA in this program — it is not itself the gate.)
- **Status:** PROPOSED, phase-gated on the five-condition ungate
  checklist (records/41_p17_12 §3).

### F-14 — Dynamic operating envelopes from live DSO feeds (PROPOSED)

- **Source:** P14.15; `crates/flexcore-domain/src/doe_envelope.rs`
  (envelope types exist); no DOE data reaches the optimizer —
  no reference to `doe_envelope` exists in
  `crates/flexcore-application/src/` or
  `crates/flexcore-optimization-api/src/`. Gap analysis §7
  (Project EDGE, SAPN Flexible Exports, CSIP-AUS [S-157][S-158]
  [S-159][S-160]).
- **What is new:** the optimizer respects the network's live
  limits: per-connection-point import-export envelopes arrive
  from the DSO with issue timestamps and expiry, feed the
  optimizer as constraint data, and lapse to the static limit
  when the feed goes stale — instead of dispatch assuming the
  static limit is the whole truth.
- **Owning phase:** Phase 14 (P14.15).
- **Acceptance criteria:** (1) a live envelope constrains a
  dispatch that the static limit would have allowed; (2) an
  expired envelope reverts to the static limit and the reversion
  is telemetered; (3) a breach or curtailment is recorded with
  the envelope version that was in force.
- **Status:** PROPOSED.

### F-15 — Live FDO-pattern zero-touch onboarding (PROPOSED)

- **Source:** P18.09;
  `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/plan.md`
  §2 row P18.09 (FDO-pattern late binding: ownership-voucher
  analog, rendezvous service seam, claim flow for
  factory-provisioned devices; the manual provisioning path
  (P9.33) stays the default and the fallback);
  `crates/edge-agent/src/zero_touch.rs` (the shipped rendezvous
  seam `NoRendezvousConfigured` refuses EVERY lookup; "no device
  is claimed; no voucher exists; no rendezvous...").
- **What is new:** a factory-provisioned device onboards itself
  — it claims through the rendezvous service with its ownership
  voucher and joins the fleet with no human enrolling it on
  site. Today the voucher register and the rendezvous seam exist
  and every lookup is refused; the manual P9.33 enrollment path
  is the only way a device joins.
- **Owning phase:** Phase 18 (P18.09).
- **Acceptance criteria:** (1) a device with a recorded
  ownership voucher claims through the rendezvous service and
  joins the fleet with no human enrollment step; (2) a device
  without a voucher entry is refused; (3) the manual P9.33 path
  still works and remains the fallback; (4) every claim attempt
  is recorded in the audit trail.
- **Status:** PROPOSED. Depends on a rendezvous service
  existing (external infrastructure) and a voucher-issuance
  process at provisioning.

### 5.6 Considered and not selected — with reasons

- **RLS enforcement verification as the `flexcore_query` role —
  not a feature: it already exists.** The test is written in
  `crates/adapters/postgres/tests/telemetry_reader_rls.rs`. It is
  gated and unrun — the pre-publication item is V-1 (run it), not
  building it.
- **Fleet health scoring — already exists.**
  `crates/flexcore-application/src/fleet_health.rs` implements
  P15.12 (deviation-from-normal scores, availability KPI). Not
  new; its operator views ride on F-09.
- **Interconnection records / IEEE 1547 settings — already
  exist.** `crates/flexcore-domain/src/interconnection_records.rs`
  carries UL 1741 SB COC, 1547.1 test reports, and settings
  profiles (P14.13/P15.15). Not new.
- **DNP3 master role — not selected: needs a counterparty.**
  The adapter implements the OUTSTATION role; the master role is
  a record-level DEFER (`crates/adapters/protocol-dnp3/src/
  lib.rs`). A master with no utility SCADA to talk to is code
  with nothing to talk to. Revisit on a named SCADA integration.
- **IEC 60870-5-104 client and IEC 61850-7-420 MMS client — not
  selected: record-only designs, not ready to build on.** Both
  are design records only (records 41/42); the standard texts
  were NOT consulted, every protocol detail is an UNVERIFIED
  secondary-source reading, no crate exists, and both need a DSO
  telecontrol counterparty. The live client would first need its
  standard text verified. Revisit on a named DSO integration.
- **OCPI/OICP roaming — not selected: explicitly out of scope.**
  P17.17 record and gap analysis §8: a depot fleet behind a site
  meter is not roaming. Revisit only on the record's three
  revisit conditions.
- **Market participation (programs, bids, settlement, revenue)
  — deferred: needs D1.** Phase 16 exists as design and
  record-only depth; live bidding needs a named first market
  (D1 OPEN) and a market counterparty, and ADR-011 forbids a
  generic tariff engine. Gap analysis §8 keeps wholesale live
  bidding out of v1.x. Stays post-v1.
- **V2G bidirectional dispatch — not selected: needs F-13
  first.** P17.13 is design records only; discharge dispatch
  needs PnC, OCPP 2.1 DER hooks, and a utility path. After F-13,
  not before publication.
- **ADMM / DMPC / GNN warm-start / MARL — not publication
  features.** P8 research capabilities; the gap analysis places
  them post-v1 (phases 11–20 §9) and none is publication-blocking.
  They stay research-gated, never dispatch inputs.
- **Kubernetes migration — deliberately deferred.** Gap analysis
  §8: Compose is correct until a multi-host / DR / canary trigger
  fires (Phase 20). Not a pre-publication feature.
- **Kafka migration — rejected.** Gap analysis §8: JetStream fits
  the lifecycle shape. Not a feature.
- **Operator-approved control qualification — NOT a feature.** It
  is a P10 human gate (§6 item 8), not something to implement in
  this round. No code in §5 enables it.
- **Remaining phases 11–20 items stay post-v1.** The designs
  exist; only the §5 features above are proposed for pulling
  forward. Everything else in the gap analysis (certification
  readiness, Postgres HA, TimescaleDB, SPIFFE, chaos/DR drills as
  scheduled game days) stays on its phase-11–20 track.

---

## 6. Publication gate — the ordered checklist

All items must be true before v1.0.0 is published. Order matters:
verification first, then decisions, then the release mechanics.

1. **Gated postgres re-run green** (V-1) on a fresh database,
   including the E_STORE_UNAVAILABLE trio (V-6) and fix M (V-7).
   Unblocker: Postgres 18 re-provision approval.
2. **Worker gated tests green** (V-2, 11 tests).
3. **Finish step green** (V-4): `fmt` decision executed (§4.3),
   `cargo clippy --workspace -- -D warnings` green,
   `check_dependency_rule.py` and `verify_docs.py` pass, warning
   triage closed (V-5).
4. **Cursor decision recorded and implemented if Option A**
   (§4.2), or the Option B exclusion recorded in the
   contract.
5. **Proposed features dispositioned** (§5): each F-01–F-15
   approved (then implemented + tested) or rejected (then
   recorded with the reason). No PROPOSED item may be silently
   left open at publication.
6. **D1–D6 recorded** (§4.1). Minimum for publication: D1–D3
   decided; D4 license decided; D5/D6 decided or explicitly
   deferred with a recorded gate date.
7. **Release-notes draft revised** — the stale statements
   ("no Cargo.lock", "no test suite executed") corrected to the
   §2.3 evidence, and every NOT-QUALIFIED verdict either closed
   with evidence or re-confirmed as still open.
8. **P10 human gates dispositioned** — security review convened
   and signed, or explicitly deferred with a record (P10.25);
   operator-mode qualification verdict recorded (P10.15 — today
   NOT QUALIFIED); benchmark baseline run exists, or its absence
   is explicitly deferred (P10.09/P10.10). **A deferred gate is
   a recorded deferral, not a silent pass.**
9. **Release artifact set exists**: tag `v1.0.0` only after §6
   items 1–6; changelog assembled from this document and the
   revised release-notes draft; D4 license decision recorded
   (§4.1); SBOM generated from the committed `Cargo.lock` (+
   console lockfile if the console ships); reproducible-build
   note — exact toolchain (Rust 1.99.1, `rust-toolchain.toml`),
   lockfiles, and build command recorded, and a second
   clean-environment build reproduces the artifact checksums;
   signing key material exists under the recorded procedure —
   or an explicit deferral record for any leg.
10. **Simulation-only rule restated at publication.** v1.0.0
    publication does not authorize operator-approved control at
    any site. Operator-approved control begins only after the
    §22.3 gate 11 qualification passes for that site
    (release-notes draft §2). Bounded automation remains
    post-1.0, per deployment, site, and asset class. Nothing in
    this document is a safety case, a certification, or
    permission for live grid control.

---

*Final implementation before publication — DRAFT-PROPOSED,
2026-11-19, HEAD `b8012e7`. Written from the ledger
(`ledger/MASTER_LEDGER.md`, v1.0 FINALIZED 2026-11-07), the Patch
A/B handovers, and the test-run log
(`~/workspace/flexcore-test-debug/BATCH_LOG.md`, batches 0–66).
The test run is STOPPED by Krishna's order; the gated suite and
finish step are unfinished (§3). Nothing in §5 is decided. The
foundations spec wins on any conflict.*
