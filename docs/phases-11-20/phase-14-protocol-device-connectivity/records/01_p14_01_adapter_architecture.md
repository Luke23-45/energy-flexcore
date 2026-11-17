# P14.01 — Protocol adapter architecture and crate layout

| Field | Value |
|---|---|
| Subtask | P14.01 — the phase's architecture record: the `crates/adapters/protocol-*` layout, the common adapter-ports crate, per-adapter capability manifests, additive `tools/check_dependency_rule.py` updates, the simulator-first testing policy, and the external-events-enter-at-PROPOSED rule |
| Wave / agent | Phase 14 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the two Agent-A crates (this batch) and the whole Phase 0–10 base are written, never compiled; the first human build is the program gate (conventions §7.1). No protocol is connected, no device is reached, no message is exchanged. |
| Sources | [S-1] IEEE 2030.5-2018 (via gap analysis §11), [S-3] SunSpec CSIP Implementation Guide v2.1, [S-5] QualityLogic Intro to 2030.5, [S-10] OpenADR Alliance, [S-14] IEEE 1815.2-2025, [S-16] SunSpec Modbus, [S-39] MQTT/Sparkplug secondary, [S-156] SunSpec IEEE 2030.5 in production; seams: `crates/edge-agent/src/adapters/port.rs` (P6.12), `crates/edge-agent/src/adapters/registry.rs` (P6.12), `crates/flexcore-grid-api/src/scope.rs` (P4.02), `crates/flexcore-ports/src/`, `crates/flexcore-kernel/src/manifest.rs` |
| Files touched | `crates/flexcore-protocol-ports/` (NEW: Cargo.toml, src/lib.rs, src/scope.rs, src/manifest.rs, src/connection.rs), `crates/adapters/protocol-20305/` (NEW — P14.02's crate, landed in this batch per the subtask), root `Cargo.toml` (additive members + workspace deps), `tools/check_dependency_rule.py` (additive entries) |

## 1. The crate layout (proposals + this batch's landings)

The Phase 14 protocol surface is one common port crate plus one crate per
protocol. **Crate additions follow conventions §6.2: a crate is listed as a
member only in the batch that gives it real code** — the unlanded rows below
are proposals in this record, never scaffolding.

| Crate (proposed path) | Package name | Status in this batch | Role | First real code |
|---|---|---|---|---|
| `crates/flexcore-protocol-ports` | `flexcore-protocol-ports` | **LANDED (this batch)** | CORE crate: the common protocol scope declaration, capability manifest, and connection lifecycle every protocol adapter implements | this batch |
| `crates/adapters/protocol-20305` | `flexcore-protocol-20305` | **LANDED (this batch)** | COMPOSITION crate: IEEE 2030.5 AggregatorServer (P14.02) | this batch |
| `crates/adapters/protocol-openadr` | `flexcore-protocol-openadr` | proposal — Agent B, wave 1 | OpenADR 3.0 VTN (P14.05) | P14.05 batch |
| `crates/adapters/protocol-modbus` | `flexcore-protocol-modbus` | proposal — Agent B, wave 1 | SunSpec Modbus client (P14.07) | P14.07 batch |
| `crates/adapters/protocol-dnp3` | `flexcore-protocol-dnp3` | proposal — Agent B, wave 1 | DNP3 outstation-first (P14.08) | P14.08 batch |
| `crates/adapters/protocol-104` | `flexcore-protocol-104` | proposal — Agent A, wave 2 | IEC 60870-5-104 client (P14.09) | P14.09 batch |
| `crates/adapters/protocol-61850` | `flexcore-protocol-61850` | proposal — Agent A, wave 2 | IEC 61850-7-420 MMS client (P14.10) | P14.10 batch |
| `crates/adapters/protocol-mqtt` | `flexcore-protocol-mqtt` | proposal — Agent A, wave 2 | MQTT 5 + Sparkplug B uplink (P14.11) | P14.11 batch |
| `crates/adapters/protocol-cim` | `flexcore-protocol-cim` | proposal — Agent A, wave 2 | CIM/CGMES + 61968-5 import (P14.12) | P14.12 batch |

Naming decision (E-decision-adjacent, recorded here as the P14.01 DEFAULT,
not closed): the common crate is **`flexcore-protocol-ports`**, alongside
`flexcore-ports` (the proposal in plan §2 P14.01). Rationale: (a) it keeps
the platform's existing port layer free of protocol vocabulary (the P1.07
no-vendor-types gate stays intact); (b) the `protocol-` infix namespaces
the protocol adapter surface without nesting `flexcore-ports` deeper; (c)
the checker classifies it as a CORE crate with an EMPTY allowed internal
edge set — it depends on no workspace crate, exactly like `flexcore-grid-api`'s
self-contained `scope.rs`. Adapter crates land under `crates/adapters/`
(the P1.13 composition-side directory) with the `protocol-` infix, so the
checker's existing `crates/adapters/*` composition classification extends
naturally.

## 2. The common port layer (`flexcore-protocol-ports`)

Three contracts, one per module:

| Module | Contract | The rules it encodes |
|---|---|---|
| `scope.rs` | `ProtocolScopeDeclaration` | Declared never inferred; **no `Default` impl**; mandatory non-empty `not_modelled` (an empty list is a construction error); fail-closed `assert_supports` refusing out-of-scope role/control/resource requests; a validated `authority_statement` that MUST name that external events enter the command lifecycle at PROPOSED and that the adapter is NOT a dispatch authority (mirrors `flexcore-grid-api/src/scope.rs` field-for-field, per phase-14 context §3 constraint 4) |
| `manifest.rs` | `ProtocolAdapterManifest` + `AuthorityDeclaration` | The per-adapter capability manifest: protocol identity (id + declared edition string — a design pin, NEVER a conformance claim), the scope declaration held BY VALUE (the two cannot diverge), a closed `DeclaredCapability` vocabulary, a declared non-zero cadence, and a **one-variant** `AuthorityDeclaration::SupervisoryRelayOnly` — dispatch authority is not expressible in the type; adding a variant is a reviewed ADR change against ADR-010 and the never-granted set |
| `connection.rs` | `ProtocolConnection` port | The connect/disconnect lifecycle and honest connection state (CLOSED / CONNECTING / CONNECTED / DEGRADED / UNREACHABLE — the state names the P11 observability consumers map onto). Deliberately NO write path and NO data path: absence is structural (the P6.12 `EmsAdapter` rule carried forward). Sync traits (the `flexcore-ports` house rule); adapters own their I/O shape |

Dependency line: serde only. No `flexcore-contracts` dependency —
canonical-unit and quality-state translation stays at the edge-agent ACL
(the P6.12 pattern); the protocol port layer carries protocol-shaped
declarations only.

## 3. The external-events-enter-at-PROPOSED rule (binding for every adapter)

**External control objects (2030.5 DERControls, OpenADR events, 1547
settings changes) enter the command lifecycle at PROPOSED and pass the
validation, feasibility, and authorization gates like any other candidate.**
An adapter that constructs an APPROVED or ISSUED state from external input
is a defect, not a shortcut (plan §7.4). Enforcement is structural, not
conventional: the manifest's authority statement must name the rule
(construction error otherwise); `AuthorityDeclaration` has no dispatch
variant; and the 2030.5 mapping crate types its output with the FROZEN
`flexcore-domain::command_state::CommandState` so the frozen transition
table — which has no PROPOSED → ISSUED edge — is the enforcement point
(record 02 §5). The never-granted permission set
(`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) is
untouched and stays byte-identical (conventions §6.4).

## 4. Per-adapter capability manifests

Every protocol adapter constructs exactly one `ProtocolAdapterManifest` at
composition time, holding its `ProtocolScopeDeclaration` by value. The
manifest is the single reviewed surface a composition root reads:
adapter id (the registry-allowlist key shape), protocol id, declared
protocol edition, scope (roles / control scope / modelled resource
classes / not_modelled / authority statement), capabilities (read,
discovery, command-relay, external-event-ingest, model-import,
bounded-history), cadence (the P14.20 polling-budget seam), and the
authority declaration. Nothing about an adapter is inferred from its wire
behaviour — the declaration IS the adapter's contract, and the
`coverage_limitation_statements()` of its scope surface verbatim wherever
its outputs are consumed (the P4.02 coverage-limitation pattern).

## 5. Checker updates (additive)

`tools/check_dependency_rule.py` gained, additively:

1. `CORE_CRATES`: `flexcore-protocol-ports` (core: adapters depend on it;
   no core crate depends on it).
2. `ALLOWED_INTERNAL_EDGES`: `"flexcore-protocol-ports": set()` (depends on
   no workspace crate; serde sits outside the internal-edge check).
3. `COMPOSITION_CRATES`: `flexcore-protocol-20305` (composition side, like
   every `crates/adapters/*` crate; nothing depends ON it except future
   composition roots).
4. `MEMBER_DIRS`: `crates/flexcore-protocol-ports`,
   `crates/adapters/protocol-20305`.

Agent B's entries (openadr/modbus/dnp3) are theirs to land in their batch,
the same way; the shared-file discipline is re-read-before-edit (plan §3).

## 6. The simulator-first testing policy

Every protocol adapter is testable with ZERO hardware, before any field
surface exists:

1. **No adapter lands without its simulated counterpart named.** The
   simulated 2030.5 client, OpenADR VEN, and Modbus device land in
   `flexcore-simulator` in wave 3 (P14.17), the same program as the
   adapters they exercise (plan §7.7). Until then, adapter crates ship
   model/port code and written-unrun tests only.
2. **The `simulated-test` discipline carries over** (the P6.08 pattern):
   simulated counterparts are TEST-ONLY — they name themselves in their
   ids and outputs, and no production composition root may wire them for a
   real site. Simulated data is never evidence about real devices
   (ledger P14.17 evidence rule).
3. **Written-unrun test suites are evidence preparation, never results.**
   The 2030.5 suite (record 04) is UNRUN; CSIP certification is a
   human-gated lab process (conventions §7.3, P14.19).
4. **Hardware claims are human lab gates entirely** — no agent runs a lab,
   reaches a device, or exchanges a message, ever (plan §6 acceptance 2).

## 7. ASSUMPTION / UNVERIFIED / UNRUN register

1. **UNRUN** — every module of both landed crates and every test file:
   never compiled, never executed. Evidence cells EMPTY. The first human
   build gates the workspace (conventions §7.1).
2. **UNVERIFIED (protocol detail, later waves)** — the protocol specifics
   each adapter crate encodes (2030.5 attribute spellings and hrefs in
   record 02; SunSpec registers in P14.07; DNP3 objects in P14.08) are
   cited from secondary sources and carry UNVERIFIED labels at their
   definitions; verification against the standard text / a lab Edge is an
   implementation-time human gate (plan §7.6).
3. **REGISTERED DEFAULT, not closed** — the `flexcore-protocol-ports`
   naming (§1). A reviewer may rename before more crates land; the cost is
   one manifest name and two checker lines today, and grows with every
   landed adapter crate — which is why this record lands FIRST (P14.01 is
   the batch's first artifact).
4. **Never-granted set** — byte-identical before and after this batch
   (`crates/flexcore-kernel/src/manifest.rs` untouched).

## 8. Handoff notes

1. **Agent B (wave 1)**: consume this record before landing
   `protocol-openadr`/`protocol-modbus`/`protocol-dnp3`; re-read the root
   `Cargo.toml` and checker before your additive edits (A landed two
   members and four checker entries); reuse `flexcore-protocol-ports` — do
   NOT fork a second scope/manifest implementation.
2. **Wave 2 / wave 3**: the proposals table (§1) names the remaining
   crates; P14.17 owns the simulated counterparts; P14.20/P14.21 consume
   the manifest cadence and the `ConnectionState::name()` vocabulary.
3. **Human gates**: first build of the workspace; the standard-text
   verification pass; the P14.19 lab-evidence path.
