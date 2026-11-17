# Phase 11 — Operational Robustness and Observability: Exit-Gate Checklist

**Exit gate (plan §6, the phase's own acceptance criteria): every subtask
carries a decision record; the ops-tree configs are checked in and never
executed; the called-for test files are complete and UNRUN; the three
permitted checks stay green; and no run/build/scan/conformance claim
exists anywhere, with all targets RECOMMENDATION and all three
E-decisions OPEN with DEFAULTs.**

Each item states the evidence and its status. A checked item without
evidence is not checked. **Nothing in this phase has been built, run,
scraped, or scanned** — the first human build is the program gate; every
"prepared" status below awaits that gate, and none of these items claims
runtime behavior.

## Plan §6 acceptance criteria (one-for-one)

- [x] **Criterion 1 — every subtask has a decision record under `records/`
  citing the files touched and the [S-n] sources.** Evidence: records
  01–09 and 21–27 (wave 1) and 51, 52, 54 (wave 2 Agent B) exist under
  `records/`; the wave-2 Agent A records (31–45 range) were IN FLIGHT at
  close time and their audit status is carried in
  `records/53_p11_22_phase_close.md` §3. The ledger's per-row citations
  are the ORCHESTRATOR's ticks (program conventions §2.2). Status:
  **satisfied by written artifacts** for every record that exists at
  close; the P11.15–P11.19 citation check is the orchestrator's
  post-landing step.
- [x] **Criterion 2 — alert rule files, dashboard JSONs, recording rules,
  gitleaks-class config, and Collector config are checked in, never
  executed.** Evidence: `deploy/compose/observability/` (records 21, 25),
  the Collector config + Compose service (record 06), the secret-scanning
  / pre-commit / CI config files (record of P11.17, wave 2 Agent A). No
  Prometheus/Grafana/Collector/scan execution exists — a live
  integration check is a human gate input, UNRUN. Status: **satisfied by
  written artifacts (UNRUN)**.
- [x] **Criterion 3 — the P11.08 redaction and P11.12 duplicate-delivery
  test files are complete and UNRUN.** Evidence:
  `python/workers/tests/test_redaction.py`,
  `python/workers/tests/test_log_schema_and_trace_propagation.py`
  (record 08), `apps/worker/tests/command_duplicate_delivery.rs` (record
  24), plus the in-module Rust unit tests (records 51, 52 add more).
  Their execution is the human gate. Status: **satisfied by written
  artifacts (UNRUN)**.
- [x] **Criterion 4 — the three permitted checks stay green after every
  implementation batch.** Evidence: the verbatim outputs quoted in the
  wave-1 close-outs (records 09 §3, 27 §3) and in the close record
  `records/53_p11_22_phase_close.md` §2 — `verify_docs.py` OK,
  `validate_agent.py` OK, `check_dependency_rule.py` OK (20 workspace
  members) at each close. Status: **satisfied by the quoted tool
  outputs** (the tools check structure and the dependency rule, not
  runtime behavior).
- [x] **Criterion 5 — no run/build/scan/conformance claim; SLO targets
  RECOMMENDATION; P11-E-1/E-2/E-3 OPEN with DEFAULTs.** Evidence: the
  close record's honesty sweep (record 53 §3.4) across every phase-11
  artifact; the SLO register (record 02) labels every target
  RECOMMENDATION; record 54 registers P11-E-1 OPEN, record 22 registers
  P11-E-2/E-3 OPEN — no agent closed any of them. Status: **satisfied by
  the audit** as of the close record's date; the sweep is an inspection,
  not a tool run.

## Human-gate inputs still pending (none of them closable by an agent)

| # | Gate input | Unblocks |
|---|---|---|
| 1 | First human `cargo build --workspace` / clippy / `cargo test --workspace` | every WRITTEN-UNRUN claim in the phase; expected to surface recorded integration defects |
| 2 | Operator tuning of the registered thresholds (SLO targets, stuck threshold + scan interval, retention bounds) | the RECOMMENDATION labels across records 02, 25, 51 |
| 3 | A live Prometheus / Grafana / Collector integration check | the rule files, dashboards, and gateway shape (plan risk 5) |
| 4 | The gitleaks-class scan run + findings review | P11.17's checked-in configs (findings are a human gate) |
| 5 | Human closers for P11-E-1/E-2/E-3 (record 54 §3) | the three OPEN decision points |
| 6 | The orchestrator's ledger ticks for P11.15–P11.22, citing the wave-2 records | the expected 22/22 end state (record 53 §3.1) |

Full close position and the consistency-audit results:
`records/53_p11_22_phase_close.md`.
