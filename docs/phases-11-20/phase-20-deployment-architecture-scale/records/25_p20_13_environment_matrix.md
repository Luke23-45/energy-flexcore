<!-- P20.13 — Environment matrix (dev/staging/prod parity). A parity DESIGN
     table: what differs per environment (recorded, deliberate) vs what
     must stay identical. Config management shape; secret injection
     consumed from P11.16 (Vault DEFAULT, Compose file-per-host floor —
     consumed, not reinvented); the P10.24 mode table consumed as the base.
     No environment beyond the dev Compose stack exists; nothing was
     deployed, configured, or verified. -->
# P20.13 — Environment Matrix (dev/staging/prod parity — DESIGN; only the dev Compose stack exists; the P10.24 mode table consumed as the base)

| Field | Value |
|---|---|
| Subtask | P20.13 — Environment matrix (`ledger_phase_20.md` §P20.13) |
| Wave / agent | Phase 20 wave 1, Agent B |
| Status | **PARITY DESIGN RECORD — WRITTEN-UNRUN.** Exactly one environment exists in this repository: the local-developer Compose stack (`deploy/compose/docker-compose.yml`, DEV-ONLY credentials, marked). No staging environment and no production environment exists; no parity property has ever been verified between environments because there are not yet two environments to compare. |
| Consumed by citation (never restated) | P11.16 secrets management (`docs/phases-11-20/phase-11-operational-robustness/records/32_p11_16_secrets_management.md` — the Vault DEFAULT (dynamic Postgres credentials ~1h TTL, short-lived NATS credentials) and the Compose file-per-host floor — consumed, not reinvented); `docs/operations/deployment-upgrade-model.md` §1 (the P10.24 deployment-mode table — consumed as the base) and §4 (the configuration model ceilings) |
| Sources | `deploy/compose/docker-compose.yml` (the dev environment of record: postgres 18.6, nats 2.15.2 pinned EXACT tags, `/ready` healthchecks, entrypoint shims composing `FLEXCORE_DATABASE_URL` from mounted secret files); `docs/operations/deployment-upgrade-model.md` §2 (the five-zone topology the environments share by shape); P14/P16 seams only as recorded consumer shapes (no new interplay claimed) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**Only the local-developer environment exists; staging and production do
not exist, and nothing has been deployed or verified in any of them.**
The matrix below is the parity CONTRACT a future staging/prod environment
must satisfy at creation time — recorded now so that when those
environments appear, what differs is a deliberate, recorded value and
what must stay identical is not silently eroded. Every environment-specific
VALUE cell is `<EMPTY>` until a real environment exists to hold a value.

## 2. What must stay IDENTICAL across environments (the parity invariants)

| Invariant | Statement | Governs |
|---|---|---|
| I-1 Artifact identity | The SAME digest-pinned images/artifacts run in every environment — no per-environment rebuild (the P20.14 promotion property, consumed; a rebuilt image breaks parity by definition) | P20.14 (expected path) |
| I-2 Migration set | The embedded `sqlx::migrate::Migrator` set is identical (same migration checksums — drift is refused, `E_STORE_MIGRATION`; the P11.18 migration-policy discipline) | `deployment-upgrade-model.md` §2.1 |
| I-3 The 14-state command lifecycle + three gates | No environment relaxes validation, feasibility, or authorization gates | AGENTS.md §4 |
| I-4 The never-granted permission set | `dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any` — byte-identical in every environment | AGENTS.md §3 |
| I-5 Tenant isolation mechanics | RLS ENABLE + FORCE, the `SET LOCAL` tenant-GUC discipline, and the fail-closed NULL-setting behavior are identical everywhere | Migration 0008/0010 policies (cited) |
| I-6 Telemetry quality vocabulary | Exactly the six P0.08 states; no environment-local state is added | `flexcore-contracts` (cited) |
| I-7 Health/readiness semantics | `/health` liveness vs `/ready` readiness with named checks — the P1.22/P11.21 contract, never regressed per environment | `apps/control-plane/src/probes.rs` |
| I-8 Audit + audit-chain behavior | Every environment records the same audited events; the 0019 chain verifies everywhere | 0008/0019 (cited) |
| I-9 No secret values in config files | Configuration carries REFERENCES only (P11.16; `deployment-upgrade-model.md` §4) | P11.16 (consumed) |
| I-10 Versioned additive config files | Profiles/config files are versioned, additive; a variant is a NEW file, never an in-place edit (the P6.10 D-10.1 rule, consumed) | `deployment-upgrade-model.md` §4 |

## 3. What DIFFERS per environment (recorded, deliberate — every value EMPTY)

The mode row is consumed directly from the P10.24 §1 table
(`deployment-upgrade-model.md` §1): local developer, public demo/lab,
customer private cloud, customer on-premises, hybrid — the base this
matrix organizes. The differences below are the recorded, deliberate
deviations; a difference not in this table is a parity violation, and a
filled difference without a recorded basis is a finding:

| Dimension | dev (the ONLY existing environment) | staging | prod |
|---|---|---|---|
| Control-authority mode | Simulation; command path disabled by default in code, config, and every shipped profile | Shadow/read-only by default; raise requires the per-site enablement gates — `<EMPTY until a staging environment exists>` | Operator-approved only after the P10.15 qualification chain (VERDICT: NOT QUALIFIED — recorded); bounded automation does not exist in v1.0.0 (the P10.24 §4 mode-ceiling row, consumed) |
| Topology | Single-host Compose | `<EMPTY>` (the P20.04 register governs whether staging migrates) | The five-zone model with per-site `<PLACEHOLDER>` membership (P10.24 §2 — unchanged) |
| Secret injection | The P11.16 Compose file-per-host floor (`secrets/*.txt`, `*_FILE`, entrypoint shim, chmod 600 — consumed verbatim in substance) | The P11.16 Vault DEFAULT is the RECOMMENDATION for any non-dev environment — dynamic Postgres credentials, short-lived NATS credentials; standing up Vault is the operator act P11.16 §1 records | Same DEFAULT; nothing deployed — `<EMPTY>` |
| Credentials | DEV-ONLY credentials, marked (compose README) | `<EMPTY>` | `<EMPTY>` — no credential value anywhere (I-9) |
| Data | Simulation-only synthetic data; no site material | Production-shaped SYNTHETIC data only (the P20.02 policy, consumed) — never customer material | Real per-site data under signed retention/residency rows (the P13.14 registry; D1 OPEN — `<EMPTY>`) |
| Scale / replica counts | `num_replicas: 1`, single instance per service | `<EMPTY>` | `<EMPTY>` — derived from the P20.01 capacity framework when it has measured figures (Agent A, expected path) |
| Retention/residency | n/a (synthetic, disposable) | `<EMPTY>` | Per data class from the customer contract (P9.10; P10.24 §4 `<PLACEHOLDER>` rows — unchanged) |
| TLS floors | dev dev-auth overlay as recorded (`deploy/compose/smoke.sh` authenticated path) | `tls13` default (the P10.24 §4 floor row, consumed) | `tls13` default; `tls12-floor` only with a signed site D6 record |
| Buffer bounds | Compose defaults as written | `<EMPTY>` | Sized from the site's outage-duration × telemetry-rate measurement (P9.18/P9.39 — `<PLACEHOLDER>`, never assumed) |
| Observability export target | `debug` exporter placeholder (P11.06) | The P11.10/P20.15 store topology decision — `<EMPTY>` | Same — `<EMPTY>` |

## 4. Config management shape

| Aspect | Shape |
|---|---|
| Base + delta | A single base configuration shape with per-environment deltas that are EXPLICIT files/values under review — the Compose-era shape is the base file plus documented overlays (the dev-auth overlay is the existing example); the K8s-era shape (base + per-environment overlays) is Agent A's P20.05 chart layout, consumed by expected path, inert until the P20.04 trigger |
| Parity enforcement | §2's invariants are the checklist a promotion (P20.14) verifies between environments; an unverifiable invariant is a finding, not a waiver |
| Delta review rule | Every §3 difference carries a recorded basis; adding a difference is a reviewed change to this table, never a silent environment edit (the anti-silent-growth discipline, carried) |
| Secrets | Consumed from P11.16 exactly: REFERENCES only; Vault DEFAULT above the dev floor; the floor's honest limitation (shell-special characters out of the shim's shape; production uses the Vault DEFAULT) is carried, not reinvented |

## 5. What this record does NOT establish

1. No staging or production environment exists; no parity property has
   been verified anywhere; the dev Compose stack itself has never been
   brought up by this program (the compose file is written-unrun).
2. No mode is claimed anywhere beyond simulation for dev: staging/prod
   mode cells are EMPTY or consumed defaults; the P10.15 qualification
   chain verdict (NOT QUALIFIED) stands untouched.
3. No Vault, secret store, or credential exists in this repository.
4. No capacity or scale figure is asserted (Agent A's P20.01 owns the
   measurement layer; nothing is measured).
5. It adds no dispatch authority (ADR-010) and touches no file outside
   this record.

## 6. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: exactly one environment exists; the
   matrix is a forward contract, not a description of existing
   environments.
2. Confirm §2's invariants trace to their governing records and that
   none is softened; confirm I-1 states digest-pinned identity and no
   per-env rebuild.
3. Confirm §3's mode row consumes the P10.24 §1/§4 tables (cited) and
   that every staging/prod value cell is `<EMPTY>`; confirm the data row
   keeps synthetic-only outside prod.
4. Confirm §4 consumes P11.16 verbatim in substance (Vault DEFAULT +
   file-per-host floor) without re-deriving the mechanism.
5. Confirm no ledger checkbox was ticked by this agent.

## 7. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.13-1 | The parity contract assumes a future staging environment mirrors prod topology | That mirroring is the P10.24 review-deployment's open acceptance item, consumed — not claimed |
| A-P20.13-2 | The Vault DEFAULT mechanism descriptions are P11.16's [S-74][S-75]-documented behavior of those tools | Consumed by citation; no FlexCore deployment of any secrets manager exists |
| F-P20.13-1 | No second environment exists, so parity is UNVERIFIABLE today by construction | Binding honesty statement |
| F-P20.13-2 | Agent A's P20.01/P20.05 records were first consumed by EXPECTED PATH and reconciled to the landed paths during the same wave | Landed: `records/01_p20_01_capacity_model_framework.md` (the cardinality budget, §4) and `records/05_p20_05_helm_chart_design.md`; reconciliation note in the close-out record |
| A-P20.13-3 | The local-developer environment's observability wiring is the INERT `observability` Compose profile (prometheus + grafana + nats-exporter, added by the final-review fix FRV1-R8-001) | Profiles start only when requested; nothing has been pulled/run (the recorded INERT-until-trigger pattern). The P11.09 dashboards/datasource/rules tree now has its consumer at dev level only; staging/prod observability targets stay `<EMPTY>` (§3 row above) until real environments exist |

Status: **WRITTEN-UNRUN** — a parity contract cannot be verified with one
environment; the staging and production environments, their
configuration, and their verification are human gates that have not
happened and are not scheduled.
