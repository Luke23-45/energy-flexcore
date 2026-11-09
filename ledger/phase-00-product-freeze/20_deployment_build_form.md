# P0.20 — Deployment and build form freeze

| Field | Value |
|---|---|
| Subtask | P0.20 — Freeze the deployment and build form |
| Report trace | §21.1 (hosting and deployment modes), §21.2 (availability and recovery), §21.3 (observability), §21.4 (build form, distribution, control authority), §1.2 (application surfaces) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.15 (API surface), P0.18 (control-authority modes), P0.19 (plugin model) |
| Status | Draft for Stage 0 review |
| Decisions | D1–D6 remain OPEN (§25.2); this document decides none of them |

---

## 1. Hosting and deployment modes (§21.1 — all five, with intended use)

Reproduced from the §21.1 table (mode and intended use; the components column is summarized in the last column for Phase 1 targeting):

| Mode | Intended use | §21.1 components (abridged) |
|---|---|---|
| Local developer | Development | Compose, control plane, Postgres, NATS, simulator, Python workers, web console |
| Public demo / lab | Demonstration and evaluation | Single-node/container deployment, synthetic assets, no physical dispatch |
| Customer private cloud | Enterprise deployment | Kubernetes or customer-managed container platform, HA database/broker, central services, edge connectors |
| Customer on-premises | Data/control sovereignty | Customer-managed control plane and site edge, controlled outbound connectivity |
| Hybrid | Central portfolio management + customer site control | Central FlexCore and customer edge/EMS in OT DMZ |

The public demo/lab mode runs synthetic assets with **no physical dispatch** (§21.1), consistent with the simulation-only 0.x line (§0, §2.1; P0.18).

## 2. Sequencing rule: Compose first, Helm later (§21.1)

Start with Docker Compose for developer reproducibility. Add Helm/Kubernetes deployment artifacts after the core contracts and service lifecycle stabilize. **Kubernetes is a deployment option, not a prerequisite for modular design** (§21.1).

Consequence frozen here: Phase 1 Compose work targets the "Local developer" mode in §1; no Kubernetes artifact is a Phase 1 deliverable (ledger "Risks / failure modes": Kubernetes treated as a Phase 1 prerequisite is the named failure).

## 3. Availability and recovery (§21.2)

Service levels are defined per deployment and operating service. **Do not set RPO/RTO values without customer and service requirements** (§21.2). No RPO or RTO value appears anywhere in this document — that is the rule, not an omission.

Required capabilities (§21.2, in report order):

1. PostgreSQL backup and tested restore;
2. NATS stream replication/retention design;
3. durable command/audit records;
4. edge offline buffering;
5. idempotent replay/recovery;
6. health and readiness probes;
7. rolling upgrades and rollback;
8. schema migrations with compatibility windows; and
9. documented degraded modes.

Central-outage rule (§21.2, closing sentence): **a central service outage must not disable site-local protective controls or force an unsafe device state.** This is a platform invariant; it does not make FlexCore a protection layer (AGENTS.md §3.3; ADR-012 — no hard-real-time or protection claims). Site-local protective controls live outside FlexCore, consistent with the §14 lifecycle rule that acknowledgement is not physical delivery.

## 4. Observability (§21.3)

Correlate every workflow with (§21.3, in report order):

1. tenant/site/portfolio/asset IDs;
2. schedule and command IDs;
3. plugin/solver/model versions;
4. telemetry snapshot and grid-model version;
5. trace ID and actor identity; and
6. validation/authorization evidence.

Measure (§21.3, in report order): ingestion delay, stale telemetry, queue depth, plugin health, solver duration/status, command delivery, acknowledgement, and tracking deviation.

Log hygiene (§21.3): **never put secrets or unnecessary personal/customer data in logs.** No numeric target is attached to any measured item here — §19.4 owns the no-arbitrary-targets rule for metrics, and §21.2 forbids service levels without customer requirements.

## 5. Build and distribution form (§21.4)

Reproduced from the §21.4 build/distribution table:

| Surface | Build artifact | Runtime | Distribution |
|---|---|---|---|
| Operator Console | Static React/TypeScript bundle served by the control plane or a CDN | Current and previous major versions of Chrome, Edge, Firefox, and Safari | Bundled with the server release; no native mobile or desktop client |
| Control Plane | Linux x86-64 and ARM64 OCI image plus native binary | Linux container preferred; Windows/macOS only for development | Signed OCI image; Helm chart from v1.0.0 |
| WASM Plugin Host | Linux x86-64/ARM64 container | Linux container with sandbox and capability policy | OCI image; enabled from v0.5.0 |
| Numerical / Research Workers | Linux Python container images pinned by lockfile | Linux containers | OCI images matching the control-plane release |
| Site Edge Agent | Linux x86-64/ARM64 container or systemd service | Linux at the site or in the OT DMZ | Versioned package/OCI image with signed updates |
| CLI / Simulation Lab | Cross-platform Rust CLI plus containerized simulator environment | CLI on Linux, macOS, and Windows; simulators in Linux containers | Release archives for the CLI; OCI images for the simulation environment |

Release rule (§21.4): every release publishes **checksums, an SBOM, and a support matrix**; the Helm chart ships **from v1.0.0** (not before). The WASM plugin host is enabled **from v0.5.0** (§21.4 row 3; §5.8).

Surface corollary (§1.2): the platform is not a mobile app and not a single desktop binary — the console runs in a supported evergreen browser, the control plane and workers run as Linux services or containers, and site edge software is a separate deployment because it crosses an enterprise IT/OT boundary. No mobile app and no single-desktop build is promised anywhere in this document (§21.4 console row; §25 summary).

## 6. What this document does not do

- It sets no RPO/RTO or availability number (§3), no scale or performance target (§4), and no infrastructure sizing.
- It freezes no Compose file, Dockerfile, Helm chart, or CI pipeline: Phase 1 owns the Compose scope, checked against this note.
- It promises no field control: the entire 0.x line is simulation-only (§0, §2.1), the demo/lab mode runs synthetic assets with no physical dispatch (§1), and control-authority modes are frozen separately in P0.18 (§21.4 control-authority table).

## 7. Acceptance-criteria coverage (ledger P0.20)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| All five §21.1 modes are listed with intended use | §1 |
| The Compose-first / Helm-later rule is stated | §2 |
| All §21.2 recovery capabilities and the central-outage rule are stated | §3 (nine capabilities; outage rule; no RPO/RTO invented) |
| The §21.3 correlation list and the no-secrets-in-logs rule are stated | §4 (correlation and measured lists; log-hygiene rule) |
| The §21.4 distribution table is reproduced; no mobile app or single-desktop build is promised | §5 (table + release rule + §1.2 corollary) |

Verification method (ledger): comparison against §21.1–§21.4 — performed in §§1–5.
