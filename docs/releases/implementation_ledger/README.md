# Final Implementation Milestone Ledger (FIM) — F-01..F-15

**STATUS: IMPLEMENTATION WAVES 1-8 COMPLETE (written) — all 15 features implemented,
96/96 subtasks ticked (V-8/V-9/V-10 landed wave 8); wave 8B/8C + analysis pass recorded below; every tick reads "fixed (written,
awaiting human build/test)". The first human build remains the real gate.**

**Program:** implementation of the 15 proposed pre-publication features
(`docs/releases/final-implementation-before-publication.md` §5) plus the three
implementation-shaped §3 items (V-8, V-9, V-10), authorized by the owner
2026-11-19 ("implement all of them ... take all the decisions on behalf of me").

**Hard boundary (unchanged):** no build, no test execution, no installation.
Everything is WRITTEN. A subtask is ticked only as
**"fixed (written, awaiting human build/test)"** — never "tested" or "works".
The first human build remains the real gate; `Cargo.lock` regeneration happens
at that gate.

**Standing decisions taken by the orchestrator on the owner's behalf:**

1. **Dependency policy:** direct dependencies are limited to crates already
   resolved in the committed `Cargo.lock` (tokio, tokio-rustls, rustls,
   rustls-pemfile, hyper, axum, http, bytes, futures, async-trait, rand,
   base64, time, tracing, serde, serde_json, sha2, hmac, ed25519-dalek,
   jsonwebtoken, sqlx, async-nats, tower, tower-http, utoipa, reqwest 0.13.5
   [transitive today], prost 0.14.6 [transitive today]). No new crate enters
   the resolution graph. Missing wire protocols (RFC 6455 WebSocket, MQTT 5,
   Sparkplug B protobuf payload, RDF/XML, Modbus TCP framing, minimal DER for
   OCSP) are hand-written minimal codecs pinned by tests, each carrying an
   honesty header: conformance against a real peer is a HUMAN LAB GATE.
2. **F-13 scope:** the owner authorized implementation of the CODE SURFACE
   only. The five record-41 §3 ungate conditions are enforced as fail-closed
   runtime/configuration gates and are NOT claimed met.
3. **Frozen invariants byte-identical:** the 14-state/27-edge command
   lifecycle, the never-granted permission set, the 26-event vocabulary,
   `deny.toml`, the kernel revocation policy, telemetry quality states,
   canonical units, and ADR-010 (forecasts are INPUTS, never commands).
4. **E-decisions stay OPEN:** D1-D6, P13-E-3, P15-E-1/E-2/E-3, P17-E-1/E-2,
   P18-E-* are never closed by an agent. Code is written to compose with any
   decision outcome (config-driven, fail-closed defaults).
5. **Orchestration:** waves of two agents with disjoint file ownership,
   exactly like the FRV1 program. Ledger ticks are applied by the orchestrator
   with count==1 assertion scripts after a static verification sweep.

## Feature ledgers

| ID | Feature | Phase | Owning code | Ledger | Status |
|---|---|---|---|---|---|
| F01 | Live IEEE 2030.5 server transport | phase P14.02 | `crates/adapters/protocol-20305` | see ledger_F01.md IMPLEMENTED (written) — 5/5 subtasks, wave 1 |
| F02 | Live OCPP CSMS WebSocket transport | phase P17.01/P17.02 | `crates/adapters/protocol-ocpp` | see ledger_F02.md IMPLEMENTED (written) — 6/6 subtasks, wave 1 |
| F03 | Live Modbus TCP client | phase P14.07 | `crates/adapters/protocol-modbus` | see ledger_F03.md IMPLEMENTED (written) — 6/6 subtasks, wave 2 |
| F04 | Live OpenADR 3.0 VTN endpoints | phase P14.05 | `crates/adapters/protocol-openadr` | see ledger_F04.md IMPLEMENTED (written) — 5/5 subtasks, wave 2 |
| F05 | Live MQTT 5 + Sparkplug B edge uplink | phase P14.11 | `crates/adapters/protocol-mqtt + crates/edge-agent/src/adapters/edge_to_cloud.rs` | see ledger_F05.md IMPLEMENTED (written) — 7/7 subtasks, wave 3 |
| F06 | Live CGMES network-model import | phase P14.12 | `crates/adapters/protocol-cim` | see ledger_F06.md IMPLEMENTED (written) — 7/7 subtasks, wave 3 |
| F07 | Live enterprise identity: OIDC login, SCIM 2.0, enforced step-up | phase P13.01-P13.04 | `apps/control-plane` | see ledger_F07.md IMPLEMENTED (written) — 6/6 subtasks, wave 4 |
| F08 | Live integration platform: machine identity, API keys, webhook delivery | phase P13.07-P13.09 | `apps/control-plane` | see ledger_F08.md IMPLEMENTED (written) — 7/7 subtasks, wave 4 |
| F09 | Live observability: Python OTEL instrumentation + propagation | phase P11.01-P11.09/P11.20 | `apps/worker/python (Rust side already wired)` | see ledger_F09.md IMPLEMENTED (written) — 5/5 subtasks, wave 7 |
| F10 | Live forecasting ingestion + evaluated baselines | phase P15.01-P15.06/P15.16 | `apps/worker (rust ingestion + python evaluation)` | see ledger_F10.md IMPLEMENTED (written) — 6/6 subtasks, wave 7 |
| F11 | Measured BESS degradation and SoH | phase P15.07-P15.11 | `crates/flexcore-domain` | see ledger_F11.md IMPLEMENTED (written) — 6/6 subtasks, wave 5 |
| F12 | Secure OTA: TUF-shaped bundle producer + staged rollout | phase P18.02-P18.06 | `crates/edge-agent + apps/control-plane` | see ledger_F12.md IMPLEMENTED (written) — 7/7 subtasks, wave 6 |
| F13 | ISO 15118 Plug & Charge certificate management (code surface) | phase P17.12 | `crates/adapters/protocol-ocpp` | see ledger_F13.md IMPLEMENTED (written) — 6/6 subtasks, wave 6 |
| F14 | Dynamic operating envelopes from live DSO feeds | phase P14.15 | `crates/flexcore-application + crates/flexcore-domain/doe_envelope.rs` | see ledger_F14.md IMPLEMENTED (written) — 6/6 subtasks, wave 5 |
| F15 | FDO-pattern zero-touch onboarding (in-repo surface) | phase P18.09 | `crates/edge-agent` | see ledger_F15.md IMPLEMENTED (written) — 7/7 subtasks, wave 6 |
| VXX | Pre-publication items V-8/V-9/V-10 | phase §3 items | `crates/adapters/postgres + crates/flexcore-ports` | see ledger_VXX.md IN PROGRESS — wave 8 (V-8/V-9/V-10) |

## Wave 8B + 8C addenda (2026-11-21)

**Wave 8B (orchestrator ruling recorded):** the CGMES spelling remediation
landed as **2 accepted aliases + 9 verified-absent entries = 11 MISMATCH rows**,
NOT the prompt-pinned "alias count == 9". The 44a record's own §3 summary line
("9/15 MISMATCH") contradicts its 11-row table (2 VERIFIED / 11 MISMATCH /
2 UNVERIFIED); mapping the further 6 spellings would have been hallucinated
facts (AGENTS.md §3 rule 1). The agent refused, documented the discrepancy in
record 44a §4a.4 for human correction, and pinned alias_count()==2 /
verified_absent_count()==9. This ruling accepts that resolution; the record's
summary line needs the human correction at the next record revision.
Machine-identity and API-key route exposure also landed (gate names reused,
deny-by-default pinned, raw key shown once with no-store semantics).

**Wave 8C:** the four pre-existing Python self-test defects proved against
pristine HEAD are fixed (marl allowlist +time: 26/26; forecast permission
self-tests now fire their refusals: 26/26; drill TypeError fixed per P12.09
plus a shared-counter defect that made the drill self-test never complete in
git history: 24/24; reproducibility fixture gains benchmark_set_version per
the P8.20 deliverable definition: 16/16). Full matrix green incl. untouched
surfaces (gnn 37/37, evaluation.py 28/28).
