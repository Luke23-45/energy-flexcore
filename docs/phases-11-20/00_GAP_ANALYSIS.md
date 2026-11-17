# Gap Analysis — What FlexCore Is Missing for Enterprise Credibility

Date: 2026-11-14. This is the review deliverable behind Phases 11–20. It
answers: *are we missing important features, and which?* Method: one internal
repository inventory plus six deep online research tracks (~180 distinct
searches; primary sources fetched where reachable; unverifiable items marked
UNVERIFIED). Synthesis below maps every gap to a phase and every phase to
evidence. Source IDs `[S-n]` resolve in §11.

**Standing caveat.** The Phase 0–10 codebase is WRITTEN-UNRUN: nothing has
been compiled or executed. Every "implemented" status below means written
code with records, not verified behavior. The first human build is the gate
for everything.

---

## §1 Method

| Track | What it produced |
|---|---|
| A. Internal inventory | 20-crate workspace map, capability matrix, keyword presence sweep (75 terms), limitation registers (LR-001..211 + proposed amendments, O-registers, KN-0, F-P10.07-5), 25-item absent-capability list |
| B. Standards research | IEEE 2030.5/CSIP, OpenADR 2.0b/3.0, IEC 61850-7-420, DNP3/IEEE 1815.2, IEC 60870-5-104, SunSpec Modbus, CIM/CGMES/IEC 61968-5, IEEE 1547-2018/1547.1-2020, UL 1741 SB, IEC 62443, NERC CIP, FERC 2222 status, ENTSO-E balancing stack, M&V/IPMVP/ANSI C12, EU RfG, UK LFM, MQTT/Sparkplug |
| C. Commercial landscape | 20+ products profiled (Tesla Autobidder, GE GridOS DERMS, EnergyHub, Uplight, Enode, Kraken, Voltus, CPower, Next Kraftwerke, Enbala, sonnen, Tibber, Generac, Sunrun, Siemens, ABB, Schneider, Opus One, Utilidata, GridBeyond, Camus, WeaveGrid; OSS: OpenEMS, VOLTTRON, GridLAB-D, HELICS, OpenDSS, ThingsBoard, Fledge, OpenFMB) → 10-layer feature taxonomy, minimum enterprise feature set |
| D. Engineering practices | SRE/SLO/burn-rate alerting, OpenTelemetry (Rust/Python SDK maturity), metrics stack, logging, event streaming (Kafka vs JetStream), time-series storage, Postgres HA/PITR/pooling/migrations, Kubernetes timing, secrets, zero-trust identity, supply chain (SLSA/sigstore/SBOM), SOC 2/ISO 27001 mechanics, chaos engineering, performance engineering, API platform practice, GDPR/data governance → 4-tier robustness baseline |
| E. Market mechanics | PJM/CAISO/ERCOT/ISO-NE/NYISO DER-aggregation registration, telemetry cadences (2s/6s/5-min), baselines (CBL 10-in-10 + SDA), settlement formats (PJM Manual 29 CSV/XML, CAISO 5-min SQMD, EBGL 15-min ISP), performance scoring (PJM precision redesign), market connectivity (ICCP/DNP3/eSchedules/DataMiner2/ENTSO-E TP), revenue-share/billing basics |
| F1. EV integration | OCPP 2.0.1/2.1 (91 messages, functional blocks, security profiles 1–3), ISO 15118-2/-20 PnC PKI, V2G landscape (CharIN, UL 1741 SC/J3072 path), smart-charging profile composition, dual-source SoC practice, AFIR/Eichrecht, OCPI/OICP (out-of-scope verdict) |
| F2. Asset tech | BESS degradation models (calendar+cycle, rainflow, $/MWh penalties), SoH estimation, warranty structures (Megapack/Fluence), BMS/PCS/EMS telemetry hierarchy, inverter settings ownership + CSIP primacy, DOEs (Project EDGE, SAPN Flexible Exports, CSIP-AUS), OTA security (TUF/SUIT/IEC 62443-4-2 CR 3.4/A-B slots), device identity (802.1AR, FDO), digital twins, health scoring, interconnection records (UL 1741 SB COC) |

## §2 What exists today (Track A summary)

Implemented (written, unrun) across 20 workspace members: canonical value
types and 6-state telemetry quality (`flexcore-contracts`); 14-state/27-edge
command lifecycle with three distinct gates (`flexcore-domain`);
hexagonal ports (`flexcore-ports`); registry/telemetry/envelope/scheduling/
command use cases (`flexcore-application`); frozen 26-event vocabulary with
mandatory idempotency keys (`flexcore-events`); deny-by-default authz
(`flexcore-authz`); solver-provider contract with HiGHS Tier-1 / Clarabel
Tier-2 and tier-guard (`flexcore-optimization-api`); fail-closed grid
feasibility gate (`flexcore-grid-api`); Postgres adapter with RLS and
migrations 0002–0018; NATS JetStream adapter with outbox/idempotent
consumers/DLQ; deterministic simulator; edge agent (offline buffer, mTLS
uplink, OpenEMS adapter, enrollment ceremony, fail-closed command path and
update seam); WASM plugin kernel (manifests, lifecycle, profiles,
revocation); Phase-8 research crate (ADMM/DMPC/proposals-only per ADR-010);
Axum control plane (GET-only dispatch routes, Prometheus-format `/metrics`,
SSE live); sim-runner replay/benchmark CLI; Python workers (grid via
pandapower, guarded GNN/MARL/ANDES, portfolio analytics, reproducibility,
drill, HELICS federates); console scaffold with 12 workspaces.

Recorded gaps and defects carried from 0–10: no market-records store
(LR-074/LR-190); no IdP (LR-047); no step-up/MFA enforcement (LR-048);
test-grade FNV-1a digests (LR-203); backup never restored (LR-204);
RTO/RPO unsettable (LR-205); no update-bundle producer; F-P10.07-5 clock
median defect; no console lockfile; KN-0 LR-numbering discrepancy; all
evaluation cells EMPTY; v1.0.0 NOT released (0 tags).

## §3 Commercial minimum enterprise feature set vs FlexCore [Track C]

The commercial research found features no enterprise VPP platform sells
without. FlexCore status and the phase that closes each gap:

| # | Table-stakes capability | Evidence | FlexCore today | Phase |
|---|---|---|---|---|
| 1 | Multi-OEM, multi-asset-class connectivity + enrollment | Uplight "40+ OEMs, 10+ protocols" [S-43]; Enode "100+ brands, 1,000+ models" [S-45]; EnergyHub partner ecosystem [S-42] | OpenEMS REST adapter only (UNVERIFIED A1–A8); generic-rest reserved, never implemented | 14 |
| 2 | Near-real-time fleet telemetry + monitoring | Voltus 30-second interval data [S-47]; GridBeyond real-time telemetry [S-51]; GE Operations [S-41] | 7-stage pipeline written; simulator-only source; no dual-source fusion | 11, 14, 15 |
| 3 | Forecasting (load/generation; price for market-facing VPPs) | GE Forecasting model library [S-41]; Tesla 15-min price forecasts [S-50]; GridBeyond price+consumption forecasts [S-51] | Synthetic labeled fixture only (P3.31); no forecasting engine | 15 |
| 4 | Constraint-aware optimization/dispatch (envelope-aware) | GE "grid constraints, economics, and DOEs" [S-41]; Schneider grid-constraint management + DER dispatch, se.com EcoStruxure DERMS; Camus flexible operating limits [S-52] | Flexibility envelopes + rolling horizon + feasibility gate exist — a genuine strength | 15, 16 (degradation/forecast inputs) |
| 5 | Program/contract/enrollment lifecycle management | GE Program Manager [S-41]; Uplight turnkey program management [S-43]; CPower curtailment plans [S-48] | Nothing: no program entity, no enrollment workflow | 16 |
| 6 | Market or utility-program participation layer | Voltus nine wholesale markets [S-47]; Next Kraftwerke day-ahead/intraday/control reserve [S-49]; GE Markets Orchestration [S-41] | MarketSignalReceived event has no producer; market workspace shows gap panels; bidding excluded by report §0.3 scope and ADR-011 forbids a *generic* tariff adapter | 16 |
| 7 | M&V / baselining / settlement-grade metering | Uplight same-day M&V [S-43]; GridBeyond "metering, verification, and settlement needed to prove delivery" [S-51]; NJ BPU straw [S-60]; RMI metering framework [S-59] | "revenue grade"/"measurement and verification": zero hits; settlement quality rules are EMPTY placeholders (P9.11) | 16 |
| 8 | Operator console with control-system integration | EnergyHub single pane of glass [S-42]; GE Operations [S-41] | 12-workspace console scaffold, never built; no ADMS/SCADA write path | 13, 20 |
| 9 | Customer-facing experience + incentives | ecobee/Generac Grid Resiliency rewards and Tibber Grid Rewards [S-171][S-172]; Uplight engagement + incentive processing [S-43] | Out of product scope for now — recorded decision, not a phase (§8) | — |
| 10 | Security attestations (SOC 2 Type II minimum) | Enode SOC 2 [S-45]; Uplight year-after-year SOC 2 + pen tests [S-44] | Nothing: security consolidation verdict NOT APPROVED (P10.25) | 19 |
| 11 | Interoperability standards adherence (OpenADR / IEEE 2030.5) | GE Gateway "certified to IEEE 2030.5" [S-41]; OpenADR Alliance DER framing [S-10] | Zero protocol stacks; only OpenEMS REST + HELICS co-sim | 14 |
| 12 | Proven scale/deployment evidence | GE >90 deployments [S-41]; Next Pool 14,375 units/15,541 MW [S-49]; KrakenFlex 2 GW [S-46] | No deployment exists at all (Stage 9 chain unheld) | human gate, not a phase |

Additional commercial differentiators FlexCore lacks: multi-service
co-optimization (energy + ancillary + capacity) [S-50][S-41]; payment
lifecycle transparency to asset owners ("forecast to earned to paid",
Voltus [S-47]) → Phase 16; M&V as a productized module (almost nobody
markets it — an honest differentiator opportunity) → Phase 16.

## §4 Standards obligations vs FlexCore [Track B]

| Obligation | What support means | FlexCore today | Phase |
|---|---|---|---|
| IEEE 2030.5 server (CSIP) — CA Rule 21 / HI 14H default interface [S-1][S-3][S-5][S-6] | AggregatorServer resource tree (FunctionSetAssignments, DERProgram/DERControl, Response, Pricing, LogEvent), mTLS with CA-chain validation, cert lifecycle [S-7][S-8][S-9] | Absent | 14 |
| OpenADR VTN — 3.0 REST/JSON current [S-10]; 2.0b legacy XML [S-11] | Event/report/registration endpoints, polling + push, certification path | Absent | 14 |
| SunSpec Modbus client [S-16] | Information models for inverters/meters/batteries over Modbus TCP/RTU | Absent (site-level via OpenEMS only) | 14 |
| DNP3 / IEEE 1815.2-2025 DER profile [S-14][S-15] | Master/outstation roles, DER data objects, secure transport | Absent | 14 |
| IEC 60870-5-104 client (EU DSO telecontrol) [Track B; CESI via S-14 analogue] | 104 client + compensating security (no native encryption) | Absent | 14 |
| IEC 61850-7-420 MMS client [S-12][S-13] | DER data-model awareness, SCL/ICD ingestion at DSO boundary | Absent | 14 |
| CIM/CGMES + IEC 61968-5 DER groups [S-17][S-18] | Network-model import; DER group management payloads | Absent | 14 |
| IEEE 1547-2018 settings profiles [S-19][S-20] | Function catalog, jurisdiction default tables, read/write via 2030.5/Modbus; aggregator must respect utility override rights (Clause 10) | Absent | 14 |
| UL 1741 SB [S-21] | NOT claimable by a platform; must ingest/list certified equipment | Absent (no document store) | 15 |
| IEC 62443 posture [S-22] | 4-1 SDL process, 4-2 component hardening, zone/conduit architecture docs | Partial (secure SDLC practices exist informally) | 19 |
| NERC CIP readiness [S-23][S-24][S-25] | Applicability assessment, CIP-013-style supply-chain flow-down; CIP currently does not apply to DERAs (voluntary CIP-003 Att. 12 baseline) | Absent | 19 |
| FERC 2222 participation models [S-26][S-27] | Registration data, telemetry (e.g., NYISO 6-second [S-31][S-106]), settlement-ready interval data, dual-participation rules; timelines: CAISO live Nov 2024, ISO-NE Nov 2026, NYISO end-2026, PJM Feb 2027/28, MISO 2027/29, SPP 2030 | Absent | 16 |
| ENTSO-E balancing (FCR/aFRR/mFRR/RR; PICASSO/MARI) [S-28] | BSP prequalification evidence, bid submission, activation telemetry, performance scoring; German prequalification: FCR 30 s / aFRR 5 min full response [Track E] | Absent | 16 |
| M&V: IPMVP options A–D [S-32]; CBL 10-in-10 + ≤20% SDA [S-30]; ANSI C12.19/C12.20 revenue-grade data [S-31][S-33] | Baseline engines, interval data at settlement resolution, revenue-grade metering interfaces | Absent | 16 |
| EU RfG Type A–D evidence [S-34][S-35]; UK LFM (Piclo tenders, availability+utilisation, P483 dual participation) [S-36][S-37] | Registration/tender/utilisation integration | Absent | 16 |
| MQTT 5 + Sparkplug B edge transport [S-39][S-40] | Birth/death state management for DER fleets; OpenFMB validates NATS at grid edge [S-40] | Edge agent exists; Sparkplug absent | 14 |
| PKI operations (2030.5 CA hierarchy; V2G Root CA for 15118) [S-8][S-9] | Trust anchors, issuance, revocation, renewal | Absent (edge mTLS exists but no CA operations) | 14, 17, 18 |

## §5 Engineering robustness baseline vs FlexCore [Track D]

The engineering research produced a 4-tier baseline for exactly this stack
shape (Rust/Axum + Python workers + TS console + Postgres RLS + NATS
JetStream + Compose). FlexCore status per tier:

**Tier 0 — correctness bedrock (Phase 11)**
| Practice | FlexCore today |
|---|---|
| Golden signals + multi-window multi-burn-rate alerts on SLOs [S-61] | `/metrics` endpoint only; no SLOs, no rules ("slo"/"error budget": zero hits) |
| OpenTelemetry three signals; gateway Collector; Rust traces Beta/metrics+logs Stable [S-62][S-63][S-64] | OTel is a docs mention only; no SDK |
| Structured logs with tenant+command+trace correlation, SeverityNumber mapping [S-62] | JSON logs exist; no trace_id, no OTel model |
| Idempotency keys on command POSTs + consumer idempotency [S-61][S-65] | Idempotency exists in event envelope/outbox; the API path does not enforce it on command submission |
| JetStream hardening: MaxDeliver, DLQ streams, DLQ-depth alerts, schema-version headers [S-65] | DLQ exists per consumer; hardening incomplete |
| Per-tenant rate limiting + backpressure [S-85] | Plugin call-rate limits only; no API-edge limiter |
| Production-grade audit chain (hash chain, INSERT-only role) [S-88] | Append-only audit exists; chain digests are test-grade FNV-1a [LR-203] |
| Secrets out of .env; dynamic credentials [S-74][S-75] | Compose .env posture |
| RLS + pooling safety: transaction-scoped SET LOCAL [S-69][S-70] | RLS present; pooling interaction unaudited |
| Expand/contract migration policy for lifecycle tables [S-71] | Migrations 0002–0018 exist; no policy document |

**Tier 1 — survivability (Phase 12)**
| Practice | FlexCore today |
|---|---|
| Tested PITR (pgBackRest/WAL-G), timed restore drills [S-67][S-68] | "A backup configuration that has never been restored" [LR-204] |
| RTO/RPO registers per tier | Unsettable [LR-205] |
| Chaos experiments as scripts + game days [S-81][S-82] | "chaos": zero hits; fault injection exists only as simulator scenarios |
| Long-term metrics + dashboards as code [S-61] | Nothing |
| OTel adoption ( Collector on Compose) [S-64] | Nothing |
| Supply-chain baseline: cargo-deny/cargo-audit/pip-audit/SBOM/cosign; SLSA L1→L2 [S-78][S-79] | deny.toml exists (P6.27), never run; no SBOM |

**Tier 2 — enterprise readiness (Phases 13, 19, 20)**
| Practice | FlexCore today |
|---|---|
| Machine identity (OAuth2 client-credentials JWT) [S-76][S-77] | DenyAllResolver + non-default dev-auth only |
| Enterprise SSO (SAML/OIDC) + SCIM [S-77] | No IdP [LR-047]; "sso"/"saml"/"scim": zero hits |
| MFA/step-up enforcement | UI-only component [LR-048] |
| Outbound webhooks: HMAC signing, tolerance, retries, DLQ, secret rolling [S-83] | "webhook": zero hits |
| API versioning + Deprecation/Sunset policy [S-84] | /api/v1/ paths exist; no deprecation policy |
| Vault: dynamic Postgres creds, PKI short-lived certs [S-74] | .env posture |
| Chaos/DR drills as scheduled game days | Nothing |
| Compliance automation readiness [S-80] | Evidence chain unmapped |

**Tier 3 — scale-stage architecture (Phase 20; deliberately deferred)**
| Practice | Research verdict for this shape |
|---|---|
| Kubernetes + GitOps + progressive delivery [S-72][S-73] | Do not migrate now; Compose is correct until multi-host/DR/canary triggers |
| Postgres HA (Patroni, sync mode for lifecycle tables) + PgBouncer [S-67][S-69] | Design now, deploy on availability contracts |
| TimescaleDB for domain time-series [S-66] | Extend existing partitioning first; adopt on evidence |
| Kafka migration [S-65] | No: the 14-state lifecycle is state-machine-shaped; JetStream suffices |
| SPIFFE/SPIRE mesh [S-76] | Deferred to multi-host |

## §6 Market participation build-list [Track E] → Phase 16

What the ISO/regulator research says a platform must build (each tied to
market evidence in §11): multi-cadence meter/telemetry ingestion
(device-level 5-min/1-min for ERCOT ADER validation [S-100][S-101]; 2-second
ICCP for ADER [S-100]; 6-second SCADA + SOC for NYISO ESRs [S-105][S-106];
PJM telemetry ≥10 MW aggregations [S-90]); registration data management
(per-DER + per-aggregation registrations, EDC 45-day reviews, PUCT+ERCOT
dual registration, CAMS/eMarket asset lifecycle [S-90][S-100][S-103]); bid
composition from asset envelopes (single aggregation node in PJM; PDR
two-ID load-shift bidding in CAISO [S-96]; ISO-NE DAM Asset tab [S-103]);
telemetry export (ICCP/DNP3 pipelines; net-consumer normalization; weighted
ramp-rate aggregation [S-100]); settlement ingestion/reconciliation (PJM
Manual 29 CSV/XML + MSRS line items [S-93]; CAISO 5-minute SQMD settlements
[S-97]; EBGL Art. 52–55 15-minute ISP imbalance [S-107][S-108][S-109]);
baseline-driven DR settlement (10-in-10 + SDA + DLA [S-30][S-96]);
performance evidence capture (PJM precision scoring post-Oct-2025 single
consolidated score, RegUp/RegDn after Phase 2 [S-94][S-95]; German FCR
30 s / aFRR 5 min full-response evidence [Track E]; SOC traces); billing
basics for revenue share (pooled-revenue disaggregation, contract splits,
owner statements, reconciliation [S-118]); program enrollment and event
management (OpenADR event lifecycle, opt-in/out, report delivery for M&V
[S-116][S-117]).

## §7 EV integration build-list [Track F1] → Phase 17; asset intelligence [Track F2] → Phases 15/18

**EV (F1).** OCPP CSMS adapter (WebSocket OCPP-J framing, subprotocol
negotiation `ocpp2.0.1`/`ocpp2.1`; 2.0.1 baseline with 2.1 additive
compatibility [S-121][S-123][S-125]); security Profile 3 onboarding with
per-device certificates and security-event ingestion [S-126][S-127];
smart-charging schedule composition (dispatch targets →
ChargingStationExternalConstraints / TxDefaultProfile / TxProfile with
stack-level precedence, minChargingRate handling, W/A conversion,
GetCompositeSchedule reconciliation [S-124]); dual-source SoC arbitration
(OCPP MeterValues authoritative while plugged; OEM telematics stale-when-
sleeping [S-139][S-140]; Tesla 500 ms telemetry the outlier [S-139]);
plug-state as participation ground truth; departure-time constraints from
telematics [S-141][S-142]; user-override precedence over curtailment; depot
load management under site import limits [S-143][S-142]; ISO 15118 PnC
certificate management (V2G Root CA trust store, OCPP block L flows, OCSP)
phase-gated because PKI is the last commercial barrier [S-128][S-130][S-125];
V2G readiness hooks (OCPP 2.1 block Q/R; North-American AC path via
2030.5/V2G-AC/J3072/UL 1741 SC as a separate integration [S-4][S-131]);
AFIR/Eichrecht data hooks (MeterValuesSign; ad-hoc payment awareness)
[S-137][S-138]; OCPI/OICP explicitly out-of-scope for depot-fleet VPP
[S-134].

**Asset intelligence (F2).** BESS degradation data model (monotonic
throughput/FEC counters, rainflow DOD×mean-SOC histogram, temperature
distribution, high-SOC dwell, periodic SoH observations; chemistry-
parameterized semi-empirical models per NREL BLAST-Lite [S-146][S-147][S-148]);
degradation cost curve into the optimizer as $/MWh throughput penalty or
piecewise-linear aging cost [S-149][S-150]; warranty object (capacity-
retention schedules, cycle/throughput limits, RTE/availability commitments,
augmentation events; Megapack CMA and Fluence Dispatchable Energy Guarantee
as the reference structures [S-151][S-152][S-153][S-154]) + evidence-pack
generator; BMS/PCS/EMS telemetry hierarchy awareness (VPP consumes 1–10 s
aggregates + alarms, never the cell-scan path [S-155][S-156]); inverter
settings management (certified baseline vs aggregator schedules with CSIP
primacy resolution; utility override per 1547-2018 Clause 10 [S-156][S-20]);
DOE consumption interface (per-connection-point envelopes with expiry and
cadence handling; Project EDGE + SAPN Flexible Exports + CSIP-AUS as
reference deployments [S-157][S-158][S-159][S-160]); fleet health scoring
(deviation-from-normal models, alarm normalization, availability KPI
independent of O&M — Nispera pattern [S-53]); telemetry-drift detection;
digital-twin-lite (calibrated per-asset models + drift detection; not grid
state estimation) [S-168]; interconnection-record ingestion (UL 1741 SB COC,
IEEE 1547.1-2020 test reports, function category, settings profile [S-21][S-169][S-170]).

**Edge lifecycle (F2 → Phase 18).** The missing update-bundle producer
becomes a TUF-style metadata repository (root offline/targets/snapshot/
timestamp roles, N+1 rotation) whose device artifacts are SUIT-shaped signed
manifests (applicability, sequence-number anti-rollback) [S-162][S-163];
device flow: download → verify → install → status incl. failure states,
A/B slots with health-check auto-rollback and anti-rollback counters
[S-162]; staged cohort rollouts with abort thresholds; compliance
mapping to IEC 62443-4-1 SAR 3.4 / 4-2 CR 3.4 + CR 2.1 [S-163][S-164];
device identity lifecycle (IDevID/LDevID, FDO-pattern late binding,
decommissioning) [S-165][S-166]; eSIM state tracked as data only [S-167].

## §8 Explicitly out of scope (recorded, with revisit conditions)

| Item | Why deferred | Revisit when |
|---|---|---|
| Retail billing / CIS replacement (Kraken-class [S-46]) | Different product; revenue-share billing in Phase 16 is the VPP-relevant slice | If FlexCore becomes a retailer |
| OCPI/OICP roaming [S-134] | Depot-fleet VPP controls its own chargers via OCPP | If operating a public CPO network |
| Consumer app + incentives | B2B operator platform first; commercial table-stakes list marks customer experience for *consumer* VPPs | Product decision by owner |
| Sub-second distributed control / meter-chip edge AI (Enbala/Utilidata class) | FlexCore is supervisory; §0.3 non-promises exclude fast control | Never without re-baselining the safety case |
| Protection-grade anything, hard real-time | Foundations non-promises | Never |
| ERCOT/wholesale live bidding in v1.x | Market adapters land with paper-trading + jurisdiction register first (ADR-011 boundary: jurisdiction-specific, not generic) | Per E-decision on first market |
| Kubernetes migration by default | Single-host Compose shape; triggers recorded in Phase 20 [S-72] | Multi-host / DR / canary trigger fires |
| Kafka migration [S-65] | JetStream fits the lifecycle shape | Sustained >100k msg/s or external-consumer ecosystem need |

## §9 Phase architecture (10 phases, 196 subtasks)

Ordering logic: harden the existing core first (a dispatch platform's state
machine must be correct under failure before anything is added on top), then
survivability, then the enterprise surface, then connectivity, intelligence,
and revenue, with certification-readiness collecting the evidence and
deployment architecture last because its triggers are growth-dependent.

```text
P11 Operational Robustness & Observability ──────────────┐ (foundation for all)
P12 Backup/DR/Resilience (needs P11 SLOs as steady state) │
P13 Enterprise Identity & Integration (needs P11 audit)   │
P14 Protocol & Device Connectivity (needs P11 metrics)    │
P15 Forecasting & Asset Intelligence (needs P11)          │
P16 Market/Settlement/Revenue (needs P13 identity, P14    │
    exports, P15 forecasts)                               │
P17 EV Fleet & Charging (needs P14 adapter architecture,  │
    P15 departure/SoC intelligence)                       │
P18 Edge Fleet Lifecycle & OTA (mostly independent;       │
    reuses P11 audit + P14 security patterns)             │
P19 Security Certification & Compliance (collects evidence│
    from P11-P13; pen-test program)                       │
P20 Deployment Architecture & Scale (needs P12 HA design, │
    P11 capacity signals; last by trigger)                │
```

Dependency rules for implementation: no phase starts before its listed
predecessors' records exist; cross-phase seams (e.g., trace_id, audit chain,
permission sets) are defined once in the earlier phase and consumed, never
redefined.

## §10 Relationship to the Phase 0–10 program

1. Everything in 0–10 is WRITTEN-UNRUN; the first human build/test pass is
   still the program's actual gate and gates the start of phases 11–20
   implementation (planning can proceed; implementation cannot precede a
   green build without the owner's explicit exception record).
2. Known 0–10 defects are first-class inputs: F-P10.07-5 (clock median test)
   is expected to surface at first `cargo test`; the P8.25 F-1 HIGH
   unguarded `torch.load` blocks with-framework GNN activation; no update
   producer (closed by Phase 18); LR-204 unverified backups (closed by
   Phase 12); LR-047/048 identity gaps (closed by Phase 13); LR-074/190
   market-store gap (closed by Phase 16); LR-203 test-grade chains
   (upgraded by Phase 11 with production crypto as a deployment
   requirement); KN-0 LR-numbering reconciliation stays with the register
   owner.
3. The 15 open P0/P1 human-gate criteria and D1–D6 remain human-closed; new
   E-decisions join them in the per-phase plans.
4. Nothing here changes the release ladder, ADR-010, or the never-granted
   permission set; Phase 16 explicitly revisits ADR-011's boundary
   honestly: it forbids a *generic* tariff adapter, and market adapters
   remain jurisdiction-specific records, not a generic engine.

## §11 Source register

Track B — standards: [S-1] IEEE 2030.5-2018, standards.ieee.org/ieee/2030.5/6378/ · [S-2] IEEE P2030.5.1 PAR, standards.ieee.org/ieee/2030.5.1/11913/ · [S-3] SunSpec CSIP Implementation Guide v2.1, sunspec.org/wp-content/uploads/2019/08/CSIPImplementationGuidev2.103-15-2018.pdf · [S-4] SunSpec IEEE 2030.5 V2G-AC Profile v1.0, sunspec.org/wp-content/uploads/2009/03/SunSpec-IEEE-2030.5-V2G-AC-Profile-v1.0.pdf · [S-5] QualityLogic Intro to 2030.5, qualitylogic.com/wp-content/uploads/2020/08/QL-Intro-to-2030.5-Webinar-082620.pdf · [S-6] Opal-RT IEEE 2030.5 explainer, opal-rt.com/blog/what-ieee-2030-5-means-for-der-communication-and-dispatch-interoperability · [S-7] dersec.io 2030.5 compliance, dersec.io/ieee-2030-5-compliance-what-der-operators-need-to-know · [S-8] Cogito Group IEEE 2030.5 CA types, cogitogroup.net/ieee-20305-ca-types · [S-9] SunSpec CA Rule 21 PKI primer, solarplaza.com/resource/12008/sunspec-ca-rule-21-pki-primer · [S-10] OpenADR Alliance, openadr.org (+ /specification-download) · [S-11] OpenADR 2.0b Profile Spec, gridlinktechnologies.com/wp-content/uploads/2021/05/OpenADR_2_0b_Profile_Specification_v1_1-4.pdf · [S-12] IEC 61850-7-420:2021, webstore.iec.ch/en/publication/6633 · [S-13] NIST TN 2217 DER profile, nvlpubs.nist.gov/nistpubs/TechnicalNotes/NIST.TN.2217.pdf · [S-14] DNP Users Group IEEE 1815.2-2025, dnp.org/ieee-releases-18152-standard-for-der-communications · [S-15] MESA IEEE 1815.2 Guidelines, mesastandards.org/wp-content/uploads/MESA-IEEE-1815.2-Guidelines-Overview-2.pdf · [S-16] SunSpec Modbus, sunspec.org/modbus/ · [S-17] ENTSO-E CIM for grid model exchange, entsoe.eu/digital/common-information-model/cim-for-grid-models-exchange · [S-18] IEC 61968-5:2020, standards.iteh.ai/catalog/standards/clc/f75807e9-8ef3-4bac-a6cc-9f3d359690d2/en-iec-61968-5-2020 · [S-19] MA TSRG Inverter SRD, mass.gov/doc/tsrg-inverter-source-requirements-document/download · [S-20] IEEE 1547-2018, standards.ieee.org/standard/1547-2018.html · [S-21] CEC UL 1741 SB addition, energy.ca.gov/sites/default/files/2022-03/Addition_of_UL_1741_3rd_Ed_Supplement_SB_to_the_Solar_Equipment_Lists_ADA.pdf · [S-22] ISASecure, isasecure.org · [S-23] NERC joint white paper on DER aggregators, nerc.com/globalassets/our-work/white-papers/jointwhitepaper_privacyandsecurityimpactsofderaggregators.pdf · [S-24] NERC CIP Roadmap (Jan 2026), nerc.com/globalassets/our-work/reports/special-reports/nerc_cip_roadmap_01122026.pdf · [S-25] NERC CIP-015-1, nerc.com/standards/reliability-standards/cip/cip-015-1 · [S-26] FERC Order 2222 explainer, ferc.gov/ferc-order-no-2222-explainer-facilitating-participation-electricity-markets-distributed-energy · [S-27] NARUC 2222 tracker, pubs.naruc.org/pub/0C205FC7-C2D1-071A-B5F3-70BF27231837 · [S-28] ENTSO-E Balancing, entsoe.eu/network_codes/eb/ · [S-29] PJM Regulation Redesign Phase 1 FAQ, pjm.com/-/media/DotCom/markets-ops/ancillary/regulation-redesign-phase-1-faq.ashx · [S-30] CAISO BPM for Demand Response v10, bpmcm.caiso.com/BPM%20Document%20Library/Demand%20Response/BPM_for_Demand_Response_V10_Redline.pdf · [S-31] NYISO M-25 metering manual, nyiso.com/documents/20142/2923231/M-25-rev_mtr_req_mnl.pdf · [S-32] FEMP M&V options, energy.gov/cmei/femp/measurement-and-verification-options-federal-energy-and-water-saving-projects · [S-33] ANSI C12.20 accuracy classes, blog.ansi.org/ansi/ansi-c12-20-2015-electric-meters-accuracy-classes · [S-34] ENTSO-E RfG, entsoe.eu/network_codes/rfg · [S-35] EUR-Lex 2016/631 (RfG), eur-lex.europa.eu/eli/reg/2016/631/oj/eng · [S-36] Piclo, piclo.com · [S-37] ENA flexibility overview, energynetworks.org · [S-38] Enode, enode.com · [S-39] Ubidots MQTT vs Sparkplug, ubidots.com/blog/mqtt-sparkplug-vs-plain-mqtt · [S-40] OpenFMB, openfmb.org/learn

Track C — commercial: [S-41] GE Vernova GridOS DERMS, gevernova.com/software/products/gridos/distributed-energy-resources-management-system · [S-42] EnergyHub, energyhub.com · [S-43] Uplight VPP, uplight.com/solutions/virtual-power-plant · [S-44] Uplight security, uplight.com/resources/integrated-approach-security-privacy-compliance · [S-45] Enode SOC 2 update, enode.com/updates/introducing-api-versioning-soc-2-accreditation-and-support-for-new-features · [S-46] Kraken, kraken.tech (+ engineering.kraken.tech EV VPP) · [S-47] Voltus, voltus.co · [S-48] CPower, cpowerenergy.com · [S-49] Next Kraftwerke VPP, next-kraftwerke.com/vpp · [S-50] Tesla Autobidder/Opticaster, tesla.com/support/energy/tesla-software/autobidder (+ /opticaster) · [S-51] GridBeyond taxonomy, gridbeyond.com/virtual-power-plant-vs-derms-vs-aggregation-whats-the-difference (+ /our-technology) · [S-52] Camus Energy, camus.energy · [S-53] Fluence Nispera, fluenceenergy.com/nispera-energy-asset-performance-management-software · [S-54] Emerson Ovation BESS EMS, emerson.com/en/automation-systems/distributed-control-systems-dcs/ovation-distributed-control-system/battery-energy-management-system · [S-55] OpenEMS, github.com/OpenEMS/openems · [S-56] VOLTTRON, volttron.org · [S-57] Enel NA DR, enelnorthamerica.com/insights/blogs/demand-response-june-2025-heatwave · [S-58] sonnen USA, sonnenusa.com (+ CharIN V2G S-131 for rewards framing) · [S-59] RMI VPP Metering & Telemetry, rmi.org/app/uploads/dlm_uploads/2025/10/virtual-power-plant-metering-and-telemetry.pdf · [S-60] NJ BPU VPP straw proposal, nj.gov/bpu/pdf/publicnotice/VPP%20Straw%20Proposal%20Final.pdf

Track D — engineering: [S-61] Google SRE Workbook, sre.google/workbook/alerting-on-slos/ · [S-62] OTel logs data model, opentelemetry.io/docs/specs/otel/logs/data-model/ · [S-63] OTel Rust, opentelemetry.io/docs/languages/rust/ · [S-64] OTel Collector gateway, opentelemetry.io/docs/collector/deploy/gateway/ · [S-65] NATS JetStream, docs.nats.io/nats-concepts/jetstream · [S-66] TimescaleDB docs, docs.timescale.com · [S-67] Patroni replication modes, patroni.readthedocs.io/en/latest/replication_modes.html · [S-68] Postgres backup tools comparison, dev.to/piteradyson/postgresql-backup-tools-comparison-databasus-wal-g-pgbackrest-and-barman-2kg · [S-69] RLS leak across pooled connections, stackoverflow.com/questions/80003232 · [S-70] AWS multi-tenant RLS guide, aws.amazon.com/blogs/database/multi-tenant-data-isolation-with-postgresql-row-level-security · [S-71] pgroll / zero-downtime migrations, github.com/xataio/pgroll + encore.dev/articles/zero-downtime-migration · [S-72] When not to use Kubernetes, medium.com/write-a-catalyst/when-not-to-use-kubernetes-5d4200793133 · [S-73] Argo CD vs Flux, docs.aws.amazon.com/prescriptive-guidance/latest/choose-devops-tools/best-practices-argocd-flux.html · [S-74] Vault database secrets engine, developer.hashicorp.com/vault/docs/secrets/databases · [S-75] HashiCorp short-lived credentials, hashicorp.com/en/blog/why-we-need-short-lived-credentials-and-how-to-adopt-them · [S-76] SPIFFE, spiffe.io · [S-77] Enterprise SSO providers + Keycloak, workos.com/blog/enterprise-sso-providers-b2b-saas + keycloak.org · [S-78] SLSA levels, slsa.dev/spec/v1.0/levels (v1.2 current — verify) · [S-79] Sigstore, docs.sigstore.dev · [S-80] SOC 2 Type II mechanics, vanta.com/collection/soc-2/soc-2-compliance-requirements + drata.com/learn/soc-2/type-2-overview · [S-81] Litmus steady-state hypothesis, litmuschaos.github.io/tutorials/tutorial-defining-steady-state-hypothesis/index.html · [S-82] Gremlin game days, gremlin.com/docs/fault-injection-gamedays · [S-83] Stripe webhooks, docs.stripe.com/webhooks · [S-84] RFC 8594 Sunset, rfc-editor.org/rfc/rfc8594 · [S-85] Speakeasy rate limiting + API versioning, speakeasy.com/api-design/rate-limiting + speakeasy.com/blog/api-versioning-best-practices · [S-86] GDPR Art. 17, gdpr-info.eu/art-17-gdpr · [S-87] GDPR erasure exceptions, safeguard.sh/resources/blog/gdpr-right-to-erasure-exceptions-and-limits · [S-88] Immutable/hash-chained audit logs, emergentmind.com/topics/immutable-audit-log · [S-89] OpenLineage, openlineage.io

Track E — markets: [S-90] PJM FERC 2222 participation model overview, pjm.com/-/media/DotCom/committees-groups/subcommittees/disrs/postings/ferc-order-no-2222-overview.pdf · [S-91] PJM Manual 11, pjm.com/-/media/DotCom/documents/manuals/m11.pdf · [S-92] PJM Manual 18, pjm.com/-/media/DotCom/documents/manuals/m18.pdf · [S-93] PJM Manual 29 settlement formats, pjm.com/-/media/DotCom/documents/manuals/m29-redline.pdf · [S-94] PJM DataMiner2 reg_market_results, dataminer2.pjm.com/feed/reg_market_results/definition · [S-95] PJM regulation precision scoring coverage, modoenergy.com · [S-96] CAISO demand response, caiso.com/generation-transmission/load/demand-response · [S-97] CAISO Tariff §11 settlements (Nov 2024), caiso.com/documents/section-11-california-iso-settlements-and-billing-as-of-nov-1-2024.pdf · [S-98] CAISO Expanded Metering & Telemetry Options Phase 2, caiso.com/Documents/DraftFinalProposal_ExpandedMetering_TelemetryOptionsPhase2_DistributedEnergyResourceProvider.pdf · [S-99] CAISO BRS for FERC 2222, caiso.com/documents/business-requirements-specification-ferc-order-2222.pdf · [S-100] ERCOT ADER pilot, ercot.com/mktrules/pilots/ader · [S-101] ERCOT ADER Phase 3.3 report, ercot.com/files/docs/2026/05/22/10.1-Aggregate-Distributed-Energy-Resource-ADER-Pilot-Project-Phase-3.3.pdf · [S-102] ERCOT registration & qualification, ercot.com/services/rq · [S-103] ISO-NE Order 2222 participant readiness, iso-ne.com/participate/support/participant-readiness-outlook/ferc-order-no-2222 · [S-104] ISO-NE metering & telemetry checklist, iso-ne.com/static-assets/documents/100037/order2222_unit4-1_meteringandtelemetry_checklist_companion_final.pdf · [S-105] NYISO Metering Fundamentals (Dec 2024), nyiso.com/documents/20142/3035389/Metering-Fundamentals.pdf · [S-106] NYISO DER 6-second telemetry MIWG deck, nyiso.com/documents/20142/2549675/ · [S-107] EBGL (EU) 2017/2195, eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32017R2195 · [S-108] ACER Decision 18-2020 (ISHM), eepublicdownloads.entsoe.eu/clean-documents/nc-tasks/200715_A52(2)_ACER%20Decision%2018-2020 · [S-109] RTE 15-minute ISP transition, services-rte.com/en/news/br-party-system-transition-to-the-15-minute-imbalance-settlement.html · [S-110] aFRR-IF amendment (Jan 2024), eepublicdownloads.entsoe.eu/clean-documents/nc-tasks/240131_Art%2021_EB%20Regulation_allTSOamendment%20aFRR-IF · [S-111] ENTSO-E Transparency Platform + API guide, entsoe.eu/data/transparency-platform + transparency.entsoe.eu/content/static_content/Static%20content/web%20api/Guide.html · [S-112] Amperon price forecasting, amperon.co/products/price-forecasting · [S-113] EU DSO Entity flexibility practices report (Feb 2026), eudsoentity.eu/wp-content/uploads/2026/02/Report-on-Distributed-Flexibility-Practices-Markets-for-Local-Services-2.pdf · [S-114] NGED flexibility procurement report 2025-26, dso.nationalgrid.co.uk/downloads/15152/ · [S-115] Elexon P483 (live 27 Nov 2025), elexon.co.uk (via industry coverage) · [S-116] OpenADR DR Program Guide, openadr.org/assets/openadr_drprogramguide_v1.0.pdf · [S-117] OpenLeadr reporting, openleadr.org/docs/reporting.html · [S-118] Navitasoft VPP settlement blueprint, navitasoft.com/en/news/disaggregation-settlement-virtual-power-plants-vpp-blueprint-8 · [S-119] German TSO prequalification (FCR 30 s / aFRR 5 min), regelleistung.net (v1.05, Jul 2024) · [S-120] ESIG MMS aggregated-DER bids, esig.energy (Session 2A presentation)

Track F1 — EV: [S-121] OCA OCPP protocols, openchargealliance.org/protocols/open-charge-point-protocol · [S-122] OCA What is new in OCPP 2.0.1, openchargealliance.org/wp-content/uploads/2024/01/new_in_ocpp_201-v10.pdf · [S-123] OCA OCPP 2.1 announcement, openchargealliance.org/ocpp-2-1-is-now-available · [S-124] ocpp.md OCPP 2.0.1 smart charging, ocpp.md/ocpp-2.0.1/smart-charging · [S-125] ocpp.md OCPP 2.1, ocpp.md/ocpp-2.1 · [S-126] OCPPLab security profiles, ocpplab.com/blog/ocpp-security-profiles-explained · [S-127] OCA Security Operations Guide (63 pp.), openchargealliance.org · [S-128] ISO 15118-20:2022, iso.org/standard/77845.html · [S-129] Typhoon HIL ISO 15118-2 reference, typhoon-hil.com/documentation/typhoon-hil-software-manual/References/iso15118_protocol.html · [S-130] TRENDPOWER PnC guide, trendpowercharge.com/iso-15118-plug-and-charge-guide · [S-131] CharIN V2G, charin.global/technology/v2g (+ /technology/mcs) · [S-132] Nuvve school-bus V2G, investors.nuvve.com/news-releases/news-release-details/nuvve-launches-first-v2g-capable-electric-school-bus-deployment · [S-133] Argonne 2024 EVSE security report, publications.anl.gov/anlpubs/2024/09/190544.pdf · [S-134] OCPI 2.3.0, evroaming.org/wp-content/uploads/2025/02/OCPI-2.3.0.pdf · [S-135] AFIR (EU) 2023/1804, eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX%3A02023R1804-20260108 · [S-136] Eichrecht guide, elvo.io/blog/eichrecht-compliance-guide (+ ampeco.com Eichrecht) · [S-137] Tesla Fleet Telemetry, developer.tesla.com/docs/fleet-api/fleet-telemetry/available-data · [S-138] evcc stale-SoC issue, github.com/evcc-io/evcc/issues/26486 · [S-139] Geotab EV monitoring, geotab.com · [S-140] DOE FEMP smart charge management, energy.gov/cmei/femp/smart-charge-management-implementation-federal-fleets · [S-141] Ampcontrol OCPP smart charging, ampcontrol.io/ocpp-guide/how-to-use-smart-charging-with-ocpp · [S-142] IEC 61851 charging modes, allaboutcircuits.com/technical-articles/four-ev-charging-modes-iec61851-standard · [S-143] Driivz, driivz.com · [S-144] QualityLogic V2G-AC implementation, qualitylogic.com/knowledge-center/implementing-interoperable-v2g-ac

Track F2 — asset tech: [S-145] Werner et al. 2021 calendar aging, mdpi.com/1996-1073/14/2/333 · [S-146] NREL BLAST, nrel.gov/transportation/blast · [S-147] BLAST-Lite, github.com/NREL/BLAST-Lite · [S-148] Wankmüller et al. 2017 degradation penalty, sciencedirect.com/science/article/am/pii/S2352152X16303231 · [S-149] Modo Energy BESS revenue, modoenergy.com/research/en/how-does-battery-energy-storage-make-money · [S-150] Tesla Megapack, tesla.com/megapack · [S-151] Fluence 25-year guarantees, blog.fluenceenergy.com/maximizing-battery-storage-lifespan-value · [S-152] Avantis BESS warranty guide, avantisenergy.eu/battery-warranty-throughput-cycles-years · [S-153] Sunlith C&I BESS warranty guide, sunlithenergy.com/ci-bess-warranty-guide · [S-154] BESS control hierarchy (BMS/EMS/SCADA), batterydesign.net/system/battery-energy-storage-systems/bess-control-hierarchy-bms-ems-scada/ · [S-155] Robustel BMS/PCS/EMS data collection, robustel.com/bms-pcs-and-ems-data-collection-how-bess-site-data-moves-from-field-systems-to-monitoring-platforms · [S-156] SunSpec IEEE 2030.5 in production (Dec 2025), sunspec.org/blog-ieee-2030-5 · [S-157] AEMO Project EDGE reports, aemo.com.au/initiatives/major-programs/nem-distributed-energy-resources-der-program/der-demonstrations/project-edge/project-edge-reports · [S-158] Project EDGE Workstream 3 (DOE update frequency), aemo.com.au/-/media/files/initiatives/der/2023/project-edge-determining-the-impact-of-update-frequency-on-operating-envelope-efficacy--workstream-3.pdf · [S-159] SAPN Flexible Exports, sapowernetworks.com.au/industry/flexible-exports · [S-160] Amber Electric flexible-export note, help.amber.com.au/hc/en-us/articles/36080407522573 · [S-161] TUF specification v1.0.37, theupdateframework.github.io/specification/latest/ · [S-162] IETF RFC 9019 (SUIT), datatracker.ietf.org/doc/rfc9019/ · [S-163] IEC 62443-4-2:2019, webstore.iec.ch/en/publication/34421 · [S-164] UL 2900 overview, blackduck.com/glossary/what-is-ul-2900.html · [S-165] FIDO Device Onboard, fidoalliance.org/device-onboarding-overview · [S-166] IETF T2TRG security setup draft, ietf.org/archive/id/draft-irtf-t2trg-security-setup-iot-devices-06.html · [S-167] IDEMIA eSIM IoT Manager, idemia.com/esim-manager-iot-devices · [S-168] MDPI DER state-estimation twin, mdpi.com/1996-1073/14/3/774 · [S-169] CHINT IEEE 1547-2018 / UL 1741 SB bulletin, chintpowersystems.com/wp-content/uploads/2023/05/Technical-Bulletin-IEEE1547-2018-and-UL-1741-SB.pdf · [S-170] Intepro UL 1741 SB vs IEEE 1547.1, inteprosystems.com/insights/ul-1741-sb-vs-ieee-1547-1

Track C additions: [S-171] Tibber Grid Rewards, tibber.com/en/magazine/inside-tibber/grid-rewards-and-energy-markets · [S-172] Generac/ecobee Grid Resiliency + Community Energy Savings, generac.com (grid-services pages, via search)

UNVERIFIED items carried from research (do not cite as fact): IEC 61968-128
(does not exist publicly; verified part is 61968-5 [S-18]); "ECR" as an EBGL
product; UL 1741 Supplement SC details; PJM <10 MW telemetry-exemption
threshold; ERCOT "4-second" telemetry (2-second ICCP confirmed [S-100]);
PJM Regulation Phase 2 exact date (reported Dec 1, 2026); school-bus V2G
revenue figures; ISO 15118-20 Amendment 1 (Jul 2026) contents; OCPP 2.1 "AC
charging flexibility" as a distinct feature; Profile-3-mandates-TLS-1.3
claim (OCA guide allows TLS 1.2+); VictoriaMetrics/Mimir vendor benchmarks;
ISO 27001 Annex A specifics; revenue-split percentage norms; settlement data
volume benchmarks.
