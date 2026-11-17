# Phase 14 — Wave 3, Agent A close-out (P14.17–P14.18, P14.20–P14.21)

| Field | Value |
|---|---|
| Batch | Phase 14 wave 3, Agent A: P14.17 (simulator integration), P14.18 (edge-agent seam extension), P14.20 (backpressure/rate limiting), P14.21 (protocol observability). P14.19 is NOT this agent's (Agent B, documentation-only). |
| Status | WRITTEN-UNRUN throughout. Nothing was built (no `cargo build`/`check`/`fmt`/`clippy`), no test was run (no `cargo test`), nothing was installed. The three permitted verification tools were the ONLY build-adjacent verification (outputs quoted verbatim in §3). |
| Honesty boundary | No protocol connected, no device reached, no message exchanged, no server bound, no broker contacted, no publish acknowledged, no profile parsed from real bytes, no metric observed, no alert fired, no conformance/certification/security claim anywhere. The simulated counterparts are code the tests will compose; every composing test is WRITTEN-UNRUN and execution is a human gate. |
| Ledger | NOT edited — no checkbox ticked by this agent (ticking is the coordinator's exact-match procedure; hard ban 6). |
| Decision records | `81_p14_17_simulated_counterparts.md`, `82_p14_18_edge_agent_seam_extension.md`, `83_p14_20_backpressure_rate_limiting.md`, `84_p14_21_protocol_observability.md`, this close-out |

## 1. Files created / modified (complete list)

**Created:**

| File | Notes |
|---|---|
| `crates/flexcore-simulator/src/adapters/mod.rs` | The counterparts' module map + the hard boundary (record 81 §2) |
| `crates/flexcore-simulator/src/adapters/sim_20305.rs` | Simulated 2030.5 device client + scripted in-process `AggregatorTransport` (record 02 §5 seam) + tests |
| `crates/flexcore-simulator/src/adapters/sim_openadr.rs` | Simulated OpenADR VEN over `VtnRegistry`/`DeliveryPlanner` (record 21 §3 surface) + tests |
| `crates/flexcore-simulator/src/adapters/sim_modbus.rs` | Simulated Modbus device over `ModbusTransport` (record 23 §4 seam) + tests |
| `crates/flexcore-simulator/src/adapters/sim_dnp3.rs` | Simulated DNP3 master over `Dnp3Transport` (record 24 §4 seam) + fixture inventory + tests |
| `crates/flexcore-simulator/src/adapters/sim_mqtt.rs` | Simulated MQTT broker sink over `MqttUplinkTransport` + the REAL `BirthDeathTracker` (record 43 §2 seam) + tests |
| `crates/flexcore-simulator/src/adapters/sim_cim.rs` | CIM fixture source over `RdfProfileSource` (record 44 §2 seam) + tests |
| `crates/flexcore-simulator/src/adapters/permissions.rs` | The fleet's kernel-side permission binding (`flexcore_kernel::adapter_permissions`; record 63 §6 handoff) + tests |
| `crates/flexcore-simulator/src/doe_profile.rs` | The DOE behaviour profile (record 63 §5 handoff; consumes the domain's `doe_envelope` semantics) + tests |
| `crates/flexcore-simulator/tests/p14_17_simulated_adapters.rs` | The integration suite — WRITTEN-UNRUN |
| `crates/flexcore-protocol-ports/src/observability.rs` | The shared P14.20/P14.21 taxonomy + budget vocabulary (records 83/84) + tests |
| `crates/adapters/protocol-modbus/src/backpressure.rs` | The gateway-budget binding (record 83 §5) + tests |
| `crates/adapters/protocol-mqtt/src/backpressure.rs` | The per-edge-node uplink budgets (record 83 §5) + tests |
| `crates/adapters/protocol-dnp3/src/backpressure.rs` | The outstation event-queue/report-rate binding (record 83 §5) + tests |
| `crates/adapters/protocol-openadr/src/backpressure.rs` | The agreed-poll-frequency/fleet-connection binding (record 83 §5) + tests |
| `crates/edge-agent/src/adapters/edge_to_cloud.rs` | The edge-to-cloud mapping + fleet uplink budgets (record 82 §3–§4) + tests |
| `deploy/compose/observability/prometheus/rules/protocol_adapter_alerts.yml` | The ADDITIVE P14.21 rule file (record 84 §2) |
| `docs/…/records/81…85_*.md` | The four decision records + this close-out |

**Modified (additive or recorded):**

| File | Notes |
|---|---|
| `crates/flexcore-simulator/src/lib.rs` | Two additive module declarations |
| `crates/flexcore-simulator/Cargo.toml` | Eight additive dependency lines, each with the recorded rationale (record 81 §1) |
| `Cargo.toml` (root) | ONE additive `[workspace.dependencies]` line: `flexcore-kernel = { path = "crates/flexcore-kernel" }` (the kernel was a member but absent from the single-version table) |
| `crates/flexcore-protocol-ports/src/lib.rs` | ONE additive module declaration |
| `crates/adapters/protocol-{openadr,modbus,dnp3,mqtt,cim}/src/lib.rs` | ONE additive `ADAPTER_ID` const each (record 81 §6 / record 82 §2) |
| `crates/adapters/protocol-modbus/src/transport.rs` | ONE additive error variant `WindowOutOfRange` + Display arm (record 81 §2) |
| `crates/adapters/protocol-mqtt/src/transport.rs` | ONE additive error variant `PublishRefused` + Display arm (record 81 §2) |
| `crates/adapters/protocol-{modbus,mqtt,dnp3,openadr}/src/lib.rs` | ONE additive `pub mod backpressure;` line each |
| `crates/edge-agent/src/adapters/registry.rs` | `generic-rest` REMOVED (record 82 §1); six PLANNED protocol ids; id-table + inline tests updated |
| `crates/edge-agent/src/adapters/mod.rs` | `edge_to_cloud` module + doc lines |
| `crates/edge-agent/src/lib.rs` | ONE doc-table line |
| `crates/edge-agent/Cargo.toml` | Two additive dependency lines (`flexcore-mqtt-adapter`, `flexcore-protocol-ports`) with recorded rationale |
| `crates/edge-agent/tests/adapter_acl_and_registry.rs` | The RESERVED-slot test pins the removal + the PLANNED ids (a removal necessarily updates the tests that pinned the slot) |
| `crates/edge-agent/tests/conformance_suite_p6_19.rs` | Case 10 pins the PLANNED refusal + the removed id (refusal assertions preserved) |

**Byte-untouched, verified:** `crates/flexcore-kernel/src/manifest.rs`
(the never-granted set), `crates/flexcore-domain/src/command_state.rs`
(the frozen lifecycle), `crates/flexcore-simulator/src/{scenario,
profiles,engine,source,rng}.rs` (the frozen Phase 2 simulator — the DOE
profile is a separate module, record 81 §4), `apps/control-plane/**`
(the P11 metric families and the alarm surface), the three existing
Prometheus rule files, `docs/foundations/*`, `docs/plan/*`, `.agent/*`,
`ledger_phase_14.md`, and every Agent-B path (records 101–119, operator
docs).

## 2. Decisions of record (none silent; full reasoning in records 81–84)

1. **The dependency-rule arbiter allowed simulator→adapter edges** —
   `flexcore-simulator` is a COMPOSITION crate; the simulated
   counterparts live in the simulator (record 81 §1). No new crate, no
   checker entry.
2. **`generic-rest` REMOVED, not spec'd** — the placeholder's premise
   expired; the extension rule is "an id is registered when its
   implementation lands, never before" (record 82 §1).
3. **The six Phase 14 protocol ids registered PLANNED** in the
   edge-agent registry per the P14.02 contract (record 82 §2).
4. **The shared P14.20/P14.21 vocabulary is an additive module on the
   CORE `flexcore-protocol-ports`** — the one placement every adapter
   can consume (record 83 §2). The common crate's API surface is
   otherwise unchanged; the MQTT publish-capability vocabulary gap
   (record 43 §2) stays the reviewed human handoff it already was.
5. **Two additive seam-error variants** (`WindowOutOfRange`,
   `PublishRefused`) — the minimum honest refusals the simulated
   counterparts needed (record 81 §2); every existing match verified
   unaffected.
6. **The v1 scenario schema is UNCHANGED** — the DOE profile is a
   composition-time module, not a new `AssetType` (record 81 §4).
7. **2030.5 and CIM carry no new backpressure module** — their budget
   surfaces already exist (`ServerLimits`, the one-way batch parse);
   recorded rather than duplicated (record 83 §5).

## 3. Permitted-tool outputs (verbatim)

Run from the repo root, after the last code and record edit (the
batch's final state); all three exited 0.

```text
$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 27 workspace members.
EXIT=0
```

(Encoding note, recorded honestly — the same artifact records 05, 45,
and 65 document: the script's literal text is `OK — dependency rule
holds for 27 workspace members.`; the em-dash renders as `ù` in this
Windows console's code page. **Count provenance:** this batch added no
workspace member and no checker entry — the 27 members are the wave-1
and wave-2 counts unchanged. The batch DID add dependency EDGES
(simulator→adapters/protocol-ports/kernel; edge-agent→mqtt-adapter/
protocol-ports), all composition→composition or composition→core, all
legal under the checker's policy (composition crates are skipped from
the edge check entirely; only core→composition is forbidden) — and the
green run above is the checker's own confirmation over the post-batch
manifests.)

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0
```

```text
$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0
```

(A lexical scan for `TODO`/`FIXME`/`todo!`/`unimplemented!` over every
crate and file this batch touched returned NO PLACEHOLDERS FOUND.)

## 4. Deviations (recorded, none silent)

1. **Tool-discipline deviation on five lib.rs edits:** the five
   additive `ADAPTER_ID` constants were first applied via a Bash
   Python-heredoc instead of the Edit tool. The heredoc carried
   exact-match `count==1` and end-of-file assertions (the same safety
   property the ledger-tick procedure requires), the appended content
   was verified byte-for-byte against the intended diff afterwards, and
   every subsequent edit in the batch used the Edit tool. No
   verification tool was invoked through Python and no ledger line was
   touched by any script. Recorded here because the conventions' letter
   reserves Python heredocs for exact-match ledger ticks; the deviation
   is tool-discipline only — content, intent, and verification path are
   unchanged. A later mechanical rename inside one test file also used
   `sed` (three identical tokens); same class, same honesty statement.
2. **No deviation against the hard bans.** Nothing installed, nothing
   built, no test executed, no placeholder, no new external dependency
   (every new Cargo line is an existing workspace pin: the six adapter
   crates, `flexcore-protocol-ports`, `flexcore-kernel`), no
   ledger/plan/context/foundations/.agent edit, no checkbox tick, no
   E-decision closed (P14-E-1/2/3 remain OPEN with their DEFAULTs).
3. **Honest limitation carried forward:** every new module and test is
   WRITTEN-UNRUN against a workspace that has never compiled; the first
   human build may surface recorded defects (e.g. hand-built fixtures
   that must compile against validating constructors). Failures at that
   gate are fixed as code edits with a record update, never silently.

## 5. Handoff notes

**To Agent B (wave 3) / the close record (P14.22–P14.24):** the four
records above (81–84) are the phase's wave-3 decision records — cite
them, do not edit them. The generic-rest removal (record 82 §1) and the
PLANNED registry registrations (record 82 §2) are the operator-doc
surface for P14.23's edge sections. The alert-rule file (record 84 §2)
is the ops-tree artifact to reference in the close/exit-gate record.

**To the orchestrator (tick flips):** P14.17 → record 81; P14.18 →
record 82; P14.20 → record 83; P14.21 → record 84. No checkbox was
touched by this agent.

**To the next implementation wave (post-P14):** (a) the common
`DeclaredCapability` publish-vocabulary gap (record 43 §2, consumed
as-is by the simulated MQTT counterpart and its permission binding) is
still OPEN as a reviewed common-layer change — a human decision, not a
drive-by; (b) the registry's PLANNED ids flip to Implemented only by a
reviewed code landing with a mapping-table plan and a P6.02 audit note;
(c) the message-rate alert's registered placeholder budget (record 84
§3) must be confirmed or mechanically updated against the composition's
declared `MessageRateBudget` when the first real adapter lands; (d) the
OCPP simulated counterpart is NAMED for Phase 17 (plan §2 P14.17) and
deliberately not built here.

**To the human gates:** (a) the first workspace build — every module
and test in this batch is WRITTEN-UNRUN; (b) the written-unrun test
suites' execution (the simulator integration suite, the per-adapter
backpressure tests, the shared-vocabulary tests, the edge-agent
registry/mapping tests); (c) the standard-text verification passes
inherited from records 02/21/23/24/43/44/63 (this batch added none);
(d) operator sign-off of the registered alert thresholds (record 84);
(e) the live Prometheus evaluation and alert-routing dry run (UNRUN).
