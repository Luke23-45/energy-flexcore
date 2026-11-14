<!-- P10.02 — Public API and plugin-contract freeze (decision record). FREEZE RECORD over the CURRENT surfaces: the Buf lint/breaking runs, the OpenAPI diff, and the manifest/WIT compatibility validation are NOT RUN here (nothing built, run, or installed — ENTRY_NOTE boundary 6); every build-evidence cell is UNVERIFIED. The freeze POLICY and the drift-DETECTION procedure are recorded; execution is a human/CI gate (P10.18). -->
# P10.02 — Public API and Plugin-Contract Freeze (1.x Line)

Date: 2026-11-14. Subtask: P10.02 (`ledger_phase_10.md` §P10.02).
Owner role: Platform engineering. Report trace: §21.4 (v1.0.0 freezes the
public API and plugin contracts for the 1.x line); §10.5 (Buf lint/breaking);
§9.3 (SemVer, compatibility matrix); §15.1–§15.5 (REST, event vocabulary,
NATS); §10.1–§10.3 (entities, units, time).
Inputs: P10.01 scope freeze (`01_release_scope_freeze.md`); the current
surfaces inventoried below with their repository paths; P1.14/P1.15 (NATS
subject mapping and stream topology — cross-references, not re-builds);
P0.14 (event vocabulary), P0.15 (API surface), P1.05, P1.18 (Buf
governance), P5.07–P5.08 (plugin manifest, WIT worlds), P7.05–P7.07, P7.21.

## 1. Honest status — the plain statement

**This is a freeze POSITION over the current surfaces, not an executed
freeze proof.** The contract inventory below was taken from the actual
repository code by reading it; nothing was built, run, or installed while
writing this record, so:

- **Buf lint and Buf breaking-change detection: NOT RUN.** The `buf` CLI is
  not installed in this environment and the loop bans forbid installing or
  running it. The Buf configurations exist (`proto/buf.yaml`,
  `proto/buf.gen.yaml`, `python/workers/contracts/buf.yaml`,
  `python/workers/contracts/buf.gen.yaml`) and the exact commands are
  recorded in §6 — the runs are the human/CI gate (P10.18).
- **OpenAPI diff / breaking check: NOT RUN.** There is no generated OpenAPI
  spec in the repository to diff yet — the REST surface exists as the frozen
  candidate list (P0.15), not as a generated spec (P1.18 records that the
  internal Protobuf contracts land with the first worker RPC; no REST
  implementation exists).
- **Plugin manifest / WIT compatibility validation: NOT RUN as a tool;**
  the WIT packages are pinned at `@1.0.0` in the repository and the platform
  plugin-API version is pinned in code at `0.5.0` — the evidence below is
  code citation, not build output.
- **Every build-evidence cell in the compatibility table (§7) is
  UNVERIFIED** until P10.18 runs the §22.2 gates against the release
  candidate and files the logs.

Ticking P10.02 reads "implemented, awaiting human build/test", per
ENTRY_NOTE.md boundary 6.

## 2. The frozen public contracts — inventory of the CURRENT surfaces

Each row names the code artifact that IS the freeze anchor: a change to that
artifact without a recorded, versioned change is drift (procedure in §6).

| # | Contract surface | Freeze anchor (repository path) | Frozen content at this freeze |
|---|---|---|---|
| 1 | REST base path | `crates/flexcore-contracts/src/api_paths.rs::API_BASE_PATH` | `/api/v1` — every public resource path carries the versioned base path (§15.1) |
| 2 | REST resource list | `crates/flexcore-contracts/src/api_paths.rs::REST_RESOURCE_PATHS` (16 paths, §15.2 order) | `tenants`, `portfolios`, `sites`, `assets`, `grid-models`, `telemetry/query`, `flexibility/envelopes`, `forecasts`, `schedules`, `optimization-runs`, `feasibility-checks`, `dispatch-commands`, `market-commitments`, `plugins`, `audit-events`, `simulation-runs` |
| 3 | REST prohibition | same file (P0.15 rule) | **No generic "execute arbitrary command" endpoint** — every route that mutates dispatch state must be one of the frozen resources or a documented revision of the list (§15.2) |
| 4 | Internal Protobuf governance | `proto/buf.yaml` (lint `STANDARD`, breaking `FILE`), `proto/buf.gen.yaml` | Born-under-Buf rule (P1.18): no .proto exists at the module root yet; the first one lands under lint + breaking discipline |
| 5 | Python worker RPC contract | `python/workers/contracts/worker_api.proto` — `package flexcore.worker.v1`, proto3, Buf-governed by `python/workers/contracts/buf.yaml` | The worker protocol the Python numerical/research workers speak (§7.4, §5.3 Class C); version suffix `v1` frozen |
| 6 | Plugin WIT worlds | `wit/flexcore-grid@1.0.0/grid.wit` (world `grid-model-provider-plugin`, interface `grid-model-provider`); `wit/flexcore-optimizer@1.0.0/optimizer.wit` (world `optimizer-plugin`, interface `optimizer`); `wit/flexcore-telemetry@1.0.0/telemetry.wit` (world `telemetry-source-plugin`, interface `telemetry-source`); shared types `wit/deps/flexcore-core@1.0.0/types.wit` (package `flexcore:core@1.0.0`, interface `types`) | Three plugin worlds + the shared core types package, all pinned at package version `1.0.0` (§5.5, §9.3) |
| 7 | Plugin manifest + platform API version | `crates/flexcore-kernel/src/resolve.rs::CURRENT_PLATFORM_API_VERSION = SemVer::new(0, 5, 0)`; manifest rules `crates/flexcore-kernel/src/manifest.rs`, guide `docs/plugin-sdk/manifest-guide.md` | The plugin manifest schema and the compatibility rule `covers_api_version(CURRENT_PLATFORM_API_VERSION)` — a plugin not covering `0.5.0` is refused by the resolver (§9.3) |
| 8 | NATS event vocabulary | `crates/flexcore-events/src/events.rs::FROZEN_EVENT_NAMES` (26 names, §15.3 order) and `EVENT_COUNT = 26` | All twenty-six §15.3 events, verbatim spellings: `AssetRegistered`, `AssetCapabilityUpdated`, `TelemetryReceived`, `TelemetryQualityChanged`, `AssetAvailabilityChanged`, `FlexibilityEnvelopeUpdated`, `ForecastPublished`, `MarketSignalReceived`, `ScheduleProposed`, `ScheduleValidated`, `FeasibilityCheckCompleted`, `DispatchProposed`, `DispatchApproved`, `DispatchRejected`, `DispatchIssued`, `DispatchAcknowledged`, `DispatchTrackingUpdated`, `DispatchCompleted`, `DispatchFailed`, `DispatchExpired`, `DispatchCancelled`, `DispatchSuperseded`, `DispatchFallbackActivated`, `PluginActivated`, `PluginFailed`, `OperatorActionRecorded` |
| 9 | Event subject mapping | `crates/flexcore-events/src/subject.rs` + `crates/adapters/nats/src/topology.rs::EVENT_SUBJECT_PREFIX` | `flexcore.v1.<area>.<event_slug>` — one event, exactly one subject; lower_snake_case slugs; **no tenant identifier in any subject** (P1.14 D-J2); DLQ subjects outside the namespace: `flexcore.dlq.v1.<area>.<event_slug>` (P1.14 §4.2) |
| 10 | JetStream stream configuration | `crates/adapters/nats/src/topology.rs` (P1.15, implementing P1.14 §3–§5) | Stream inventory and names (`FLEXCORE_REGISTRY`, `FLEXCORE_STATE`, `FLEXCORE_TELEMETRY`, `FLEXCORE_FLEXIBILITY`, `FLEXCORE_FORECAST`, `FLEXCORE_MARKET`, `FLEXCORE_SCHEDULING`, `FLEXCORE_VALIDATION`, `FLEXCORE_DISPATCH`, `FLEXCORE_PLUGINS`, `FLEXCORE_AUDIT`, `FLEXCORE_DLQ`); per-stream policy shape: `Limits` retention (replayable within the configured window), `File` storage, `DiscardOld`, replication 1; durable pull consumers with `AckExplicit`; acks only after the processing transaction commits |
| 11 | Canonical entities / units / time | `crates/flexcore-domain/src/entities.rs` + `dispatch_command.rs` (P1.06); `crates/flexcore-contracts/src/units.rs`, `time_model.rs` (P0.07, P0.09); vocabulary P0.10 | §10.1 entity vocabulary, §10.2 units (MW/MVAr/MVA/MWh; `P > 0` export; declared Q sign; SOC fraction in [0,1] at the API boundary), §10.3 time fields (`event_time`, `ingest_time`, `valid_from`/`valid_until`, source sequence/version, UTC internally) |
| 12 | Python worker template protocol | `python/workers/worker_template/`, `python/workers/contracts/` | The template protocol the shipped Python workers follow (request/response framing over the `flexcore.worker.v1` RPC contract) |

**What is NOT frozen by this record:** deployment-configuration values of the
JetStream per-stream limits (max-age/max-msgs/max-bytes/duplicate-window) —
P1.14 D-J4/D-J5 deliberately make those deployment configuration, never design
values; the freeze covers the stream inventory, names, subject filters, policy
SHAPE, and consumer discipline. Per-resource REST method sets, payload
schemas, pagination, and error conventions are likewise not frozen here — the
P0.15 candidate list is a resource list, and their freeze lands with the
generated OpenAPI spec (§5, gap G-1).

## 3. The freeze policy (recorded position)

1. **SemVer contract rule (§21.4, §9.3).** The public contracts — REST API,
   Protobuf schemas, plugin WIT interfaces, and plugin manifests — follow
   Semantic Versioning. **The 0.x line may break contracts with a documented
   migration note; from v1.0.0 the public API and plugin contracts are frozen
   for the 1.x line.** Any 1.x change to a §2 surface is at most
   backward-compatible (additive); a breaking change requires a major version.
2. **Deprecation rule (recorded position for the 1.x line).** A surface may
   be *deprecated* before it is removed: the deprecated form stays functional
   and is marked deprecated in the contract metadata (OpenAPI
   `deprecated: true`, Protobuf field option, WIT `@deprecated`), release
   notes (P10.26) name it, and removal happens only in a major version —
   never silently inside 1.x. Renaming, dropping, or re-subjecting a §15.3
   event without a versioned change IS a breaking change (ledger step 6),
   not a deprecation.
3. **Versioned-change discipline.** Every change to a §2 anchor is
   (a) deliberate — recorded in the Phase 10 ledger with the subtask that
   made it, (b) versioned — the package/resource version moves per SemVer
   (`/api/v2` base path for a breaking REST change; WIT package `@2.0.0` for
   a breaking world change; a new `.proto` file version for a breaking RPC
   change), and (c) recorded — the compatibility matrix (§7) gains a row.
   A contract changed without a version bump is a release-blocking finding
   (the P10.02 risk row: a pilot integration silently broken).
4. **No raw database schemas as public contracts (§10.5).** API payloads are
   contract types; the §2 anchors contain no table/DDL dumps, and internal
   schema evolution must not leak into the public surface (P10.13 owns the
   migration-side check).
5. **No plugin may bypass the gates the contracts protect (§5.7, ADR-005).**
   The plugin surfaces (WIT worlds, manifests) are capability-scoped; a
   manifest declaring capabilities the platform does not grant is refused at
   resolution (`covers_api_version` + capability checks, P7.05–P7.07).

## 4. The fourteen §10.1 canonical entities — coverage as of this freeze

The ledger step 5 requires the frozen contracts to cover all fourteen
canonical entities. Honest coverage position:

| Entity (§10.1) | Contract-type coverage today |
|---|---|
| Tenant, Portfolio, Site, Asset, AssetState | Modelled as vocabulary-faithful domain types in `crates/flexcore-domain/src/entities.rs` (P1.06 Stage-1 subset), with typed IDs (`ids.rs`), tenant-chain derivation (P0.16), and canonical units |
| DispatchCommand | `crates/flexcore-domain/src/dispatch_command.rs` (P1.06) — §14.3 construction invariants |
| AuditRecord | `crates/flexcore-audit/src/record.rs` (`AuditRecordFields`) |
| Schedule / schedule revision | `crates/flexcore-storage/src/schedule_revision.rs`, `crates/flexcore-application/src/schedule_comparison.rs` (P3.22) |
| CapabilityEnvelope | `crates/flexcore-contracts/src/capability.rs` + `crates/flexcore-application/src/envelope_staleness.rs` |
| GridModel | Reference/traceability types (`crates/flexcore-grid-api/src/port.rs::GridModelRef`, `optimization_result_record.rs::GridModelTraceability`); no full GridModel entity type yet |
| ForecastBundle, DispatchExecution, MarketCommitment, PluginInstance | P0.10 §1 vocabulary; no dedicated contract type yet |

**Recorded finding (not silently absorbed):** the fourteen-entity coverage is
COMPLETE at the vocabulary level (P0.10 §1) but PARTIAL at the contract-type
level, and there is no generated OpenAPI spec yet in which entity coverage
could be mechanically proven. Ledger acceptance criterion (f) is therefore
**OPEN** as item O-1 (§8): the fourteen-entity / units / time-field check
completes when the REST surface is generated and diffed (the human/CI gate),
with the check commands in §6. What IS verifiable today: the §10.2 unit types
are enforced by construction in the modelled entities (`ActivePowerMW`,
`EnergyMWh` — never bare `f64`), SOC is a fraction in [0,1] at the API
boundary (P0.07), reactive-power sign is declared per grid model / device
profile (never assumed), and the §10.3 time fields exist as distinct
contract types in `flexcore-contracts/src/time_model.rs` (`EventTime`,
`IngestTime`, `ValidFrom`, `ValidUntil` — UTC internally).

## 5. Known contract gaps — carried, not new

These are the Phase 7 follow-ups and recorded decisions the freeze inherits;
they are cited, not re-decided (ENTRY_NOTE boundary 7):

| Gap | Carried in |
|---|---|
| No REST implementation / no generated OpenAPI spec yet | P1.18 decision (internal contracts first; REST lands with the first REST adapter) — gap G-1 of this record |
| No .proto at the internal module root yet (worker contract lives under `python/workers/contracts/`) | P1.18 — the internal `.proto` set lands with the first worker RPC |
| Plugin-contract gaps from the Phase 7 conformance wave | `ledger/phase-07-plugin-sdk/30_retrospective_release_gate.md` open items |
| JetStream streams beyond the four provisioned ones created lazily by first publisher | P1.14 §4.2 / P1.15 (`topology.rs` header) |

## 6. Freeze-verification and drift-DETECTION procedure (described; human/CI gate)

The drift detector is a two-layer procedure. Layer 1 is mechanical and
cheap — the §2 anchors are code constants whose in-module tests already pin
them (P0.15 path list, P1.09/P0.14 event names, P1.15 topology); re-running
those suites detects any unrecorded change to the anchored surfaces. Layer 2
is the tool-based contract check (Buf, OpenAPI diff, WIT validation) that
runs from a clean checkout in CI (P10.18) against the previous published
contracts.

### 6.1 Layer 1 — mechanical re-extraction (commands)

```sh
# REST surface: re-extract the frozen list and diff against §2 row 2.
grep -n '"/api/v1/' crates/flexcore-contracts/src/api_paths.rs

# Prohibition check: no execute pass-through anywhere in the route list.
grep -rn "execute\|run-command" crates/flexcore-contracts/src/api_paths.rs
#   -> expected: no route hit (the word appears only in the rule comment).

# Event vocabulary: re-extract the 26 names in order and diff against §2 row 8.
sed -n '/pub const FROZEN_EVENT_NAMES/,/^]/p' crates/flexcore-events/src/events.rs

# Subject mapping and stream inventory: re-extract and diff against §2 rows 9–10.
grep -n "EVENT_SUBJECT_PREFIX\|DLQ_SUBJECT_PREFIX\|pub const STREAM_" \
  crates/flexcore-events/src/subject.rs crates/adapters/nats/src/topology.rs

# Platform plugin-API version: must still read 0.5.0.
grep -n "CURRENT_PLATFORM_API_VERSION" crates/flexcore-kernel/src/resolve.rs

# WIT package versions: must still read @1.0.0.
grep -rn "^package" wit/ | sort

# Worker RPC package: must still read flexcore.worker.v1.
grep -n "^package" python/workers/contracts/worker_api.proto
```

Any line that differs from §2 is **drift**: either revert the change, or
record it as a deliberate, versioned change (policy §3.3) and update §2/§7.
The Layer-1 suite re-run is part of `cargo test --workspace` (P10.18).

### 6.2 Layer 2 — tool-based checks (commands; NOT RUN here)

```sh
# Protobuf lint + breaking (§10.5, §22.2) — against the previous published
# contracts; buf version recorded in the run report (tool not installed here).
buf lint proto/
buf breaking proto/ --against '<previous-published-ref>'
buf lint python/workers/contracts
buf breaking python/workers/contracts --against '<previous-published-ref>'

# Plugin manifest / WIT compatibility validation (§22.2) — the P7 validator
# run against the frozen contracts:
cargo test -p flexcore-kernel --test conformance_p7_21      # human gate
cargo test -p flexcore-kernel --test resolve_p7_06          # human gate

# REST/OpenAPI: generate the spec from the code (never hand-written), then
# diff against the previous published spec:
#   (generation command recorded by the REST adapter subtask when it lands)
#   oasdiff breaking <prev-openapi.yaml> <openapi.yaml>       # human gate
```

Layer-2 acceptance, per the ledger: Buf lint zero errors; Buf breaking zero
unintended breaking changes (an intended change is listed with its versioning
decision); manifest/WIT validation passes; the OpenAPI diff reports zero
unintended breaking REST changes, every §15.2 resource present, and no
generic execute endpoint. **None of these results is claimed by this
record** — the filed report is P10.18's.

## 7. Compatibility-promises matrix — build evidence UNVERIFIED

| Contract | Version frozen | Consumed by | Promise | Build evidence |
|---|---|---|---|---|
| REST API base path | `/api/v1` | Operator console, external integrations | Additive-only within 1.x; breaking change ⇒ `/api/v2` | UNVERIFIED (no generated spec; no diff run) |
| REST resource list | 16 resources (§2 row 2) | same | list changes are documented revisions of the list; no execute pass-through | UNVERIFIED (Layer-1 extract not run by CI yet) |
| Internal Protobuf | (none at module root yet; governance live via `proto/buf.yaml`) | cross-process Rust services | born-under-Buf; breaking ⇒ new file version | UNVERIFIED (buf not run) |
| Worker RPC contract | `flexcore.worker.v1` | Python numerical/research workers (§7.4) | breaking ⇒ `flexcore.worker.v2` package | UNVERIFIED (buf not run) |
| WIT worlds | `flexcore:grid@1.0.0`, `flexcore:optimizer@1.0.0`, `flexcore:telemetry@1.0.0`, `flexcore:core@1.0.0` | WASM plugins (Class B, §5.4) | additive-only within 1.x; breaking ⇒ `@2.0.0` package | UNVERIFIED (wit-tools not run; code pin cited) |
| Plugin manifest / API version | platform plugin-API version `0.5.0` (`CURRENT_PLATFORM_API_VERSION`) | all third-party plugins | a plugin must cover `0.5.0`; resolver refuses otherwise; the frozen value may advance only compatibly within 1.x | UNVERIFIED (resolver tests UNRUN in this environment) |
| Event vocabulary | 26 §15.3 names, verbatim (§2 row 8) | all event consumers, audit trails | rename/drop/re-subject = breaking change ⇒ versioned change | UNVERIFIED (in-module name tests cited, not run) |
| Subject mapping | `flexcore.v1.<area>.<event_slug>`; DLQ `flexcore.dlq.v1.…` | NATS consumers | scheme frozen per P1.14 D-J1/D-J2 | UNVERIFIED (same) |
| JetStream topology | stream inventory + policy shape (§2 row 10) | durable work/event consumers | names/filters/policy-shape frozen; limit VALUES are deployment config (D-J4/D-J5) | UNVERIFIED (NATS adapter tests UNRUN) |
| Units/time contracts | §10.2 units; §10.3 time fields | every API consumer | MW/MVAr/MVA/MWh; `P > 0` export; declared Q sign; SOC ∈ [0,1]; UTC internally | UNVERIFIED (domain/contracts tests cited, not run) |

The matrix covers every §2 surface (ledger acceptance criterion (d)); the
evidence column is the honest part — no cell is verified by this record.

## 8. Open items — traced to the carried registers

| Open item | Owner / carried in |
|---|---|
| O-1: fourteen-entity + units + time-field mechanical check over a GENERATED OpenAPI spec (criterion f) | P10.18 CI run + the REST-adapter subtask that generates the spec (G-1 above); findings land in this record's §4 table |
| O-2: Buf lint/breaking results (criteria a–b) | P10.18 (§22.2 gates); the run report is filed there and referenced here |
| O-3: manifest/WIT validation run (criterion c) | P10.18; P7.21/P7.06 suites |
| O-4: OpenAPI diff vs previous published spec (criterion e) | P10.18 — requires G-1 first; until then "zero unintended REST breaking changes" is VACUOUSLY TRUE only because no spec exists, and is NOT claimed as a pass |
| O-5: Phase 7 plugin-contract follow-up gaps | `ledger/phase-07-plugin-sdk/30_retrospective_release_gate.md` |
| O-6: D4 cargo-deny allowlist question (affects the SBOM side of §9.3, not the contract surface) | `.agent/DECISIONS.md`; P10.19 |
| O-7: support commitment for the release (what "supported" means) | P10.21 (recorded there as undecided or decided) |

## 9. Reviewer-verification procedure

1. **Anchor check:** open every §2 anchor path and confirm the frozen content
   matches the table (spot-check rows 2, 8, 9, 10 against the files).
2. **Drift-procedure check:** run the §6.1 Layer-1 commands; every result
   must match §2. A mismatch is a finding — record it as drift per §3.3, do
   not silently update §2.
3. **Policy check:** confirm §3 states the SemVer freeze, the deprecation
   rule, and the versioned-change discipline; confirm no text here promises
   a removal inside 1.x.
4. **Honesty check:** confirm §1 claims no run results and §7 shows
   UNVERIFIED in every evidence cell; confirm §5 cites the carried gaps
   rather than absorbing them.
5. **Status check:** WRITTEN-UNRUN — nothing was built, run, or installed;
   the Layer-2 commands in §6.2 are the human/CI gate (P10.18), and the
   repository verification commands (`python tools/verify_docs.py`,
   `python tools/validate_agent.py`) are the human's to run after any edit.

Build/test status (honest): **WRITTEN-UNRUN.** The inventory was taken by
reading the repository; the tick on P10.02 reads "implemented, awaiting
human build/test."
