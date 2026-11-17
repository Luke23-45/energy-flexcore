# Phase 13 — Enterprise Identity, Administration and Integration Platform: Exit-Gate Checklist

**Exit gate (plan §8, the phase's own acceptance criteria): all 22
subtasks have complete written artifacts ticked only as "implemented,
awaiting human build/test"; no IdP is deployed, no OIDC token validated,
no SCIM object provisioned, no notification sent, no webhook delivered,
no MFA enrollment exists, no login performed — every evidence cell
EMPTY/UNRUN; P13-E-1..E-3 remain OPEN with DEFAULTs; the 14-state/27-edge
transition set and the never-granted permission set are byte-identical;
the three permitted checks stay green.**

Each item states the evidence and its status. A checked item without
evidence is not checked. **Nothing in this phase has been built, run,
deployed, or connected** — the first human build and the P13.20 lab
runbook are the program gates; every "prepared" status below awaits its
named human gate, and none of these items claims runtime behavior.

## Plan §8 acceptance criteria (one-for-one)

- [x] **Criterion 1 — all 22 subtasks have complete written artifacts;
  ledger ticks read "implemented, awaiting human build/test" with a
  decision-record citation, never "tested"/"verified"/"passed"/"works".**
  Evidence: records 01–08 + 19 (wave 1 Agent A), 21–26 + 27 (wave 1 Agent
  B), 41–47 + 48 (wave 2 Agent A), 61 (wave 2 Agent B, the close) exist
  under `records/`. The ledger's P13.01–P13.14 ticks carry citations that
  all resolve to files on disk (close record §3.1); the P13.15–P13.22
  ticks are the ORCHESTRATOR's post-close action (conventions §2.2),
  expected to cite records 41, 42, 43, 44, 45, 46, 47, and 61. Status:
  **satisfied by written artifacts** for every record that exists at
  close.
- [x] **Criterion 2 — no IdP deployed, no OIDC token validated, no SCIM
  object provisioned, no notification sent, no webhook delivered, no MFA
  enrollment exists; the lab environment (P13.20) is a written
  specification, not a running stack.** Evidence: the honesty sweep
  (close record §3.4) over every phase artifact including the compose/
  realm drafts (`deploy/compose/idp-lab/` — no image pulled, no container
  started, no realm imported) and every code module's test surfaces;
  every evidence cell in every record is EMPTY/UNRUN. Status:
  **satisfied by the audit** as of the close record's date (an
  inspection, not a tool run).
- [x] **Criterion 3 — P13-E-1..E-3 remain OPEN with their DEFAULTs
  recorded, never closed by an agent; LR-047/048 close as designed
  (implemented, awaiting human build/test) and LR-062 closes as data
  capture with legal sign-off still a human gate.** Evidence: P13-E-1
  registered OPEN (record 01 §1; the DEFAULT executed by records 02 and
  46 with the dependence recorded), P13-E-2 OPEN (record 23 §4), P13-E-3
  OPEN (record 04 §5); the ledger's decision table reads OPEN for all
  three; record 26 models the sign-off as a human-only `UNSIGNED`→`SIGNED`
  transition and every sign-off cell is EMPTY. Status: **satisfied by the
  register entries**; no agent closed any decision.
- [x] **Criterion 4 — the 14-state/27-edge transition set and the
  never-granted permission set are byte-identical; `python -I
  tools/check_dependency_rule.py` stays green after every implementation
  batch.** Evidence: `git diff --numstat HEAD` on
  `crates/flexcore-domain/src/command_state.rs`,
  `crates/flexcore-domain/src/tenant_context.rs`, and
  `crates/flexcore-events/src/events.rs` is empty (0 added / 0 removed —
  close record §3.5); the four never-granted names still parse to `None`
  (the `action.rs` and `phase13_policies.rs` negative tests, quoted in
  close record §3.5); the tool outputs are quoted verbatim in close
  record §2 (20 workspace members — no crate was added). Status:
  **satisfied by the quoted evidence** (byte-identity by inspection and
  git; the tool checks the dependency rule, not runtime behavior).
- [x] **Criterion 5 — every capability claim traces to written code or
  design records; test files are written complete and left unrun.**
  Evidence: the per-record ASSUMPTION/UNRUN/EMPTY registers (every record
  §"ASSUMPTION"/§"honest limits"); the P13.19 suite's five Rust test
  files plus the six console test files are written complete and UNRUN
  (record 45; records 43/44); no result cell exists anywhere in the
  phase. Status: **satisfied by written artifacts (UNRUN)**.

## Human-gate inputs still pending (none of them closable by an agent)

| # | Gate input | Unblocks |
|---|---|---|
| 1 | First human `cargo build --workspace` / clippy / `cargo test --workspace` (compiles the pin, the new modules, the authz additions, the migration, and every written test file — the RFC 4231/FIPS vector tests are build-blocking for the hand-written crypto) | every WRITTEN-UNRUN claim in the phase; the recorded compile ASSUMPTIONS (records 02 §5, 05 §7, 21 §4, 45 §5, 47 §5) |
| 2 | First console install (`npm test` over the six new console test files) | the records 43/44 console artifacts |
| 3 | The P13.20 lab runbook R-1..R-8 (secret file, client secret, first `docker compose up`, realm-import verification, resolver composition, SC-1..SC-8, first login, amendment record) | the first real token, the first JWKS rotation, the record 45 §3 scenario cells, the record 02 §5 network half |
| 4 | Operator tuning of the registered RECOMMENDATION thresholds (records 02, 03, 04, 08, 21 §3, 22 §3, 42 §3) | every registered constant in the phase |
| 5 | Human closers for P13-E-1/E-2/E-3 (records 01 §1, 23 §4, 04 §5) and the D1/LR-062 jurisdiction decision + legal sign-offs (record 26 §4) | the three OPEN decision points; the retention matrix's value cells |
| 6 | The orchestrator's ledger ticks for P13.15–P13.22, citing records 41–47 and 61 | the expected 22/22 end state (close record §1) |
| 7 | Post-implementation composition follow-ups named in the records: the Postgres webhook/session stores (records 21 §5, 42 §5), the export route + chain-column read extension (record 24 §5), the login/callback/session endpoints (record 43 §4), the admin endpoints (record 44 §3) | the specified-not-implemented surfaces, each with its owning record |

Full close position and the consistency-audit results:
`records/61_p13_22_phase_close.md`.
