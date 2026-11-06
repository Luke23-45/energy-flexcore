# Phase 6 — Edge and Protocol Integration: Plan

| Field | Value |
|---|---|
| Phase | 6 — Stage 6 Edge and protocol integration |
| Report trace | §23 Stage 6; §18; §2.3; §17; §3.3; §2.1 (v0.4.0) |
| Release | v0.4.0 — Site Integration |
| Control status | Read-only / test-lab by default. No field write authority. |

## 1. Scope

### In scope

- A Rust edge agent (`apps/edge-agent`) as a separate daemon/service for
  Linux x86-64/ARM64 industrial gateways or customer-managed servers (§18.1,
  §18.3, §21.4).
- Outbound mTLS enrollment and channel: edge-initiated, mutually
  authenticated, no inbound internet access into OT (§3.3, §18.1).
- Selected OpenEMS integration: EdgeConfig discovery, channel mapping, and a
  read-only data path end to end in the lab (§2.3, §18.1).
- Exactly one protocol adapter selected with a real pilot partner, per D5
  (§2.3, §25.2): read/telemetry first; write operations only in an explicit
  test profile and only behind the P6.14 enablement gate.
- Telemetry buffering and recovery: bounded, encrypted local queue,
  store-and-forward, gap reporting (§18.1, §18.3, §21.2).
- Adapter conformance tests and the written claim boundary: implementing an
  adapter is not certification (§2.3, §9.3).
- Edge deployment profile, signed updates with rollback, local audit buffer,
  local policy enforcement, adapter health monitoring, multi-tenant
  isolation, security testing, and operator-console surfacing (§18.3,
  §20.1, §21.2–§21.4).
- Handover to Phase 7: the capability seams the edge/adapters implement,
  and the `site-edge` composition profile (§5.8).

### Non-goals (explicitly out of this phase)

- Field control authority. v0.4.0 is read-only or test-lab control by
  default (§2.1); operator-approved control is the v1.0.0 default (§21.4)
  and requires Stage 9 qualification.
- A second, third, or "complete set" of protocol adapters. One adapter,
  selected with a real pilot partner (D5). IEEE 2030.5/CSIP, IEC 61850, and
  ISO/RTO market adapters are later (§2.3).
- Certified protocol conformance. Conformance/certification belongs to the
  Alliance's own program and test houses; FlexCore's suite is a
  FlexCore-side conformance suite, not a certificate (§2.3).
- Hard-real-time, protection-grade, primary-frequency, synthetic-inertia,
  relay, or inverter inner-loop control (§0.3, §14.5, §17.5). The edge agent
  is not a BMS, PLC, relay, or hard-real-time controller (§18.1).
- Unbalanced three-phase modelling, unless the pilot requires it earlier
  than the §25.2 default (it enters the track from v0.4.0; the default
  0.x target is balanced radial distribution feeders).
- Any plugin SDK work. That is Phase 7. The edge agent must not grow a
  plugin loader in this phase.
- Production pilot deployment. That is Stage 9, after D6 and site
  commissioning.

## 2. Architecture decisions in play

| Decision | Report | Position in this phase |
|---|---|---|
| ADR-002 — Rust modular monolith, edge separate | §22.1, §8.3 | Edge agent is a separate deployment/security boundary; control plane stays a modular monolith. |
| ADR-003 — Site edge is a separate boundary | §22.1 | Edge crosses the OT boundary; independent lifecycle, own identity, own update channel. |
| ADR-005 — Protected safety kernel non-replaceable | §22.1, §5.1 | Command authorization, tenant isolation, audit, lifecycle, final dispatch validation stay in protected core; the edge enforces them, never replaces them. |
| ADR-007 — PostgreSQL source of truth; NATS JetStream transport | §22.1 | Edge does not own authoritative state; it forwards telemetry and buffers locally only as a queue, never as a second source of truth. |
| ADR-008 — Protobuf/gRPC internal; REST/OpenAPI external | §22.1, §15.1 | Edge-to-central messages use the internal contract (gRPC/Protobuf where appropriate); OpenEMS's own JSON-RPC/WebSocket is an adapter-internal detail, not a FlexCore public contract. |
| ADR-009 — Deterministic optimizer + independent feasibility validation is the baseline | §22.1 | Nothing in this phase changes the optimization baseline; the edge carries no optimizer. |
| ADR-012 — No hard-real-time or protection claims | §22.1 | Edge timing is supervisory (seconds to minutes, §3.5); sub-second/protection behaviour stays in local controllers. |
| D5 — First protocol and asset mix | §25.2 | **Open gate.** Must close (or be recorded OPEN with owner/venue/date) before adapter implementation (P6.01). |
| D6 — Cyber and OT boundary review | §25.2 | **Open gate.** Site-specific review required before any read-only pilot (P6.03). |
| D4 — License and dependency audit | §25.2 | Audit the Phase 6 stack explicitly (P6.02); do not assume the v0.1.0 audit covers new protocol/test dependencies. |

## 3. Component breakdown

| Component | Location (per §8.1) | Owns | Does not own |
|---|---|---|---|
| Edge agent binary | `apps/edge-agent/` | Process lifecycle, config, composition root | Domain invariants; authoritative state |
| Enrollment + identity | Edge agent / control plane | Key provisioning, certificate lifecycle, tenant/site binding | Customer IdP internals (§4) |
| mTLS channel | Edge agent / control plane edge service | Outbound connection, mutual auth, reconnect/backoff | OpenEMS's native WebSocket/API-key auth (adapter-internal) |
| Adapter manager | Edge agent | Adapter lifecycle, allowlist, health | Adapter protocol parsing (inside each adapter) |
| OpenEMS adapter | `crates/adapters/openems/` | EdgeConfig parsing, channel mapping, read path | OpenEMS Edge itself; its Controllers/Scheduler |
| Selected protocol adapter | `crates/adapters/<protocol>/` | Protocol decode, anti-corruption mapping, read path | Portfolio economic policy; dispatch authority |
| Telemetry buffer | Edge agent | Bounded encrypted queue, store-and-forward, gap events | Long-term telemetry storage (PostgreSQL, central) |
| Command receiver (disabled by default) | Edge agent | Signature/expiry/sequence verification, allowlist forwarding, rejection audit | Authorization decision (protected core, §9.1 `CommandAuthorizer`) |
| Local audit buffer | Edge agent | Append-oriented offline audit, ordered upload | Central audit store (append-oriented, §6.5) |
| Local policy + health | Edge agent | Rate limits, operating windows, adapter health | Operator approval policy (central) |
| Edge deployment | `deploy/edge/` | Package/OCI image, systemd service, signed updates | Control-plane deployment (Helm, v1.0.0) |

Dependency direction (§6.2, §8.2): domain depends on nothing
infrastructure; adapters depend on ports; `apps/edge-agent` wires them.
The edge agent must not become a second domain layer.

## 4. Data contracts

| Contract | Form | Notes |
|---|---|---|
| Telemetry (edge → central) | Canonical telemetry record (§10.1 `AssetState`, §10.3–§10.4) | MW/MVAr/MVA/MWh; `P > 0` export; SOC [0,1]; `event_time` + `ingest_time` + source sequence; exactly one quality state; original source code retained as diagnostic metadata. |
| EdgeConfig mapping | Versioned config file | OpenEMS Nature/Component/Channel → FlexCore asset/telemetry points; unmapped and ambiguous channels registered, never guessed. |
| Adapter mapping tables | Versioned tables in adapter docs/code | Protocol status codes → canonical quality; protocol units/signs → canonical; protocol timestamps → UTC `event_time`. |
| Enrollment identity | Certificate + server-side binding | One edge identity ↔ one tenant + approved site/asset set (§16.2). |
| Command (test-env only) | Signed command per §14.3 | Unique ID, tenant/portfolio/site/asset, source + version, schedule ref, target P/Q with units/sign, validity window, sequence, idempotency key, grid-model/telemetry snapshot refs, validation + authorization refs, signature, supersedes ref. |
| Events | NATS JetStream subjects (§15.3–§15.4) | `TelemetryReceived`, `TelemetryQualityChanged`, `DispatchIssued`, `DispatchAcknowledged`, `DispatchFailed`, etc.; at-least-once delivery, idempotent consumers, transactional outbox. |
| Audit records | Append-oriented (§6.5) | Enrollment, enable/disable of command path, rejections, health transitions, updates — with actor, object, decision, evidence, timestamp, correlation ID. |

No new public REST resources are introduced in this phase beyond what
Phases 1–5 defined (§15.2); edge enrollment/health endpoints, if needed,
are specified in P6.06/P6.17 and versioned under `/api/v1`.

## 5. Interfaces

- **Edge ↔ central:** outbound mTLS channel; edge authenticates each
  central session and validates every command; central does not assume a
  command was applied until acknowledgement + telemetry evidence (§18.2).
- **Edge ↔ OpenEMS Edge:** OpenEMS's own interfaces (WebSocket/JSON-RPC,
  EdgeConfig JSON, channel reads). FlexCore reads channels; it does not
  inject into the OpenEMS input–process–output cycle or its Controller
  ordering.
- **Edge ↔ protocol device/gateway:** the selected adapter's protocol, read
  first; writes only in the explicit test profile behind the P6.14 gate.
- **Edge ↔ operator console:** health, connectivity, certificate expiry,
  buffer depth, adapter status, command-path state (§20.1, §21.3).
- **Adapter ↔ domain:** only through ports (`TelemetrySource`,
  `AssetAdapter`, §9.1). No adapter type leaks into the domain (§6.2).

## 6. Sequencing

1. **Gates first (P6.01–P6.04):** D5, D4 follow-through, D6, readiness
   check. No adapter code before D5 closes.
2. **Edge skeleton + identity + channel (P6.05–P6.07):** crate, enrollment,
   outbound mTLS.
3. **Read path (P6.08–P6.11):** telemetry ingestion, buffering, OpenEMS
   discovery and read-only end-to-end run.
4. **Selected adapter (P6.12–P6.13, P6.19):** interface, read
   implementation, conformance suite.
5. **Command path, disabled by default (P6.14–P6.16):** gate, verification,
   forwarding/fallback — test-env only.
6. **Operations hardening (P6.17–P6.23):** policy/health, audit buffer,
   deployment/updates, isolation, performance, security testing.
7. **Exit-gate run, console, docs, CI, drill, handover, retrospective
   (P6.24–P6.30).**

Relative effort (no calendar dates): the read path and OpenEMS integration
are the bulk of the phase; the command path is deliberately small because
it is disabled by default and test-env only; the two open gates (D5/D6) are
schedule risks owned outside engineering and must not be worked around by
starting adapter code early.

## 7. Roles

| Role | Owns |
|---|---|
| Product + integration | D5, pilot partner, adapter selection, conformance claim boundary |
| Security | D6, enrollment, mTLS, command-path gate, security testing, isolation |
| Legal + engineering | D4 license audit, SBOM |
| Rust engineer | Edge agent, channel, buffer, deployment, CI |
| Integration engineer | OpenEMS + selected adapter, mapping tables, conformance suite |
| Power-systems reviewer | Fallback policy review, feasibility scope, mapping sanity |
| Frontend engineer | Console surfacing (§20.1) |
| DevOps | Packaging, signed updates, CI gates |
| Engineering lead | Readiness check, exit-gate run, retrospective |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| D5 never closes; adapter built for no pilot | P6.01 blocks adapter work; record OPEN with owner/venue/date rather than assuming. |
| D6 treated as generic architecture review | P6.03 requires a site-specific, signed review; generic text does not close D6. |
| OpenEMS monthly releases move channels/IDs | Pin the OpenEMS version in the lab and in docs; mapping is versioned config (P6.10). |
| OpenEMS native auth (API-key WebSocket) confused with FlexCore mTLS | RESEARCH.md note R6-C1; P6.07 tests reject downgrade. |
| "Read-only" integration quietly writes | P6.11 no-write attestation by network capture; P6.14 default-off command path. |
| Quality flags dropped at the adapter boundary | P6.08/P6.13 mapping tables + negative tests; `SUSPECT`/`BAD` never become `GOOD`. |
| Buffer overflow silently fabricates continuity | P6.09 explicit overflow policy + gap events; P6.28 drill. |
| Conformance suite behind membership/paywall | P6.02/P6.19 identify access terms before writing tests; claim boundary written regardless. |
| Command path enabled by a config example or fixture | P6.14 default-off in code, config, and profiles; enablement is signed, scoped, and expiring. |
| Edge becomes a second domain layer / plugin loader | §8.2 enforcement in P6.05; no plugin loading in this phase (Phase 7). |

## 9. What must NOT be built yet

- Field command authority (v1.0.0 / Stage 9).
- Additional protocol adapters beyond the D5-selected one.
- IEEE 2030.5/CSIP, IEC 61850, ISO/RTO market adapters (§2.3, later).
- A plugin loader, WASM host, or gRPC worker template (Phase 7).
- GNN/DMPC/MARL integration at the edge (Phase 8, research only).
- Certified-conformance claims of any kind.
- Hard-real-time or protection behaviour of any kind.

## 10. How the §24 vertical slice lands here

**Mostly: not in this phase.** The §24 slice (tenant → portfolio → site →
grid model → four simulated assets → telemetry → flexibility → LP/MILP/QP
schedule → feeder validation → PROPOSED command → validation/rejection/
approval → simulator-only dispatch → acknowledgement/tracking/partial
failure/timeout → console audit trail → deterministic replay) lands across
v0.1.0–v0.3.0, i.e. Phases 1–5. Phase 6 does not re-build the slice and does
not extend it to field devices.

What Phase 6 adds to the slice, and only this:

- Step 2's telemetry source can additionally be a real lab site (OpenEMS
  Edge / selected adapter) instead of only the simulator — read-only.
- Step 7's dispatch target remains the simulator adapter by default; the
  edge command receiver exists but is disabled, and is exercised only in
  the explicitly approved test environment (P6.14–P6.16, P6.24).
- Step 9's console gains edge/adapter health and command-path state
  (P6.25).
- Step 10's replay must now also reproduce the edge read path from its
  recorded EdgeConfig, mapping version, and buffer/gap events.

If the slice regresses because of Phase 6 work, the phase has failed its
readiness obligation (P6.04) regardless of what the edge agent can do.
