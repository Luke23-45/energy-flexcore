# P6.26 + P6.27 + P6.28 — Documentation, CI gates, and the failure-mode/recovery drill: IMPLEMENTED (awaiting human build/test/execution)

Date: 2026-11-13. Subtasks: P6.26, P6.27, P6.28. Owner roles: technical
writer + integration engineer (P6.26), DevOps (P6.27), integration lead
(P6.28, per the ledger).

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`):** every deliverable in this
wave is WRITTEN, NOT BUILT, NOT RUN, and NOT EXECUTED. No `cargo` command of
any kind, no CI run, no drill, no scan, no install. Every check, gate, drill
fault, and measurement below is **UNRUN** until the human executes it. No
pass, green run, finding, or measured value is claimed anywhere in this
document — the result cells are deliberately EMPTY.

Files (complete list; nothing outside it was touched):

| File | Status |
|---|---|
| `docs/operations/edge-agent-operations.md` | NEW (P6.26) |
| `docs/protocols/openems-adapter.md` | NEW (P6.26) |
| `docs/security/edge-ot-boundary.md` | NEW (P6.26) |
| `.github/workflows/ci.yml` | EDITED (additive: one header comment block, one added log step in the existing `edge-agent-suite` job, two new jobs `edge-license-gate` and `edge-supply-chain`; NO existing job or step rewritten) |
| `deny.toml` | NEW (P6.27; did not exist — checked before writing) |
| `ledger/phase-06-edge-protocol/ledger_phase_6.md` | EDITED (P6.26/P6.27/P6.28 ticks only) |
| this file | NEW (decision record) |

No Rust/TS source file was touched (the concurrency constraint holds; the
P6.25 console work was in flight in parallel and is not owned by this wave).

---

## 1. P6.26 — documentation (per-subtask coverage)

### 1.1 Doc inventory and the honesty carried into each

| Doc | Covers | Honesty status carried verbatim into the doc |
|---|---|---|
| `docs/operations/edge-agent-operations.md` | Install (P6.20 profile — the systemd unit is written NOT installed; build procedure UNRUN), enroll (P6.06 flow incl. the fail-closed matrix; CA/token service = deployment requirements; `DenyAllCredentialIssuer` shipped default), monitor (P6.17 health vocabulary VERBATIM — `HEALTHY`, `DEGRADED_<reason>`, `FAILING_<reason>` with the closed reason set — plus the P6.25 console surfaces, labelled "in progress, UNRUN"), update (P6.20 signed-update DESIGN ONLY — no PKI, no signing keys, no verification code), recover (P6.09 journal mechanics: audited drop-of-oldest, at-least-once, corrupt-tail truncate-and-audit; P6.18 audit buffer + the four-point offline-behaviour statement), decommission (P6.32 ledger procedure: revocation + wipe + the upload-then-wipe-or-gap-event rule — labelled "P6.32 open, implementation NOT built") | A status header states the whole guide describes WRITTEN-UNRUN code; §9 states the limitations plainly: encryption at rest = DEPLOYMENT REQUIREMENT (journals in the clear until closed), TLS-on-OpenEMS-hop = open item (`http://` only composes), uplink wire-format seam = `BatchPayloadEncoder` refuses, nothing forwarded until the owner implements it |
| `docs/protocols/openems-adapter.md` | OpenEMS pin **2026.11.0** per `RESEARCH.md` §1 (checked 2026-11-07; the lab must pin the version it ACTUALLY runs — the doc says exactly that), the REST API usage with the full A1–A8 UNVERIFIED assumption register carried over verbatim from `10_11_openems.md` §2, the versioned mapping-profile design (`openems-mapping-profile.v1.json`), the read-only/GET-only structural guarantee (five layers), the execution-model honesty (FlexCore reads channels; never injects into the IPO cycle; no Controller/Scheduler touched), the conformance suite + claim boundary VERBATIM from `19_23_22_conformance_security_performance.md` §3, known limitations | The A-register table states "UNVERIFIED — carried over verbatim"; the claim boundary is the ONLY permitted certification-claim text, block-quoted, and the doc says so; the OpenEMS license is recorded as UNVERIFIED/FLAG (D4), never assumed Apache-2.0 |
| `docs/security/edge-ot-boundary.md` | Marked SITE-SPECIFIC with `pilot-site-A` = PLACEHOLDER (per `01_d5_disposition.md` §4); outbound-only connectivity (structural no-inbound: lexical-scan-enforced, only socket op is an outbound connect; systemd unit ships no `Listen*`), the mTLS profile from `06_07_identity_channel.md` §2 (TLS 1.3 DEFAULT floor, client-only, root CA pinned from the credential record, deterministic backoff), the D6 review status (OPEN — review PREPARED, sign-off NOT SIGNED, reviewer NOT ASSIGNED, per `03_d6_boundary_review.md`), the disabled-by-default command path + 4-condition gate summary, the clock/hold-on-drift rule, tenant isolation + blast radius (from `18_21_audit_offline_isolation.md` §6), secrets handling (no secrets in config/source/logs; redaction `Debug` impls; OpenEMS Getting Started defaults never a deployment model), the open-items list handed to the D6 reviewer | Header states the note "describes the recommended boundary positions and the BUILT code's structural properties, and nothing here constitutes the signed review"; governing reference NIST SP 800-82 Rev. 3 (Rev. 4 is a draft, not cited as a requirement) |

### 1.2 Conventions compliance

- US "license" spelling throughout (WRITING rules; also used in `deny.toml`).
- Tables over prose for all enumerations; pipe counts consistent per table.
- Every protocol claim cites its specification version: OpenEMS 2026.11.0
  with its source (`RESEARCH.md` §1, Zenodo record, checked 2026-11-07);
  no other protocol claim exists in these docs.
- Every `§N` reference resolves to a real report heading (checked against
  the report's heading list: §2.1, §2.3, §3.3, §5.7, §9.3, §10.2–§10.4,
  §14.1, §14.3, §14.4, §14.5, §16.2, §17.2–§17.5, §18.1–§18.3, §19.2,
  §20.1–§20.2, §21.2–§21.4, §22.2, §23, §25.2, §27).
- No safety, protection-grade, or certification claims anywhere outside the
  single block-quoted boundary in `docs/protocols/openems-adapter.md` §6.
- `docs/foundations/*`, `docs/plan/*`, `.agent/*` untouched;
  `python tools/verify_docs.py` (which checks the report in `docs/plan/`)
  ran green after the doc additions — output quoted in §7.

### 1.3 Acceptance criteria status (P6.26)

| Criterion | Status |
|---|---|
| Docs exist for install/enroll/monitor/update/recover/decommission | MET (written) |
| Every protocol claim cites its specification version | MET |
| `verify_docs.py` passes | RUN — output in §7 |
| "A new engineer follows the install guide on a clean lab host and succeeds without asking the author" | **UNRUN — human gate input** (the guide's own build/install steps are UNRUN) |

---

## 2. P6.27 — CI gates (per-subtask coverage)

### 2.1 The §22.2 gate matrix (gate → job → status)

| §22.2 gate | CI wiring | Covers the Phase 6 crates? | Status |
|---|---|---|---|
| `cargo fmt --check` | existing `static-checks` job (unchanged) | Yes — workspace-wide, edge crates are members | **UNRUN** |
| `cargo clippy --workspace --all-targets -- -D warnings` | existing `static-checks` job (unchanged) | Yes — workspace-wide | **UNRUN** |
| `cargo test --workspace` | existing `unit-tests` + `integration-tests` jobs (unchanged) | Yes — workspace-wide | **UNRUN** |
| Scenario regression suite | existing `scenario-regression` job (sim-runner replay verdict + P4.31 fault-injection + shadow suites) — pattern already exists, nothing new invented | Platform layer; the edge fault coverage is in the `edge-agent-suite` job | **UNRUN** |
| Container/dependency scanning | NEW `edge-supply-chain` job: cargo-audit (RustSec advisories), Syft SBOM/container scan (`anchore/sbom-action`) — tools referenced by name in config only | Yes | **UNRUN — config only** |
| SBOM generation | existing CycloneDX step in `security-and-supply-chain` (unchanged) + the NEW Syft artifact | Yes | **UNRUN** |
| License/security review | NEW `edge-license-gate` job: `cargo deny check licenses` (FAILING gate, backed by the new root `deny.toml`) + the D4 re-verify marker check; the pre-existing `continue-on-error` cargo-deny step in `security-and-supply-chain` is UNCHANGED | Yes | **UNRUN** |
| Adapter conformance suite as its own CI job | existing `edge-agent-suite` job (P6.19) — this wave ADDS one step that logs `suite-id=flexcore-edge-adapter-conformance-suite-v1` and `openems-pin=2026.11.0 (RESEARCH.md §1, checked 2026-11-07; re-pin at the lab run)` into the job log lines, per ledger P6.27 step 2 | Yes | **UNRUN — CI execution UNRUN (human gate input); never claimed green** |

Path-filter decision (recorded, DEFAULT): the sim-runner regression job
exists and has NO path filters; introducing filters would RESTRICT existing
coverage (a non-additive change to an existing job), so none were added —
every job keeps its existing trigger. The header comment block added to
`ci.yml` records this.

### 2.2 deny.toml design and the D4 tie-in

`deny.toml` (repo root, NEW — checked absent first) carries:

- `[licenses]` with `unlicensed = "deny"` — the P6.27 acceptance item "an
  unlicensed dependency fails CI by construction", as a FAILING gate job
  (`edge-license-gate`), distinct from the older `continue-on-error` step.
- The allow list is the config form of the recorded P6.02 audit (permissive
  families only: MIT, Apache-2.0 (+LLVM-exception), ISC, BSD-2/3-Clause,
  Unicode-3.0/DFS-2016, Zlib, BSL-1.0, CC0-1.0, CDLA-Permissive-2.0,
  MPL-2.0, OpenSSL for ring's composite license). **Copyleft is
  intentionally not allowed.** The list is a transcription of records, NOT
  a scan result; the transitive set is UNVERIFIED until the first
  lockfile-backed run.
- D4 tie-in, verbatim in comments: (1) **rustls / rustls-pemfile marked
  RE-VERIFY** — licenses UNVERIFIED in-repo (crates.io metadata only),
  versions UNPINNED-REVERIFY, first scan must verify against primary
  sources and diff the transitive set (`02_license_audit.md` §8;
  `06_07_identity_channel.md` §5); (2) the **Apache-2.0/OpenEMS note** —
  OpenEMS is NOT a crates.io dependency and its license is UNVERIFIED/FLAG
  (`02_license_audit.md` §4): the Apache-2.0 attribution sometimes seen in
  the ecosystem must NOT be assumed; integration is the documented REST API
  over the network (external-system boundary) and no client library is
  added without the D4 audit note; (3) Ipopt (EPL) recorded only so D4
  expects it if Stage 8 ever introduces it.
- `[advisories]` with `yanked = "deny"`, empty ignore list (tokio stays
  pinned at 1.54.1, never 1.54.0 — the Phase 1 CVE note); `[bans]` /
  `[sources]` at template-default posture with tightening recorded as a
  reviewed change. No invented tool version pins anywhere.

### 2.3 Acceptance criteria status (P6.27)

| Criterion | Status |
|---|---|
| All §22.2 gates run on the new code and pass | Gates CONFIGURED (workspace-wide jobs already cover the crates); **passing is UNRUN — no green run is claimed** |
| Conformance suite runs in CI | Job exists (P6.19) + version-log step added; **execution UNRUN** |
| An unlicensed dependency fails CI by construction | Configured via `unlicensed = "deny"` + the failing gate job; the "demonstrated once on a scratch branch" item is a **HUMAN GATE INPUT — UNRUN** (introduce a test-unlicensed dependency, confirm CI fails, remove it) |
| Green CI run on the phase branch | **UNRUN — human gate input. Recorded, never claimed** |

---

## 3. P6.28 — failure-mode and recovery drill (edge + adapter)

**Status: PROCEDURE + REPORT TEMPLATE ONLY. No drill was executed.** The
drill runs at the EDGE/ADAPTER layer over lab hardware (the lab OpenEMS Edge
per the P6.24 run book — never the simulator alone; the ledger's recorded
risk). It does NOT re-run the platform suites: each fault is covered
somewhere by an existing suite, and this drill adds only the edge/adapter
layer measurements (no drift).

### 3.1 The fault set and REUSE-BY-CITATION (the drill invents no platform behaviour)

| # | Fault (edge/adapter layer) | Reused suite that already covers the behaviour (citation — NOT re-run here) | What THIS drill adds |
|---|---|---|---|
| 1 | Dropped messages | P6.13 conformance fault-injection set (`conformance_suite_p6_19` case 7/8; malformed/truncated refuse-or-flag); P6.09 buffer tests (no loss within capacity); P4.31 F1 (`31_fault_shadow.md` §2, platform outbox); P5.21 injection controls (`21_24_faults_visibility.md` §2) | Edge-layer detection time + reconciliation on lab hardware |
| 2 | Reordered messages | P6.13 `out_of_order` (flagged, never re-numbered); P6.15 sequence verifier (`sequence` refusal); P4.31 F2 (receive-point guard, platform) | Edge-layer detection time; order-preservation evidence through the buffer drain |
| 3 | Duplicated messages | P6.13 `duplicate_delivery` (idempotent skip); P6.08 `EdgeDedupWindow` + the at-least-once honesty record (`08_09_ingest_buffer.md` §4); P4.31 F1b; P2.09/P2.10 platform ingest dedup | Edge-layer duplicate counts before/after; zero duplicate stored values downstream |
| 4 | Clock skew | P6.13 `clock_skew_future`/`clock_skew_past`; P6.15 clock-quality fail-closed expiry (any quality ≠ `Synchronized`); P4.31 F4/F5; P5.21 `clock_skew` composition probe | Edge-layer: skew beyond tolerance → command rejected (never accepted), health state, audit event |
| 5 | Stale telemetry | P6.13 `stale_data` (never upgraded to GOOD); P6.10/P6.11 `stale_after_millis` STALE stamping; P4.31 F6/F7; P2.25 expected-quality plan | Edge-layer: time from feed stop to STALE at the uplink; downstream `MISSING` derivation (platform gap detector) |
| 6 | Unavailable assets | P6.17 health tests (`FAILING_UNRESPONSIVE` + blocked dependent actions); P6.13 forced-unreachable adapter (named violating report, no panic); P4.31 F8 (platform issue/fallback) | Edge-layer detection window (kill the lab adapter, time the transition) |
| 7 | Adapter crash | P6.13 no-panic property (all faulted readings through the total path); P6.17 health transition; P4.31 F10 is the PLATFORM analogue (modelled panic propagation) — cited, not re-run | Edge-layer: crash containment (no partial record), restart of the adapter, recovery time |
| 8 | Partial fleet response | P4.31 F11 (per-asset acknowledgement, never portfolio-inferred); P5.21 `partial_fleet_response` fleet probe | Edge-layer: per-command gate/policy outcomes; no orphaned command |
| 9 | Central outage | P6.18 audit-buffer outage tests incl. the bounded synthetic 30-minute window (`18_21_audit_offline_isolation.md` §3); P6.09 store-and-forward (outage, mid-drain outage); §21.2 platform invariant | Edge-layer: the REAL 30-minute lab outage — event-count reconciliation before/after reconnect (the recorded human gate input H1 of the P6.18 record) |
| 10 | Edge restart | P6.09 journal replay/restart tests; P6.18 restart-with-non-empty-buffer test; P6.15 sequence-checkpoint persistence (restart-replay denied) | Edge-layer: buffer survival, checkpoint integrity, time to first healthy observation |
| 11 | Solver timeout | **Cross-reference only** — P4.31 F9a/F9b (platform layer); no edge/adapter injection surface exists | Nothing (no edge-layer case; the journal carries the cross-reference) |
| 12 | Plugin crash | **N/A — there is no plugin host until Phase 7** (WASM host is v0.5.0 / Stage 7). Stated, not tested, not claimed in either direction | Nothing |

### 3.2 Drill procedure (how each cell is measured — the executable spec)

1. **Environment:** lab host + the lab OpenEMS Edge (version re-pinned at
   the run per `docs/protocols/openems-adapter.md`), edge agent built per
   `docs/operations/edge-agent-operations.md` §2, control plane reachable
   over the composed mTLS channel. Record versions (git SHA, edge-agent
   build, OpenEMS version, OS, hardware) BEFORE any fault. Simulator-only
   runs are NOT acceptable as the drill record (ledger risk row).
2. **Baseline:** start a clean run, record the event counts on both ends
   (telemetry records sent by the edge vs stored by the platform; audit
   events appended vs uploaded — `appended_total`, `pending_len`,
   `dropped_oldest`, committed counts).
3. **Per fault:** inject the fault once, with the injection instant recorded
   from the operator's log; let the system run a fixed observation window;
   remove the fault; allow recovery; then:
   - **Detection time** = first log/audit line naming the condition MINUS
     the injection instant. Timestamps come FROM THE LOGS with correlation
     IDs (§21.3) — never from memory. Every edge log/audit line carries the
     correlation id established at startup.
   - **Behaviour** = the named outcome observed (reject / flag / hold /
     fallback / redeliver), quoted from the log lines by correlation ID.
   - **Recovery time** = last log line of the recovery transition (e.g.
     drain completed, health returned, channel re-established) MINUS the
     fault-removal instant.
   - **Data loss (records)** = event-count reconciliation: baseline counts
     + injected records − dropped counters (`dropped_oldest`,
     `dropped_expired`, `corrupt_tail_truncated`) − pending = stored. The
     P6.09/P6.18 reconciliation identity
     `appended_total == committed + pending + dropped` is asserted per open.
   - **Command issued-or-duplicated** = grep the edge and platform audit
     trails for `EDGE_COMMAND_*` events in the fault window; a duplicate
     forward is the forwarder's per-command audit showing >1 forward for
     one command id; an orphaned command is any `EDGE_COMMAND_ACCEPTED`
     without its platform-side lifecycle counterpart.
4. **Independent witness (human step):** a second engineer re-runs TWO of
   the faults from the recorded procedure alone and reproduces the recorded
   outcomes (the ledger P6.28 verification method). Which two is the
   witness's choice.
5. **Fix loop:** any fault causing silent data loss or a duplicate/orphaned
   command is fixed and re-drilled until clean.

### 3.3 Per-fault REPORT table (ALL MEASURED CELLS EMPTY — UNRUN, human lab input)

| Fault | Detection time (measured) | Behaviour (observed) | Recovery time (measured) | Data loss (records) | Command issued-or-duplicated |
|---|---|---|---|---|---|
| Dropped messages | (EMPTY — UNRUN) | | | | |
| Reordered messages | (EMPTY — UNRUN) | | | | |
| Duplicated messages | (EMPTY — UNRUN) | | | | |
| Clock skew | (EMPTY — UNRUN) | | | | |
| Stale telemetry | (EMPTY — UNRUN) | | | | |
| Unavailable assets | (EMPTY — UNRUN) | | | | |
| Adapter crash | (EMPTY — UNRUN) | | | | |
| Partial fleet response | (EMPTY — UNRUN) | | | | |
| Central outage (30 min) | (EMPTY — UNRUN) | | | | |
| Edge restart | (EMPTY — UNRUN) | | | | |
| Solver timeout (cross-ref only) | n/a — P4.31 F9a/F9b platform suite | n/a | n/a | n/a | n/a |
| Plugin crash | n/a — no plugin host until Phase 7 | n/a | n/a | n/a | n/a |

### 3.4 Acceptance rule (recorded)

**Any fault causing silent data loss or a duplicate/orphaned command blocks
the exit gate until fixed and re-drilled.** Until the table in §3.3 is
filled from actual lab output, zero-faults claims are forbidden; the drill
report being EMPTY is the honest current state.

---

## 4. Decisions (DEFAULT-labelled where recommended, not decided)

| # | Decision | Status |
|---|---|---|
| D-1 | Doc filenames follow the report's `docs/operations/`, `docs/protocols/`, `docs/security/` layout (§8.1) exactly as the ledger names them; `docs/` had only `architecture/`, `foundations/`, `plan/`, so no convention conflict existed | DEFAULT |
| D-2 | The conformance-suite version logging is an ADDED STEP inside the existing `edge-agent-suite` job (which already is "its own CI job" per P6.19) rather than a duplicate job re-running the same suite | DEFAULT (additive discipline; avoids double execution) |
| D-3 | No path filters added to any existing job (see §2.1) — adding filters would restrict existing coverage | DEFAULT |
| D-4 | `deny.toml` allow list transcribes the recorded audit tables; verdicts come only from the first lockfile-backed scan. The advisory/scan jobs are config-only with no invented tool pins | DEFAULT |
| D-5 | The license review gate asserts the deny.toml D4 markers (RE-VERIFY, OpenEMS) so the re-verify obligations cannot be silently dropped from the config | DEFAULT |
| D-6 | The drill adds only edge/adapter-layer measurements; platform suites are cited, never re-run (no drift); solver timeout and plugin crash are cross-reference/N-A rows | DEFAULT (the ledger's own step-4 rule) |
| D-7 | P6.25 console surfaces are described in the ops guide as designed surfaces with status "in progress, UNRUN" — the parallel console work is not claimed | RECORDED constraint |

## 5. Human gate inputs (all EMPTY until executed)

| # | Gate input | How | Result |
|---|---|---|---|
| 1 | Green CI run on the phase branch (all jobs incl. the new ones) | push/PR; the run record is the P6.27 evidence | (EMPTY — UNRUN, never claimed) |
| 2 | Unlicensed-dependency demonstration on a scratch branch | introduce a test-unlicensed dependency, confirm `edge-license-gate` fails, remove it | (EMPTY — UNRUN) |
| 3 | First `cargo deny check` verdicts (licenses + advisories) once Cargo.lock exists | run the gate; diff the dependency set against `02_license_audit.md` tables | (EMPTY — UNRUN) |
| 4 | rustls/rustls-pemfile license RE-VERIFY against primary sources; OpenEMS license verification | D4 owners, primary sources | (EMPTY — UNRUN) |
| 5 | Install/enroll/upgrade lab execution per the operations guide | clean lab host, both architectures | (EMPTY — UNRUN) |
| 6 | The P6.28 drill execution + the independent-witness re-run of two faults | §3.2 procedure on lab hardware | (EMPTY — UNRUN) |

## 6. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in §8) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in §8) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (output quoted in §8) |
| CI execution (any job) | **UNRUN — human gate input 1** |
| Drill execution | **UNRUN — human gate input 6** |
| cargo-deny / cargo-audit / syft / gitleaks execution | **NOT RUN — banned in this loop** |

## 7. Permitted-tool output (actual, quoted at the end of the writing loop)

The three permitted tools were run after every file in this wave was
written; their ACTUAL output is quoted in the agent completion report and
summarized here:

| Tool | Result |
|---|---|
| `python tools/verify_docs.py` | (quoted in the completion report) |
| `python tools/validate_agent.py` | (quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | (quoted in the completion report) |
