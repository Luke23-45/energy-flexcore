# Phase 6 — Edge and Protocol Integration: Context

| Field | Value |
|---|---|
| Phase | 6 — Stage 6 Edge and protocol integration |
| Report trace | §23 Stage 6; §18; §2.3; §17; §3.3; §2.1 (v0.4.0) |
| Release | v0.4.0 — Site Integration |
| Control status | Read-only / test-lab by default |

## 1. Background

FlexCore is a supervisory orchestration layer, not a hard-real-time
controller and not a SCADA/ADMS replacement (§0.3, §1.6, ADR-001). Through
Phases 1–5 it exists entirely in simulation: synthetic assets, a
deterministic scheduler, an AC power-flow feasibility gate, a protected
command lifecycle, and an operator console that can replay a scenario.

Phase 6 is the first phase that touches a real site system — and it does so
read-only. The product shape for this phase is §2.1 v0.4.0: "First site
adapter through OpenEMS; selected telemetry/protocol adapters; outbound edge
connection", with control status "Read-only or test-lab control by default".

The central architectural fact is the deployment topology in §3.3: the
central platform lives in a cloud or customer private cloud; the site lives
behind an OT DMZ; the FlexCore Edge Agent sits in that DMZ and talks to the
existing site EMS (OpenEMS in the priority integration), which in turn talks
to device gateways and physical assets. The recommended connection is
initiated **outward from the customer edge to the central platform**; inbound
internet access into OT networks is to be avoided. The edge agent is a
separate Rust daemon (§18.1), not a library inside the control plane, because
it crosses the OT boundary and needs an independent lifecycle (§8.3).

The phase's hard lesson, stated three ways in the report, is that
**implementing a protocol is not conforming to it and not being certified
for it** (§2.3; §17.5; §9.3). Each production adapter must pass the relevant
conformance or interoperability suite before it is described as certified or
qualified — and certification itself belongs to the Alliance's program, not
to FlexCore.

## 2. Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §2.1, §2.3 | v0.4.0 scope; protocol table and posture (read first; OpenEMS priority; OCPP 2.0.1/2.1; IEEE 2030.5/CSIP later; IEC 61850 later). |
| §3.3, §3.4, §3.5 | Deployment topology (outbound-only); primary workflow; time scales (supervisory minutes, not sub-second). |
| §5.3, §5.8 | Plugin categories (site integration, protocol adapter); `site-edge` profile. |
| §8.1–§8.3 | Target tree (`apps/edge-agent`, `crates/adapters/*`); dependency rules; service-extraction policy. |
| §9.1–§9.3 | Core interfaces; result requirements; compatibility and supply chain (SBOM, signatures, no self-upgrade). |
| §10.2–§10.4 | Canonical units/signs; time model; telemetry quality (six states). |
| §14.1–§14.5 | Command lifecycle, invariants, per-asset fallback, dispatch is not hard real-time. |
| §15.3–§15.5 | Event vocabulary; NATS at-least-once + idempotent consumers; transactional outbox. |
| §16.2 | Multi-tenancy, RLS defence in depth, and the warning that table owners bypass RLS. |
| §17.1–§17.5 | Security model (NIST SP 800-82 Rev. 3 final; Rev. 4 draft); zones; required controls; threat model; safety invariants. |
| §18.1–§18.3 | Edge responsibilities, trust boundary, deployment profile — the core of this phase. |
| §19.2, §19.4 | Test layers (esp. fault injection, security); evaluation metrics without invented targets. |
| §20.1–§20.2 | Console workspaces and display principles (freshness, proposed vs executed). |
| §21.2–§21.4 | Availability/recovery; observability; build form and control-authority modes. |
| §22.2–§22.3 | CI gates; release gates (incl. edge offline/degraded modes tested). |
| §23 Stage 6 | Deliverables and exit gate (quoted in CHECKLIST.md). |
| §24 | Vertical slice — mostly not in this phase (see PLAN.md §10). |
| §25.2 | Deferred decisions D1–D6; D5 and D6 gate this phase. |
| §27 [R26], [R29], [R19], [R27] | NIST SP 800-82; OpenADR 3.0; OCPP; IEEE 2030.5 references. |

## 3. Canonical units and sign conventions (§10.2)

- Active power **MW**; reactive power **MVAr**; apparent power **MVA**;
  energy **MWh**. Persisted values and API contracts use these units; the UI
  may display kW/kWh.
- **`P > 0` means export/injection into the grid** at the point of
  connection; `P < 0` means import/consumption.
- Reactive-power sign has **no universal constant**. Each grid model and
  device profile declares whether `Q > 0` means injection (generator
  convention) or consumption (load convention); each adapter records the
  mapping it applies.
- Voltage: kV at the nominal base in the network model, per-unit in the
  solver representation, base declared explicitly. Current: A or per-unit on
  an explicit base, with phase identification. Frequency: Hz.
- **SOC is a fraction in [0,1]** at the API boundary; percentages exist only
  as display or vendor-source values and are converted at the adapter.
- Adapters must convert vendor/protocol units into canonical units and
  record both the unit and the sign mapping profile. Do not rely on implicit
  unit or sign assumptions.

## 4. Telemetry quality (§10.4)

Every telemetry value carries **exactly one** canonical quality state:

| State | Meaning |
|---|---|
| `GOOD` | Valid, fresh, within range, from a trusted source |
| `SUSPECT` | Received and parsed, but a plausibility, range, or consistency check failed |
| `BAD` | Source reports the value invalid, or a hard validation rule failed |
| `MISSING` | No value received for the expected interval |
| `ESTIMATED` | Derived, interpolated, substituted, or model-generated rather than measured |
| `STALE` | Last known value whose age exceeds the use-case freshness limit |

Richer source states (`SUBSTITUTED`, `OUT_OF_RANGE`, `UNSYNCHRONIZED`,
`COMMUNICATION_LOSS`, `UNRELIABLE`, …) must be mapped to one of these six
at the adapter boundary, with the original source code retained as
diagnostic metadata. A quality flag must not be silently converted into a
normal reading. Every use case defines which quality states are acceptable.

## 5. Terminology

| Term | Meaning in this phase |
|---|---|
| Edge agent | FlexCore's Rust daemon in the OT DMZ / at site; outbound mTLS to central; manages local adapters; buffers telemetry; verifies commands (test-env only). Not a BMS/PLC/relay. |
| Site EMS | The existing site energy management system — OpenEMS in the priority integration. Owns local control algorithms and device communication; FlexCore reads from it and (only in the approved test env) forwards verified commands to it. |
| OpenEMS Edge / Backend | OpenEMS's own components: Edge runs on site (Java/OSGi, input–process–output cycle, Controllers ordered by a Scheduler, Natures/Channels/EdgeConfig JSON); Backend (incl. Backend.Edge.App) receives Edge connections. FlexCore's edge agent is a separate program that talks to OpenEMS Edge; it is not OpenEMS. |
| Protocol adapter | Code translating one external protocol into FlexCore's canonical contracts (and, in the test env, the reverse for approved commands). One adapter per D5 in this phase. |
| Anti-corruption layer | The mapping at the adapter boundary that converts external terms, units, timestamps, and status codes into canonical contracts (§6.4). |
| Read-only / shadow mode | Control-authority mode (§21.4) in which FlexCore computes and compares but writes nothing to devices. The v0.4.0 default. Distinct from "passive mode" (do not use that term). |
| Operator-approved control | Control authority where each command requires an authorized human approval (§21.4). The v1.0.0 default — not this phase. |
| Conformance suite | A test suite checking an implementation against a protocol. Passing FlexCore's suite is not Alliance/OCA/SunSpec certification. |
| VTN / VEN | Virtual Top Node / Virtual End Node in OpenADR terminology (§26). |
| CSIP | Common Smart Inverter Profile — the regional profile based on IEEE 2030.5 (§26). |
| OT / DMZ | Operational Technology; the demilitarized zone between enterprise IT and OT (§17.2). |
| mTLS | Mutual TLS: both sides present certificates. FlexCore's edge↔central channel requirement (§17.3, §3.3). Not the same as OpenEMS's native API-key WebSocket authentication. |
| SUPERSEDED / CANCELLED | Distinct §14.1 states; do not merge them. |

Use "license" (US spelling) and "acknowledgement" (per `.agent/WRITING.md`).
Use "operator-approved control", never "manual control" or
"human-in-the-loop", for the §21.4 mode. Use "dispatch command", not
"setpoint", for the lifecycle object.

## 6. Constraints

- **Simulation-only 0.x; read-only default in v0.4.0.** Field write authority
  begins at v1.0.0 with operator approval (§2.1, §21.4, AGENTS.md rule 5).
  The command path in this phase is disabled by default and enabled only
  in a test environment under the written P6.14 gate.
- **No safety/certification/protection claims.** FlexCore is not certified,
  not a safety case, not permission for live grid control; it does not
  perform primary frequency response, synthetic inertia, relay protection,
  inverter inner-loop control, or sub-cycle control (AGENTS.md rules 2–3;
  §0.3; §14.5; §17.5).
- **Solver-class discipline carries over:** HiGHS is LP/QP/MIP, not SOCP;
  Clarabel is convex conic, not MISOCP; AC power flow is not AC-OPF; IPOPT
  is local, not global (AGENTS.md rule 4). Nothing in this phase changes
  solver selection.
- **No hallucinated facts** (AGENTS.md rule 1): protocol versions, standard
  clauses, and library capabilities must trace to a primary source or be
  labelled ASSUMPTION. See RESEARCH.md.
- **Minimal, surgical edits; verify before claiming done** (AGENTS.md
  rules 6–7). Do not edit `docs/foundations/*` or the report in this phase.
- **No arbitrary Rust dylib loading** (§5.4, ADR-006). Not relevant to the
  edge agent's own code, and no plugin loader is built here.

## 7. Prior-phase outputs consumed

See `ledger_phase_6.md` header (Phases 1–5 per §23 Stages 1–5). In short:
workspace + API + Postgres + NATS (Phase 1); registry + telemetry model +
quality/stale detection (Phase 2); capability models + envelopes +
deterministic scheduler (Phase 3); grid provider + AC power-flow validation
+ command lifecycle + audit + signed commands + simulation-only dispatch
adapter (Phase 4); console + simulator adapters + replay + failure
injection (Phase 5). P6.04 verifies these before Phase 6 implementation
starts. Prior-phase ledgers exist in this repo under `ledger/phase-0*/`
through `ledger/phase-05-*/` (including a Phase 5 handover to Stage 6 at
P5.30); where a prior ledger's exit-gate evidence contradicts an assumption
in this ledger, P6.04 must be re-run against the ledger, not only the report.

## 8. D1–D6 relevance

| Gate | Status at ledger writing | Relevance to Phase 6 |
|---|---|---|
| D1 Target jurisdiction/market | OPEN (Phase 0) | Bounds which market/program adapter could ever be selected; does not block the read-only edge work, but the pilot site's jurisdiction constrains D6. |
| D2 First portfolio/service | OPEN (Phase 0) | Determines the asset mix the pilot serves; P6.01 must be consistent with whatever D2 record exists. |
| D3 Network class/model fidelity | OPEN (Phase 0) | Default 0.x target is balanced radial feeders; unbalanced three-phase enters from v0.4.0; meshed transmission out of scope. |
| D4 License/dependency audit | OPEN (before v0.1.0) | P6.02 audits the new Phase 6 dependencies explicitly; a GPL/copyleft finding is a decision point, not a footnote. |
| **D5 First protocol/asset mix** | **OPEN (before v0.4.0)** | **The phase's entry gate (P6.01). No adapter code before it closes.** |
| **D6 Cyber/OT boundary review** | **OPEN (before any read-only pilot)** | **The pilot gate (P6.03). Site-specific, signed; no pilot-site connection before it closes.** |

## 9. Open questions

1. Which pilot partner and site? (D5/D6 — P6.01/P6.03.)
2. Which single protocol adapter? (D5 — P6.01.) Candidates and their
   current state are in RESEARCH.md; the choice also fixes the conformance
   suite whose access terms must be confirmed (P6.19).
3. What is the target gateway hardware profile (CPU/RAM/disk, x86-64 and
   ARM64) for P6.20/P6.22? The report requires both architectures (§21.4)
   but names no specific hardware.
4. What is the bounded buffer capacity for P6.09 (records/bytes and the
   overflow policy)? A configuration decision to be recorded, not assumed.
5. What are the freshness limits that turn a value `STALE` for each use
   case (§10.4)? Defined per use case; the pilot's limits must be recorded
   in P6.08.
6. Does the pilot site require a DMZ, unidirectional gateway, jump host, or
   customer-controlled broker (§3.3)? Site-specific; answered by D6.
7. Is unbalanced three-phase modelling needed by this pilot earlier than
   the §25.2 default? If yes, it changes the grid-provider scope and must
   be recorded in P6.30 as a scope change, not absorbed silently.
