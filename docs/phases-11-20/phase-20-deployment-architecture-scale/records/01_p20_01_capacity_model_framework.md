<!-- P20.01 — Capacity model framework: measurement -> published capacity
     number -> SLO derivation, IN THAT ORDER, never reversed [S-61]. The
     measurement and result cells are EMPTY until the human/lab run fills
     them (X-8; conventions §5). This record is the SINGLE DEFINITION of the
     metric-cardinality budget — Agent B's P20.15 consumes it BY CITATION
     and never restates or re-derives it. -->
# P20.01 — Capacity Model Framework (measurement-first, cells EMPTY — no capacity number exists)

| Field | Value |
|---|---|
| Subtask | P20.01 — Capacity model framework (`ledger_phase_20.md` §2, row P20.01) |
| Wave / agent | Phase 20 wave 1, Agent A |
| Status | **FRAMEWORK RECORD — WRITTEN-UNRUN.** No measurement has run, no capacity figure exists anywhere in this record, every measurement and result cell is EMPTY, and every SLO row is DERIVATION-PENDING. The framework is the discipline that turns a human-run measurement into a published capacity statement; it is not, and does not contain, a capacity claim. |
| Sources | [S-61] (Google SRE Workbook — SLOs/alerting derive from measured service behavior, never precede it); gap analysis §5 (Tier 0/1 load-ladder rows); `apps/sim-runner/src/load_run.rs` §1 scope notes (the single-client honesty boundary this framework extends); `apps/sim-runner/src/benchmark.rs` (`MeasuredField` — measured with unit+method, or `not_measured` with a mandatory reason; fail-loud, never a default zero); P11.01 metric taxonomy (`docs/phases-11-20/phase-11-operational-robustness/records/01_p11_01_metric_taxonomy.md`, consumed); P11.02 SLO register (`docs/phases-11-20/phase-11-operational-robustness/records/02_p11_02_slo_register.md`, consumed — not redefined); P11.20 stuck-state threshold discipline (X-8 pre-registration pattern) |
| Files touched | this record only (the framework record; the runs that fill it live in P20.02's scripts and the human-run record) |

## 1. Honest status — the plain statement

**No capacity number exists.** The repository has never deployed a stack
(`deploy/compose/` has never been brought up by this program; the CI record's
24 jobs are UNRUN; every benchmark cell in `benchmarks/research-set-v1.json`
is `NOT_RUN`), so there is nothing real to measure and nothing measured to
publish. This record therefore publishes a FRAMEWORK: the ordered pipeline
(§2), the record shape every measurement must use (§3), the cardinality
budget for metrics (§4 — the single definition, consumed by citation), the
published-capacity-statement template with EMPTY cells (§5), and the
SLO-derivation procedure that runs only AFTER a capacity statement exists
(§6). Any text anywhere in phase 20 that reads as "N commands/s at p99 < X
ms" without a run behind it is a compliance failure (plan §7 R-P20-6).

## 2. The pipeline — measurement first, number second, SLO last

The order is FIXED and never reversed [S-61]:

| Step | Artifact | Producer | Status |
|---|---|---|---|
| 1. MEASURE | Run a P20.02 scenario (or the P5.24 `load-run` harness) on a REAL deployed stack; record figures as `MeasuredField` values with unit, method, and hardware/container capture | Human/lab operator executing P20.02's written-unrun scripts | **EMPTY — no run has happened** |
| 2. PUBLISH the capacity number | Fill the §5 capacity-statement template FROM THE RUN ONLY: "N commands/s at p99 < X ms" plus the conditions block (hardware, data shape, scenario id, date) | Human operator, from step-1 records | **EMPTY — no figure exists** |
| 3. DERIVE the SLO | The P11.02 SLO register's open rows are completed by DERIVING objectives from the measured envelope (e.g. an accept-latency objective set at or inside the measured p99 at the operating point the operator chooses) | Human owner; agents never assert an SLO that no measurement supports | **DERIVATION-PENDING** |

Anti-rule (the reversal this framework forbids): writing an SLO or a
capacity target first and then engineering the measurement to confirm it.
The P11.20 pattern is the model — a threshold may be REGISTERED before a
run (X-8), but it is never presented as a measured or achieved figure.

## 3. The record shape every measurement must use

Every figure produced under this framework is one of exactly two shapes,
carried over from the P3.27 `MeasuredField` type — never a bare number,
never a blank that reads as zero:

| Shape | Meaning | Mandatory content |
|---|---|---|
| `Measured { value, unit, method }` | Produced by the recorded method on this run | The unit (per the canonical-units discipline; latency in ms matching the P11.01 fixed bucket set), the method verbatim enough to reproduce, and the run's hardware/container capture (`capture_hardware`, P3.27) |
| `NotMeasured { reason }` | The run could not produce it | A non-empty reason. No default zero, no extrapolation, no fitted value |

Additional hard rules:

1. Percentiles use the documented nearest-rank method on the MEASURED
   samples only — never fitted, never extrapolated (the `load_run.rs`
   discipline, consumed verbatim).
2. The conditions travel with every figure: the scenario id, the synthetic
   data set version (`deploy/load-test/data/synthetic-data-policy.md`), the
   stack topology measured, and the date. A figure without conditions is
   not a measurement.
3. The anti-silent-growth rule carries over from `research-set-v1.json`:
   changing a scenario, its data set, or a registered load level is a NEW
   version of that artifact, never an in-place edit of a filled result.

## 4. Metric cardinality budget — the SINGLE definition

This section is the ONE definition of the cardinality budget. Consumers
(P20.15 cost-control notes; any dashboard or alert file) cite this section;
they never restate, re-derive, or silently change a ceiling. The ceilings
below are REGISTERED DESIGN CONSTRAINTS (X-8: written down before any run
that could measure against them), not measured cardinality counts — the
measured-count column stays EMPTY until a real deployment exposes the
registry and a human scrapes it.

| Metric class (existing registry families) | Labels in play | Ceiling (label combinations per class) | Measured count |
|---|---|---|---|
| API latency histogram `flexcore_api_request_duration_milliseconds` | route (templated path, never a raw path with embedded ids), method | ≤ 64 routes × 4 methods = **256 series** (× the fixed 12-bucket set is the exposition cost, not extra series) | `<EMPTY — UNMEASURED>` |
| `flexcore_api_in_flight_requests` | none | **1 series** | `<EMPTY — UNMEASURED>` |
| Command lifecycle `flexcore_command_state_transitions_total{from,to}` | from, to (the closed 14-state vocabulary; only legal transitions occur) | ≤ 13 × 13 bounded by the frozen edge table, ≤ **169 series** (practically far fewer; the ceiling is the vocabulary bound) | `<EMPTY — UNMEASURED>` |
| `flexcore_command_accept_duration_milliseconds` | none | **1 series** | `<EMPTY — UNMEASURED>` |
| `flexcore_commands_stuck` | state (non-terminal frozen states) | ≤ **9 series** (14 states minus the terminal set, bounded by the frozen vocabulary) | `<EMPTY — UNMEASURED>` |
| Worker consumer counters `flexcore_command_consumer_*` (5 named counters) | none | **5 series** | `<EMPTY — UNMEASURED>` |
| Telemetry counters `flexcore_telemetry_events_total{event}` | event (frozen 3-event set) | **3 series** | `<EMPTY — UNMEASURED>` |
| `flexcore_telemetry_quarantine_by_reason_total{reason_class}` | reason_class (closed class, free text never a label) | ≤ **16 series** | `<EMPTY — UNMEASURED>` |
| `flexcore_telemetry_quality_states_total` | quality (6 canonical states) | **6 series** | `<EMPTY — UNMEASURED>` |
| `flexcore_telemetry_quarantine_total`, `flexcore_telemetry_queue_depth`, `flexcore_http_requests_total{labels}`, `flexcore_http_errors_total`, `flexcore_webhook_dead_letters` | channel (closed Phase 13 channel set, ≤ 8) where labeled; otherwise none | ≤ **16 series combined** | `<EMPTY — UNMEASURED>` |
| OTel SDK self-metrics (P11.04/P11.05 runtime families) | fixed by the SDK | ≤ **64 series** (registered allowance; actual set is SDK-determined) | `<EMPTY — UNMEASURED>` |
| **TOTAL BUDGET** | — | **≤ 640 active series** across the control plane and worker combined; per-tenant or per-asset breakdowns are FORBIDDEN as metric labels (they go to logs per the P11.01 taxonomy) | `<EMPTY — UNMEASURED>` |

Budget-amendment rule: raising any ceiling is a recorded amendment to THIS
section (new version note in the amendment row), consumed downstream by
citation — never a silent edit, and never a per-consumer local ceiling.

## 5. The published-capacity-statement template (cells EMPTY)

The statement a human fills AFTER a run. Until then every dynamic cell is
EMPTY and the statement does not exist as a claim — only as this template.

| Field | Value |
|---|---|
| Capacity statement | `<EMPTY — NO RUN HAS HAPPENED; this cell is filled only from a P20.02 run's records>` |
| Scenario id / script version | `<EMPTY>` |
| Synthetic data set version | `<EMPTY>` |
| Stack topology measured | `<EMPTY>` |
| Hardware/container capture | `<EMPTY>` (P3.27 `capture_hardware` shape) |
| Accept-path p50/p95/p99 | `<EMPTY>` (nearest-rank on measured samples) |
| Accept throughput at the operating point | `<EMPTY>` |
| Worker fan-out completion rate | `<EMPTY>` |
| SSE stream concurrent-connection ceiling observed | `<EMPTY>` |
| Validity statement | `<EMPTY — the conditions under which the number holds>` |
| Explicit honesty line | A figure from the P5.24 harness (ONE sequential client) is a single-client figure and is NEVER published as a deployment capacity claim (`load_run.rs` §1 scope notes, consumed verbatim-in-substance) |

## 6. SLO derivation procedure (runs only after §5 is filled)

1. Take the §5 statement and its conditions as the ONLY evidence input.
2. For each P11.02 SLO-register row still open on latency/throughput,
   propose the derived objective AT or INSIDE the measured envelope at the
   operator-chosen operating point — never outside it, never from a
   first-principles guess [S-61].
3. Record the derivation in the P11.02 register (the owner closes SLO
   decisions; agents never assert an SLO).
4. Multi-window multi-burn-rate alerting on the derived SLOs is the P11
   alerting shape's consumption of step 3 — the framework does not create
   a second alerting regime.

## 7. What this framework does NOT establish

1. No capacity, throughput, latency, or cardinality figure exists anywhere
   in this record — every dynamic cell is EMPTY (X-8).
2. No SLO is derived, asserted, or implied; §6 has not run.
3. No measurement harness is extended or written by this record: the
   concurrent-client measurement layer is P20.02's written-unrun scripts,
   and the P5.24 single-client harness stays exactly as scoped.
4. No deployment exists to measure; the framework's first execution is a
   human gate on a stack the operator has brought up deliberately.
5. No dispatch authority, no release-ladder movement: everything measured
   is synthetic, simulation-only.

## 8. Reviewer-verification procedure (human)

1. Confirm §1 and §5 carry no number that a run did not produce.
2. Confirm §2's order is measurement → number → SLO, and §2's anti-rule
   is present.
3. Confirm §4 is the only cardinality-budget definition in the repository
   (Agent B's P20.15 cites this section) and that every Measured-count
   cell is EMPTY.
4. Confirm the §4 ceilings reference only metric families that actually
   exist in the registries (`apps/control-plane/src/metrics.rs`,
   `apps/worker/src/command_consumer.rs`).
5. Confirm the P11.01/P11.02 records are consumed by citation, not
   redefined.

## 9. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Bring up a stack deliberately, run the P20.02 ladder per its README,
#     fill §5 FROM THE RUN ONLY (nearest-rank percentiles, conditions
#     block, hardware capture). Pre-register any judged threshold (X-8)
#     BEFORE the run. Then execute §6 and record the derivation in the
#     P11.02 register.
```

## 10. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.01-1 | The §4 label enumerations match the current registry code; any new metric family must enter the budget by amendment | Static inspection 2026-11-17; re-verified at first build |
| A-P20.01-2 | The §5 template's shapes assume a P20.02 run as the producer | A different producer (e.g. an operator's own run) is acceptable if it satisfies §3's record shape |
| F-P20.01-1 | No measured figure of any kind exists | Carried by design — the human run is the generator |

Status: **WRITTEN-UNRUN** — a framework cannot publish a capacity number
by being written; the measurement run and the SLO derivation are the human
gates and have not happened and are not scheduled.
