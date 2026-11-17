# Phase 14 — Wave 1, Agent A close-out (P14.01–P14.04)

| Field | Value |
|---|---|
| Scope | P14.01 (adapter architecture + crate layout), P14.02 (IEEE 2030.5 AggregatorServer), P14.03 (2030.5 security record), P14.04 (2030.5 conformance test suite) — records 01–04 |
| Agent | Phase 14 wave 1, Agent A |
| Status | WRITTEN-UNRUN throughout. Nothing was built (no `cargo build`/`check`), no test was run (no `cargo test`), nothing was installed. The three permitted verification tools were the ONLY build-adjacent verification (outputs quoted verbatim in §3). |
| Honesty boundary | No protocol connected, no device reached, no message exchanged, no server bound, no TLS handshake, no certificate issued, no CA stood up, no conformance claim anywhere. |

## 1. Files created / modified (complete list)

| File | Created/Modified | Notes |
|---|---|---|
| `crates/flexcore-protocol-ports/Cargo.toml` | CREATED | CORE crate; serde-only dependency line |
| `crates/flexcore-protocol-ports/src/lib.rs` | CREATED | Module map + the layer's rules |
| `crates/flexcore-protocol-ports/src/scope.rs` | CREATED | `ProtocolScopeDeclaration` mirroring `flexcore-grid-api/src/scope.rs`: no `Default`, mandatory non-empty `not_modelled`, fail-closed `assert_supports`, validated authority statement |
| `crates/flexcore-protocol-ports/src/manifest.rs` | CREATED | `ProtocolAdapterManifest` + one-variant `AuthorityDeclaration::SupervisoryRelayOnly` + closed `DeclaredCapability` vocabulary |
| `crates/flexcore-protocol-ports/src/connection.rs` | CREATED | `ProtocolConnection` port: connect/disconnect lifecycle, honest `ConnectionState` (CLOSED/CONNECTING/CONNECTED/DEGRADED/UNREACHABLE); no write path, no data path |
| `crates/adapters/protocol-20305/Cargo.toml` | CREATED | COMPOSITION crate; flexcore-protocol-ports + flexcore-domain + serde + serde_json; NO transport dependency |
| `crates/adapters/protocol-20305/src/lib.rs` | CREATED | Honest-status header; `ADAPTER_ID`/`PROTOCOL_ID`/`PROTOCOL_EDITION` |
| `crates/adapters/protocol-20305/src/model.rs` | CREATED | CSIP resource-tree model types (EndDevice(+List), FSA(+List), DERProgram(+List), DefaultDERControl, DERControl(+List), DERControlBase, Response(+List), Pricing(+List), LogEvent(+List), Sfdi/Lfdi/TimeType/ListMeta) with unit tests |
| `crates/adapters/protocol-20305/src/addressing.rs` | CREATED | Validated `ResourceHref`, declared `AddressPlan` (UNVERIFIED `csip_default()`), `ResourceQuery` paging, declared `resolve_primacy` rules |
| `crates/adapters/protocol-20305/src/scheduling.rs` | CREATED | `DerControlWindow`: `effective_start`, half-open `active_at`, `overlaps`; saturating arithmetic |
| `crates/adapters/protocol-20305/src/server.rs` | CREATED | AggregatorServer skeleton: BTreeMap store, declared-plan routing, bounded lists, fail-closed `MtlsRequirements`, `DnsSdAnnouncement`, `AggregatorTransport` UNRUN seam + `ResourceRequestHandler` |
| `crates/adapters/protocol-20305/src/proposal.rs` | CREATED | DERControl → `ExternalControlProposal` at CONST PROPOSED; frozen-table `reject_at_gate`/`adapter_may_transition_to`; primacy-based `resolve_competing_controls` |
| `crates/adapters/protocol-20305/tests/csip_usecase_matrix.rs` | CREATED | P14.04 part 1: use cases U1–U10 — WRITTEN-UNRUN |
| `crates/adapters/protocol-20305/tests/primacy_and_lifecycle.rs` | CREATED | P14.04 part 2: P1–P4 + L1–L4 — WRITTEN-UNRUN |
| `Cargo.toml` (root) | MODIFIED (additive) | 2 member entries + 2 workspace-dependency entries (see §2) |
| `tools/check_dependency_rule.py` | MODIFIED (additive) | 4 additive entries (see §2) |
| `records/01_p14_01_adapter_architecture.md` | CREATED | The architecture record |
| `records/02_p14_02_aggregator_server.md` | CREATED | The P14.02 record |
| `records/03_p14_03_security_design.md` | CREATED | The P14.03 record (record-only depth) |
| `records/04_p14_04_conformance_test_suite.md` | CREATED | The P14.04 record |
| `records/05_p14_wave1_agent_a_close_out.md` | CREATED | This record |

Untouched, verified: `crates/flexcore-kernel/src/manifest.rs` (the never-granted
set is byte-identical), `docs/foundations/*`, `docs/plan/*`, `.agent/*`,
`ledger_phase_14.md` (no checkbox ticked by this agent — ticking is the
coordinator's exact-match procedure), and every Agent-B path.

## 2. The exact workspace-member and checker diffs

**Root `Cargo.toml`** — two additive member lines (with explanatory
comments) appended at the end of the `members` list:

```toml
    "crates/flexcore-protocol-ports",
    "crates/adapters/protocol-20305",
```

and two additive lines in `[workspace.dependencies]`:

```toml
flexcore-protocol-ports = { path = "crates/flexcore-protocol-ports" }
flexcore-protocol-20305 = { path = "crates/adapters/protocol-20305" }
```

**`tools/check_dependency_rule.py`** — four additive entries:

1. `CORE_CRATES`: `"flexcore-protocol-ports"` (with a comment citing this
   record set).
2. `ALLOWED_INTERNAL_EDGES`: `"flexcore-protocol-ports": set()` — the
   protocol-ports layer depends on NO workspace crate.
3. `COMPOSITION_CRATES`: `"flexcore-protocol-20305"`.
4. `MEMBER_DIRS`: `"crates/flexcore-protocol-ports"`,
   `"crates/adapters/protocol-20305"`.

No existing line was modified or removed; Agent B's crates
(`protocol-openadr`, `protocol-modbus` were present as directories mid-batch)
were not referenced, and B's entries remain B's to land. A re-read of both
shared files preceded each edit (plan §3 discipline).

## 3. Permitted-tool outputs (verbatim)

Run after the last code edit (this batch's final state); all three exited 0.

```text
$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 22 workspace members.
```

(Encoding note, recorded honestly: the script's literal text is
`OK — dependency rule holds for 22 workspace members.`; the em-dash renders
as `ù` in this Windows console's code page. The output is the pre-existing
script's own success line; the count 22 = the 20 pre-existing members + the
2 members this batch added. Agent B's crates are not yet in MEMBER_DIRS.)

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
```

```text
$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
```

## 4. Deviations and decisions (all recorded, none silent)

1. **`flexcore-protocol-ports` depends on no workspace crate** — even
   `flexcore-contracts` was left out: the scope declaration is
   self-contained (the grid-api precedent) and canonical-unit translation
   stays at the edge-agent ACL. Consequence: the allowed internal edge set
   is empty, which is stricter than the plan's minimum.
2. **`ExternalControlProposal` does not derive serde** — the frozen domain
   `CommandState` carries no serialization surface; adding one to a
   protected vocabulary through a backdoor derive was rejected. The source
   enum (`ExternalEventSource`) keeps serde (its `Sfdi` payload already
   carries it).
3. **Zero-cap defense in depth** — `AggregatorServer::new` re-validates
   `ServerLimits`/`MtlsRequirements` even though checked constructors
   exist, because the struct fields are public.
4. **P14.02's crate landed inside the P14.01 batch** — the subtask text
   explicitly ordered it ("Then create YOUR crates"); the P14.01 record's
   proposal table marks both as LANDED and the rest as proposals.
5. **No HTTP/TLS/EXI dependency of any kind** — the transport is the
   designed UNRUN seam (record 02 §5); the workspace dependency line is
   unchanged.

## 5. Handoff notes

1. **Agent B (wave 1)**: records 01 (architecture) and 02 are your
   prerequisites; re-read root `Cargo.toml` + checker before your additive
   entries (A added two members / four checker lines); reuse
   `flexcore-protocol-ports` — no second scope/manifest implementation.
2. **Wave 2 (Agent A)**: the crate proposals for 104/61850/mqtt/cim and
   their checker entries follow the same pattern; record 02 §5's transport
   seam is the template for "design the transport, land no dependency".
3. **Wave 3**: P14.17 (simulated 2030.5 client) makes the P14.04 suite
   executable end-to-end; P14.20/P14.21 consume the manifest cadence and
   the `ConnectionState::name()` vocabulary (P11 taxonomy consumed, never
   redefined).
4. **Human gates**: (a) the first workspace build — every module and test
   here is WRITTEN-UNRUN and the first build may surface recorded defects;
   (b) the standard-text verification pass (record 02 §4, record 03 §3);
   (c) the transport implementation decision (record 02 §5).
5. **Ledger**: no checkbox was ticked by this agent; the coordinator
   applies the tick flips with the exact-match procedure using these
   record citations.
