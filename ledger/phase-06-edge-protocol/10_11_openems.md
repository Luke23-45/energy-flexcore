# P6.10 + P6.11 — OpenEMS integration: EdgeConfig discovery, channel mapping, and the read-only data path — decision record

| Field | Value |
|---|---|
| Subtasks | P6.10 — OpenEMS integration: EdgeConfig discovery and channel mapping; P6.11 — OpenEMS integration: read-only data path end to end |
| Status | **Implemented, awaiting human build/test.** The P6.11 LAB RUN has **not** happened: the lab-run record below is a TEMPLATE with EMPTY measured cells, status **PREPARED/UNRUN** |
| Wave | Phase 6, wave 3 (edge/protocol) |
| Report trace | §2.3; §18.1; §8.1; §23 Stage 6; §27 (OpenEMS references) |
| Loop context | Written under the recorded loop bans (`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`): nothing built, no test executed, no cargo invocation of any kind. Every "implemented" claim below is a **written-code** claim; nothing is tested or verified by execution. |
| Research cut-off | 7 November 2026 (report §27); RESEARCH.md §1 checked 2026-11-07 |

## 1. Per-subtask coverage against the ledger steps

### P6.10 — OpenEMS integration: EdgeConfig discovery and channel mapping

| Ledger step | Coverage | Where |
|---|---|---|
| 1. Fetch and parse EdgeConfig; record the OpenEMS version pinned for the lab | Fetch + parse implemented over a minimal read-only HTTP/1.1 GET client (std-only, no new dependencies). The pinned lab version is **NOT recorded as fact**: RESEARCH.md §1 records 2026.11.0 as the latest release (checked 2026-11-07), and the lab run must pin **the version the lab Edge actually runs** (RESEARCH.md open verification task 4) — the template in §7 carries the empty cell. | `crates/edge-agent/src/adapters/openems.rs` (`OpenEmsRestAdapter::discover`, `TcpHttpGetTransport`) |
| 2. Build the mapping Nature/Component/Channel → FlexCore asset type + telemetry point; record every unmapped and every ambiguous channel | Implemented as a **versioned configuration file** plus a discovery-time **register**: every discovered channel is classified exactly one of MAPPED / EXPLICITLY_UNMAPPED (with reason) / UNMAPPED (with reason) / AMBIGUOUS (with reason). The register is exposed for the decision record and tests. | `crates/edge-agent/config/openems-mapping-profile.v1.json`; `openems.rs` (`ChannelMappingStatus`, `mapping_register()`) |
| 3. Handle the OpenEMS execution model honestly (input–process–output cycle, Controllers, Scheduler) | Recorded in the adapter module header and here (§4 below): FlexCore READS channels and never injects into the cycle; no OpenEMS Controller is enabled or configured by this adapter; nothing in the module touches a Scheduler. | `openems.rs` module header; §4 of this record |
| 4. Write the mapping as a versioned configuration file, not hardcoded | Done: `openems-mapping-profile.v1.json`, envelope-checked (`profile` identity + `version` = 1); a different version is a named refusal, never an interpreted guess. | `config/openems-mapping-profile.v1.json`; `OpenemsMappingProfile::from_json_str` |

**Acceptance criteria status (P6.10):** "The lab Edge's full EdgeConfig parses" — **NOT met yet** (lab run pending); met against the FIXTURE EdgeConfig in tests only (`tests/openems_p6_10.rs`). "Every channel is mapped, explicitly unmapped with a reason, or flagged ambiguous; no channel is mapped by guessing its units" — implemented and exercised against the fixture; the lab verification is the §7 gate input.

### P6.11 — read-only data path end to end

| Ledger step | Coverage | Where |
|---|---|---|
| 1. Stand up the lab Edge with a known simulated asset set; record versions | **NOT DONE — cannot happen in this loop.** Template with EMPTY cells (§7). The test fixture (`simulated PV + battery per the OpenEMS Getting Started shape`, labelled FIXTURE) is a test double, never a stand-in for a lab Edge. | `tests/openems_e2e_p6_11.rs` (fixture); §7 (template) |
| 2. Run the read path for a window; reconcile value-for-value | Implemented **in-process**: the full composition (registry-constructed adapter → ACL → `AdapterIngestSource` → `FileLocalBuffer` → `EdgeRunner.tick`) reconciles fixture values field-by-field at BOTH ends. A tick-driven test, not a timed lab window. | `tests/openems_e2e_p6_11.rs` (`read_path_reconciles_value_for_value_at_both_ends`) |
| 3. Verify quality mapping end to end incl. forced stale source → `STALE`/`MISSING` downstream | Implemented at the edge boundary: a channel served beyond the profile's freshness limit arrives **`STALE`** at the uplink on the first poll after it went stale. `MISSING` is **never emitted by the edge** — it is a platform-derived gap marker (P2.04 D-T6); the control plane's gap detector owns the MISSING intervals downstream. The lab-side verification of that downstream behaviour is §7's empty cell. | `tests/openems_e2e_p6_11.rs` (`forced_stale_feed_arrives_stale_within_the_freshness_limit_and_never_missing`) |
| 4. Confirm by code inspection AND network capture that no write was sent | Code inspection: implemented as a **structural guarantee** (§3) — the transport trait declares GET only, and a lexical scan over the adapter module proves no other HTTP verb exists anywhere in it. The **network capture** part is a lab-run deliverable — §7's empty cell. | `openems.rs`; `tests/openems_e2e_p6_11.rs` (scan test) |

**Acceptance criteria status (P6.11):** "Reconciliation matches within stated tolerances for every mapped channel; stale feed is flagged downstream within the freshness limit; zero write calls observed" — the first two are **exercised in tests only**; the third is proven structurally (no write call is expressible) but the *network capture* evidence does not exist yet. The lab run (a real Edge, a timed window, a capture, an independent reconciliation) is the human's gate input.

## 2. The UNVERIFIED API-shape register

RESEARCH.md §1 records the OpenEMS **architecture** (Edge/Backend, OSGi components, Natures, Channels, "EdgeConfig is the JSON definition of the Edge's configuration", the input–process–output cycle, Controller.Api.Backend's API-key WebSocket) but **no REST endpoint paths, no JSON field names, and no response shapes**. Everything HTTP/JSON in the adapter is therefore an explicit assumption, labelled at its use site (`// UNVERIFIED-ASSUMPTION — ...`) and listed here. **The OpenEMS REST API was never executed when this code was written.**

| # | Assumption | Where used | Verify at the lab Edge by |
|---|---|---|---|
| A1 | EdgeConfig is fetched at `GET /rest/edge/config` | `OPENEMS_EDGE_CONFIG_PATH` | Diff the real endpoint path (OpenEMS REST API docs; the Edge may instead expose JSON-RPC over WebSocket) |
| A2 | The EdgeConfig JSON carries `components` as an **object keyed by component id**; each component carries `factoryId`, `alias`, and a `channels` object keyed by channel id | `discover_inner` | Dump the real EdgeConfig and diff the shape (including whether `components` is an object or an array) |
| A3 | Each channel entry carries an optional `unit` string (e.g. `"W"`, `"var"`, `"%"`) and human text fields | `discover_inner` (`source_declared_unit`) | Diff the real channel metadata keys (`unit` vs `unitName` vs none) |
| A4 | A channel value is fetched at `GET /rest/channel/{component}/{channel}` | `OPENEMS_CHANNEL_PATH_PREFIX` | Diff the real per-channel read path |
| A5 | The channel-value response is an object with a numeric `value` and a numeric `timestamp` (Unix epoch **milliseconds**) | `read` | Capture a real response; confirm the timestamp unit and field names. A missing timestamp is a named `ReadFailed` — `event_time` is never fabricated |
| A6 | REST channel values carry **no quality code**; each profile entry therefore DECLARES the code attached to a fresh reading | `ProfileMapping::declared_quality_code` | Confirm whether OpenEMS exposes any channel validity/quality information at all; if it does, extend the adapter's declared quality mapping explicitly |
| A7 | Component/channel ids are REST-path-safe (`[A-Za-z0-9._-]`); anything else is recorded unmapped | `is_rest_path_safe` | Check real id character sets (e.g. `charger1`, `_meta`) |
| A8 | The Edge's clock-synchronisation status is **UNKNOWN** | `AdapterCapabilities::clock_quality = ClockQuality::Unknown` | Record the Edge's NTP/synchronisation state in the lab run |

Sign conventions are **declarations, not verifications**: the profile declares `ess0/ActivePower` export-positive, `meter0/ActivePower` import-positive, and reactive power injection-positive — each labelled UNVERIFIED-ASSUMPTION **inside the profile's own `assumptions` array**, announced at startup, and correctable by a profile edit (never a code change) when the lab shows the real sign. No unit or sign is ever inferred from observed values.

OpenEMS version pin: RESEARCH.md §1 (checked 2026-11-07) records **2026.11.0** as the latest monthly release and warns that component IDs and channels MOVE between releases (2026.11.0 removed Energy Scheduler v1). The lab run must pin the version it actually runs and re-diff the mapping (§7, empty cell).

## 3. The structural read-only guarantee (P6.11 depends on it)

1. **Trait-level:** `OpenemsRestTransport` declares exactly ONE method, `get`. No caller of the adapter surface can express a state-changing request against the Edge.
2. **Client-level:** `TcpHttpGetTransport` formats exactly one request shape — `GET <path> HTTP/1.1` with `Connection: close` — and nothing else.
3. **Module-level lexical scan:** `tests/openems_e2e_p6_11.rs` scans `src/adapters/openems.rs` for writer method definitions, writer request-builder call shapes, and verb string literals; the scan fails the build's test pass if any appears. The scan patterns are deliberately broad (they also catch commented-out or renamed writers).
4. **Runtime witness:** the test double counts GETs (1 EdgeConfig + N channel reads) and carries a `write_attempts` counter that the adapter surface cannot increment (not expressible) — asserted zero.
5. **Execution-model honesty:** the adapter reads channels only; it enables/configures NO OpenEMS Controller and touches NO Scheduler. FlexCore does not inject into the input–process–output cycle (AGENTS.md rule 3; report §18.1). The EmsAdapter port itself has no dispatch/write method (P6.12); the command path remains nonexistent (P6.14 owns the disabled-by-default receiver behind its written gate).

## 4. Execution-model honesty note (ledger P6.10 step 3)

OpenEMS Edge runs an input–process–output (IPO) cycle: a Cycle component sets the process image, then executes Controllers in Scheduler order (RESEARCH.md §1, OpenEMS Edge architecture). FlexCore is a supervisory orchestration layer: through this adapter it observes channels produced by that cycle and never injects into it. Nothing in `openems.rs` writes a setpoint, enables a Controller, or orders the Scheduler; the adapter cannot do so even accidentally because its entire outbound surface is GET-only (§3). This stays true in the lab run: standing up the lab Edge with simulated devices (Getting Started shape) is OpenEMS's own simulation running its own controllers — FlexCore only reads the resulting channels.

## 5. Versioned-profile design and fail-closed mapping rules (DEFAULT-labelled decisions)

| # | Decision | Label |
|---|---|---|
| D-10.1 | The mapping lives in `crates/edge-agent/config/openems-mapping-profile.v1.json` with an envelope check (`profile` identity string + integer `version`). This adapter understands version 1 only; a v2 file is a named refusal, and a migration is a reviewed change. | DEFAULT |
| D-10.2 | Every discovered channel is classified into exactly one register class — MAPPED / EXPLICITLY_UNMAPPED (with reason) / UNMAPPED (with reason: "no mapping-profile v1 entry") / AMBIGUOUS (with reason). Ambiguity causes: duplicate profile entries for one point; a point declared both mapped and unmapped; unknown quantity or unit spellings; an active-power entry without a declared native sign; a unit/quantity family mismatch; a source-declared unit that conflicts with (or is unnameable against) the profile unit. None of these is ever resolved by guessing. | DEFAULT |
| D-10.3 | Reactive convention: the profile carries one top-level `reactive_source_convention` (site-level declaration, P0.07 §3) with an optional per-entry override; the convention travels on every reactive record, never applied as a silent flip. | DEFAULT |
| D-10.4 | Quality: profile entries DECLARE the code attached to a fresh reading (`declared_quality_code`); an entry without the declaration yields readings with NO quality, which the ACL refuses (`QualityNotDeclared`). Readings older than the profile's `stale_after_millis` are stamped `STALE` by the adapter (an explicit declared freshness limit) and map to canonical `STALE` through the frozen ACL table. The adapter never emits `MISSING` (platform-derived, P2.04 D-T6). | DEFAULT |
| D-10.5 | Source sequence: edge-assigned, monotonically increasing per point, incremented per successful read (P0.09 §1). | DEFAULT |
| D-10.6 | Transport timeouts: connect 5 s, read 10 s — composition constants this wave; promoting them to per-site configuration is a reviewed follow-up. | DEFAULT (recorded) |
| D-10.7 | Adapter selection reuses the existing REQUIRED `FLEXCORE_EDGE_ADAPTER_ID` (P6.12) — no second selection variable was introduced. New additive variables: `FLEXCORE_EDGE_OPENEMS_BASE_URL` (only `http://` composes; absent → composition refuses) and `FLEXCORE_EDGE_OPENEMS_MAPPING_PROFILE` (defaults to the shipped v1 file). | DEFAULT |
| D-10.8 | The adapter is constructed THROUGH the registry (`AdapterRegistry::with_openems_available()` → `construct`) — see §6. | DEFAULT |

## 6. Registry flip: PLANNED → AVAILABLE, and the recorded follow-up

P6.10 landed the concrete `openems-rest` adapter, so the id is now **AVAILABLE**. Recorded precisely:

- `AdapterRegistry::with_openems_available()` (new, additive) flips `openems-rest` to `AdapterStatus::Implemented`, and the composition root (`main.rs`) composes through `registry.construct(...)` — the factory inside the registry call builds AND discovers the adapter and hands the validated mapping table to the ACL/ingest composition.
- **`with_default_allowlist()` intentionally keeps the legacy PLANNED entry.** The pre-existing integration test `crates/edge-agent/tests/adapter_acl_and_registry.rs` pins the default table's PLANNED status for `openems-rest` (two assertions), and that file is **outside this wave's ownership list** (parallel-agent constraint). Flipping the DEFAULT table is a ONE-LINE follow-up for that file's owner (change the two assertions to `AdapterStatus::Implemented` after `with_default_allowlist()` adopts the flip); until then the composition opts in explicitly. This is recorded here rather than silently absorbed (AGENTS.md rule 5 discipline).
- P6.13 (fault-injection tests against the real protocol) remains **open** and is not implied by this flip.

## 7. P6.11 lab-run record — TEMPLATE, status PREPARED/UNRUN (EMPTY measured cells)

**Every cell below is deliberately EMPTY. No figure in this record is measured, captured, or witnessed. The human executing the lab run fills these cells; nothing here stands in for that evidence.**

### 7.1 Versions and environment

| Item | Value (EMPTY — fill at the lab run) |
|---|---|
| FlexCore edge agent build (git SHA) | |
| OpenEMS Edge version (the version the lab Edge ACTUALLY runs — not assumed) | |
| OpenEMS EdgeConfig dump reference (file/commit where the captured config is stored) | |
| Java/JRE version on the Edge host | |
| Lab topology reference (per the D6 review, `03_d6_boundary_review.md`) | |
| Run window (start/end, timezone, UTC offsets) | |

### 7.2 Asset set and mapping diff

| Item | Value (EMPTY) |
|---|---|
| Simulated asset set stood up (Getting Started shape or pilot devices) | |
| Hand-checked mapping table produced from the OpenEMS documentation (ledger P6.10 verification method) | |
| Diff result: profile entries confirmed / corrected (list each correction) | |
| Channels discovered but not in the fixture expectations (list) | |
| Sign-convention findings (ess0 ActivePower, meter0 ActivePower, reactive) and the profile edits they force | |

### 7.3 Reconciliation table (ledger P6.11 step 2)

| Channel | Fixture/lab value at the Edge | Canonical value at the uplink | Tolerance | Match? (EMPTY) |
|---|---|---|---|---|
| ess0/ActivePower | | | | |
| ess0/ReactivePower | | | | |
| ess0/Soc | | | | |
| pvInverter/ActivePower | | | | |
| pvInverter/ReactivePower | | | | |
| meter0/ActivePower | | | | |
| meter0/ReactivePower | | | | |
| (any additional lab channel) | | | | |

### 7.4 Stale-feed evidence (ledger P6.11 step 3)

| Item | Value (EMPTY) |
|---|---|
| How the feed was stopped (simulator halted, channel frozen, …) | |
| Time from feed stop to downstream `STALE` flag | |
| The freshness limit in force (`stale_after_millis`) during the run | |
| Downstream `MISSING` derivation (the control plane's gap detector) evidence | |
| Capture references (log lines / telemetry records) | |

### 7.5 No-write capture review (ledger P6.11 step 4)

| Item | Value (EMPTY) |
|---|---|
| Network capture method and location (span port / mirror / Edge-side) | |
| Capture duration and byte/packet counts | |
| Reviewer who inspected the capture | |
| Result: HTTP methods observed toward the Edge (expected: GET only) | |
| Result: any write/setpoint/controller-configuration call observed (expected: none) | |

### 7.6 Run record

| Item | Value (EMPTY) |
|---|---|
| Executed by / witnessed by / date | |
| Exit status of the run book steps | |
| Deviations from this record | |

## 8. Honest limitations (what this wave does NOT claim)

1. **Nothing was built or run.** The crate (including all prior waves) carries a latent compile defect outside this wave's ownership: `crates/edge-agent/src/ingest.rs` imports `PointMapping` from `crate::adapters::port`, but the type is defined in `crate::adapters::acl` — a one-line import correction is needed before the first human build. This wave's code imports it from the correct path; the defect is recorded, not silently fixed (ownership constraint).
2. **No TLS on the OpenEMS REST hop.** Only `http://` composes this wave; a `https://` base URL is a named refusal. A rustls-backed transport for the REST hop is the recorded open item — deliberately not written against an unverifiable rustls API under the no-build ban. The FlexCore edge↔central uplink is unaffected (P6.07 mTLS; RESEARCH.md R6-C1: the two channels must not be conflated, and OpenEMS's native API-key WebSocket is adapter-internal business).
3. **No uplink wire format was invented.** The P6.07 `BatchPayloadEncoder` seam still refuses encoding, so `main.rs` keeps the fail-closed uplink: the read path and the durable buffer are real; nothing is forwarded until the wire-format owner implements the encoder and composes the P6.06 credential material into `MtlsUplinkChannel::compose`. Exactly this is stated in the startup log.
4. **The std-only JSON reader** (no new dependencies were permitted) handles objects, arrays, strings with escapes/surrogate pairs, numbers, booleans, null, a 64-level depth bound, and duplicate keys keeping the first occurrence; numbers are f64 (values beyond 2^53 lose integer precision — recorded; EdgeConfig ids are strings and lab telemetry is far below the limit).
5. **The EdgeConfig fixture is a fixture.** The tests' EdgeConfig/channel JSON is labelled FIXTURE and follows the UNVERIFIED-ASSUMPTION shape (§2). It is not a capture from any Edge, and no test result substitutes for the §7 lab evidence.
6. **Encryption at rest for the durable buffer** (§18.3) remains the P6.09-recorded open item, unchanged by this wave.
7. **No conformance claim of any kind** arises from this wave: implementing a protocol adapter is not a conformance statement (report §2.3, §17.5; AGENTS.md rule 2), and P6.19 owns the claim boundary later.
