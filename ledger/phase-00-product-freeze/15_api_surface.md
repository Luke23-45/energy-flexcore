# P0.15 — API resource surface freeze (candidate list, no implementation)

| Field | Value |
|---|---|
| Subtask | P0.15 — Freeze the API resource surface (candidate list, no implementation) |
| Report trace | §15.1 (API styles), §15.2 (candidate REST resources and prohibition), §10.5 (Protobuf and schema governance), §21.4 (SemVer for public contracts), §1.2 (external API surface) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.10 (asset/capability vocabulary), P0.11 (command state machine), P0.14 (event vocabulary) |
| Status | Draft for Stage 0 review |
| Decisions | D1–D6 remain OPEN (§25.2); this document decides none of them |

---

## 1. Candidate REST resources (§15.2 — all 16, verified line by line)

The §15.2 list is reproduced verbatim, in report order, and verified path by path against §15.2 (not taken from the ledger list). All 16 are candidates under the versioned base path `/api/v1`; §15.2 titles the list "Candidate REST resources", so nothing here is an implementation commitment.

| # | Resource path | Domain (cross-reference) |
|---|---|---|
| 1 | `/api/v1/tenants` | Tenancy (§16.2) |
| 2 | `/api/v1/portfolios` | Portfolio state (§6.5, §10.1) |
| 3 | `/api/v1/sites` | Site registry (§10.1) |
| 4 | `/api/v1/assets` | Asset registry and capability (§10.1, §11) |
| 5 | `/api/v1/grid-models` | Grid model contract (§12.4–§12.5) |
| 6 | `/api/v1/telemetry/query` | Telemetry query (§10.4 quality vocabulary) |
| 7 | `/api/v1/flexibility/envelopes` | Flexibility envelopes (§11) |
| 8 | `/api/v1/forecasts` | Forecasting |
| 9 | `/api/v1/schedules` | Scheduling (§12.1–§12.2) |
| 10 | `/api/v1/optimization-runs` | Optimization runs (§12.3 solver discipline) |
| 11 | `/api/v1/feasibility-checks` | Feasibility gate (§12.4) |
| 12 | `/api/v1/dispatch-commands` | Command lifecycle (§14; P0.11) |
| 13 | `/api/v1/market-commitments` | Market commitments |
| 14 | `/api/v1/plugins` | Plugin management (§5) |
| 15 | `/api/v1/audit-events` | Audit (§15.3 `OperatorActionRecorded`) |
| 16 | `/api/v1/simulation-runs` | Simulation Lab (§24 Stage 5) |

Count: 16 resources. The report presents the list verbatim as path literals; the "Domain" column above is this document's cross-reference aid, not report text. The ledger instruction's list matches the report exactly — no discrepancy found.

## 2. API styles (§15.1 — the five styles with their purposes)

Reproduced from the §15.1 table:

| Interface | Purpose |
|---|---|
| REST/JSON | External management API, CRUD, queries, operator actions, plugin management |
| gRPC/Protobuf | Internal worker RPC, compute services, edge service messages where appropriate |
| NATS subjects / JetStream | Durable event distribution, asynchronous work, telemetry pipelines, orchestration notifications |
| WebSocket/SSE | Live operator views and progress updates |
| WIT | Sandboxed WASM extension calls |

## 3. Versioning and contract governance

Rules frozen here, each traceable to the report:

1. **Versioned REST base path.** Use a versioned REST base path, for example `/api/v1` (§15.1). All §1 resource paths carry that prefix.
2. **Publish OpenAPI.** The external REST API publishes OpenAPI (§15.1).
3. **Protobuf cross-process with Buf checks.** Use Protobuf for cross-process contracts and generated clients; apply `buf lint` and `buf breaking` in CI; breaking schema changes must be deliberate and versioned (§10.5, [R17]).
4. **Public contracts follow SemVer.** The public contracts — REST API, Protobuf schemas, plugin WIT interfaces, and plugin manifests — follow Semantic Versioning (§21.4).
5. **0.x may break with a documented migration note.** The 0.x line may break contracts with a documented migration note (§21.4).
6. **v1.0.0 freezes the public API and plugin contracts for the 1.x line** (§21.4).

The scope of "public contracts" is the §21.4 list (REST API, Protobuf schemas, plugin WIT interfaces, plugin manifests); it does not include internal database schemas or NATS subject layouts, which §10.5 and §15.4 respectively leave to later design.

## 4. Prohibition: no generic command endpoint (§15.2)

Do not expose a generic "execute arbitrary command" endpoint (§15.2, closing sentence).

Consequences frozen here:

- Every route that mutates dispatch state must be one of the §1 resources (or a documented revision of the list), never a pass-through "execute" path added for demo convenience.
- Commands follow the frozen lifecycle (§14; `11_command_state_machine.md`): PROPOSED → VALIDATED → FEASIBLE → APPROVED → ISSUED → … with validation, feasibility, and authorization as distinct gates. An arbitrary-execute endpoint would bypass those gates and is therefore prohibited, not merely discouraged.

## 5. Schema rule: no raw database schemas as API contracts (§10.5)

Do not expose raw database schemas as public API contracts (§10.5, closing sentence).

Consequences frozen here:

- API payloads are contract types (Protobuf for cross-process, OpenAPI-described JSON externally), not database rows or table/DDL dumps.
- Internal schema evolution is governed by §6.5 and §16; it must not leak into the §1 surface as a public contract.

## 6. What this document does not do

- It freezes a candidate list only: no endpoints are implemented, and Phase 0 produces no code (AGENTS.md §1).
- It invents no additional resources, no per-resource method sets, no payload schemas, and no pagination/error conventions — §15.2 defines none, and adding them here would exceed the report.
- It makes no availability, throughput, or latency claims for any API.
- The entire 0.x line is simulation-only (§0, §2.1); nothing in this surface writes to devices in any 0.x release.

## 7. Acceptance-criteria coverage (ledger P0.15)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| All 16 §15.2 resources are listed under `/api/v1` | §1 (verified path-by-path against §15.2, report lines 1490–1507) |
| The five API styles from §15.1 are stated with their purposes | §2 (reproduced from the §15.1 table) |
| The no-arbitrary-command-endpoint prohibition is stated | §4 |
| The SemVer/migration-note rule for 0.x vs 1.x is stated | §3 (with §10.5 Buf checks and the no-raw-schema rule from §10.5 in §5) |

Verification method (ledger): comparison against §15.1–§15.2, §10.5, §21.4 — performed in §§1–5.
