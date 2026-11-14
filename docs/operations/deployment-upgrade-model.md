<!-- P10.24 — Deployment and upgrade model documentation (ADDITIVE doc). HONEST
     LABEL: this document describes the DESIGN of the v1.0.0 deployment and
     upgrade model — NOT a deployed system. No deployment exists; every
     site-specific value is a PLACEHOLDER; every referenced test (upgrade/
     rollback, migration, backup/restore) is WRITTEN-UNRUN and its record
     cells are EMPTY. Nothing here is evidence of any deployment, upgrade,
     or restore having happened. -->
# FlexCore v1.0.0 — Deployment and Upgrade Model (DESIGN — not a deployed system)

| Field | Value |
|---|---|
| Subtask | P10.24 (`ledger/phase-10-v1-release-hardening/ledger_phase_10.md` §P10.24); decision record `ledger/phase-10-v1-release-hardening/24_deployment_upgrade_model.md` |
| Document status | **DESIGN DOCUMENT — describes the model the release claims, NOT a deployed system.** No environment was deployed, configured, upgraded, backed up, or restored while writing it. Every site-specific value is a `<PLACEHOLDER>`. |
| Control status | Operator-approved control by default at v1.0.0, per deployment and per site, ONLY after the operator-approved qualification chain (§21.4, P10.15 — VERDICT: NOT QUALIFIED, recorded). Simulation and read-only/shadow are the unqualified defaults. FlexCore is a supervisory orchestration layer — no protection-grade, hard-real-time, or sub-cycle control. |
| Verification status | **WRITTEN, NEVER EXECUTED.** The review-deployment of this guide (a second person following it in a clean environment) has NOT happened — recorded as the open acceptance item, not assumed. |
| Sources | Report §21.1–§21.4; `ledger/phase-09-pilot-qualification/05_site_network_design.md` (P9.05 zone model); `ledger/phase-10-v1-release-hardening/12_upgrade_rollback_rc.md` (P10.12 — EMPTY run record); `ledger/phase-10-v1-release-hardening/13_schema_migration_compat.md` (P10.13 — windows are recorded positions, runs EMPTY); `ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` (P10.11 — EMPTY run record); `docs/operations/edge-agent-operations.md` (edge install/enroll/update/recover); `deploy/edge/flexcore-edge-agent.service`; `deploy/compose/` |
| Units | MW, MVAr, MVA, MWh; `P > 0` = export at the point of connection (§10.2); SOC a fraction in [0,1] at the API boundary |

## 1. Deployment modes (§21.1) — which modes the release claims

| Mode (§21.1) | v1.0.0 status | Notes |
|---|---|---|
| Local developer | Supported (compose: `deploy/compose/`) | development only; never a pilot posture |
| Public demo/lab | Supported | simulation-only data; no site material |
| Customer private cloud | The pilot-claim mode | the mode this document details |
| Customer on-premises | The pilot-claim mode (same model, customer-owned hosts) | identical component topology; the network design governs |
| Hybrid | Supported per the §21.1 definition | edge on-premises, control plane in the customer's cloud |

**The pilot uses: `<PLACEHOLDER — the customer private cloud / on-premises selection is a per-customer decision recorded in the signed scope freeze (P10.01 §site cells, currently EMPTY)>`.**

Per-mode deployment procedure: the component topology (§2), the Helm chart
(from v1.0.0, §21.4 — **documented here by reference; the chart is P10.21's
publication deliverable and does not exist yet — do not deploy from this
document until P10.21 publishes it**), database/broker setup (§2.1), edge
enrolment (§2.2), and the health/readiness probes (§2.3).

## 2. The deployment topology model (from the P9.05 zones)

The five-zone model is fixed by `ledger/phase-09-pilot-qualification/05_site_network_design.md` §3; zone MEMBERSHIP for a real site is customer-specific and a `<PLACEHOLDER>` filled only by the customer network/OT owner's review:

```text
Zone 1  Enterprise IT            (operator browser, IdP, admin)      membership: <PLACEHOLDER>
Zone 2  Central application      (control plane, PostgreSQL, NATS,   membership: <PLACEHOLDER>
                                  plugin host, workers, console)
Zone 3  Customer integration/DMZ (FlexCore edge agent, approved      membership: <PLACEHOLDER>
                                  gateways)
Zone 4  OT/site control          (site EMS, SCADA, PLCs, local       membership: <PLACEHOLDER>
                                  controllers)
Zone 5  Field/device             (inverters, batteries, EVSE,        membership: <PLACEHOLDER>
                                  meters, relays)
```

Flows (P9.05 §4; the structural direction of F1 is enforced by the built edge code, everything else `<PLACEHOLDER>` until the customer review fills the flow table):

| Flow | From → to | Protocol | Port | Initiator | Rule |
|---|---|---|---|---|---|
| F1 | Zone 3 edge → Zone 2 control plane | mTLS, `wss://`/`https://` only | `<PLACEHOLDER>` | **edge (fixed by code — no inbound listener exists)** | allow, explicit |
| F2 | Zone 3 edge → Zone 4 site EMS | `<PLACEHOLDER>` (OpenEMS REST is `http://` today — a recorded open item) | `<PLACEHOLDER>` | edge | allow, explicit |
| F3 | Zone 4 → Zone 5 | `<PLACEHOLDER>` (customer-owned) | `<PLACEHOLDER>` | site EMS | customer-owned |
| F4 | Zone 2 → external market | `<PLACEHOLDER>`; only when D1 closes | `<PLACEHOLDER>` | control plane | allow, explicit |

Standing rules (P9.05 §6, not fillable): deny-by-default everywhere; no
inbound internet access into any OT network; no direct database, NATS, or
plugin network access from any field-device network; every allow rule
carries an owner and a review date — no "temporary" rules.

### 2.1 Database and broker setup

- PostgreSQL: the schema is owned by the embedded `sqlx::migrate::Migrator`
  migration set (18 migrations, 0001..0018 — `crates/adapters/postgres/migrations/`);
  application runtime role must NOT own tenant tables and must not bypass
  RLS (§16.2). Version target: **PostgreSQL 18, UNVERIFIED pin** (P10.13 —
  nothing was installed in any authoring loop; verify at deployment).
  Retention values per data class: `<PLACEHOLDER — from the customer
  contract, P9.10; never a hard-coded universal period>`.
- NATS JetStream: stream/subject topology per the frozen P1.14 design and
  the P10.02 contract freeze (retention, replication, acknowledgement,
  replay, dead-letter policy per stream — §15.4). Connection material:
  `<PLACEHOLDER>`; secrets are references only (§4).
- No default credentials exist anywhere; every credential is a deployment
  input.

### 2.2 Edge enrolment (summary — full procedure: `docs/operations/edge-agent-operations.md` §3)

The edge agent installs per the operations guide (systemd unit at
`deploy/edge/flexcore-edge-agent.service` — written, NOT installed);
enrolment binds each edge identity to exactly ONE tenant and the approved
site/asset set; the CA/issuance service and revocation checking are
DEPLOYMENT REQUIREMENTS (seam defaults deny everything — no edge can
obtain a certificate until the deployment composes a real issuer).

### 2.3 Health and readiness probes

Component health/readiness gates each rolling-upgrade step (P10.12 §3.1
rule 3); the edge-side health vocabulary is the closed
`HEALTHY / DEGRADED_<reason> / FAILING_<reason>` set (`edge-agent-operations.md` §4 — health is never inferred from the absence of errors). Probe
endpoints/paths: `<PLACEHOLDER — filled from the P10.18 candidate's actual
composition>`.

## 3. The upgrade model (validated by P10.12's test design — run record EMPTY)

**Honest basis:** the upgrade model below is the design P10.12 wrote its
test matrix against; P10.12's run record is EMPTY (no update has ever been
built, signed, staged, or applied; no production-grade signature scheme
exists — the default verifier refuses everything). This section describes
the MODEL, not a tested path; acceptance criterion (b) ("matches the
P10.12 tested path") is met as a design match only until P10.12's record
is filled.

### 3.1 Rolling upgrades (central plane)

One component at a time in a staging environment mirroring the pilot
deployment; health/readiness probes between components; in-flight dispatch
transactions are drained or failed per §14.4, never corrupted; migrations
apply per the P10.13 compatibility windows (§3.3 below); plugin upgrades
follow the §5.6 drain rules (no activation or upgrade mid-dispatch-
transaction; drain → stop → replace; no hot-reload of safety or dispatch
providers in the 1.x line; no plugin self-upgrade or runtime code fetch).
Post-upgrade verification: version identifiers match the candidate, frozen
contract compatibility (P10.02), command/audit continuity, and a smoke
dispatch-lifecycle pass in shadow.

### 3.2 Edge upgrades — verify-before-stage and the rollback register

The edge applies updates ONLY through the verify-before-stage gate
(`crates/edge-agent/src/update_verification.rs`, cited by P10.12 §2 — not
re-specified here):

1. **Verification precedes staging precedes activation.** The six-step
   refusal order (manifest present → manifest valid → algorithm → trusted
   signer → version floor → digest match) runs BEFORE any file is written;
   a refused bundle leaves zero records and zero files.
2. **The append-only update register is the source of truth** for the
   active version (durable before memory); a two-slot A/B layout retaining
   the previous version untouched is the recommended deployment shape.
3. **Rollback** re-activates a previously-activated version only (never an
   unknown one), strictly behind the active version, compared numerically.
   A below-floor rollback is refused unless an explicit named-actor +
   named-authority override is presented — and the override rides BOTH the
   register record AND the audit line. **A rollback is never silent, and
   there is no silent downgrade path.**
4. **Downgrade/migration honesty:** rollback does NOT reverse database
   migrations — the runtime migrator applies UP only (P10.13 §2). An
   upgrade that included an irreversible migration (P10.13's register:
   0001 roles, 0012 correction rows) is rolled back by restore-from-backup
   (§5), never by an improvised down-migration. **No downgrade migration
   exists in this model, by design.**
5. **Staged rollout and health gating:** per site and per cohort with a
   hold window; a failing cohort freezes the rollout. NOTE the carried
   gap (P10.12 §3.2): no health-check → automatic-rollback wiring exists;
   a failed-update rollback is OPERATOR-INVOKED through the register rules
   until that composition contract lands.

### 3.3 Schema-migration compatibility windows (P10.13 — recorded positions, runs EMPTY)

Every migration in the v1.0.0 set carries a recorded compatibility window
(P10.13 §4); the rolling upgrade never violates the window of any
migration it applies; drift between an applied migration's checksum and
the deployed binary's embedded copy is refused (`E_STORE_MIGRATION`).
The windows are RECORDED POSITIONS from the migration files' own decision
comments — forward migration on production-shaped data has NOT run.

## 4. The configuration model (what a deployment configures, and its ceilings)

| Configuration element | Model | Ceiling / rule |
|---|---|---|
| Secrets | **References only.** Configuration carries REFERENCES to secrets (key/credential references, scheme-prefixed, e.g. the update manifest's signer-key REFERENCE — never key material); actual secret material is provisioned by the deployment's secret management: `<PLACEHOLDER — customer's secret store>`; no secret value is ever echoed in logs or Debug output | no secret value in any config file in the repository; redaction enforced by the edge config's Debug redaction |
| Profiles | Versioned, additive files: the shipped OpenEMS mapping profile `crates/edge-agent/config/openems-mapping-profile.v1.json` (version-checked; a different version is a named refusal); a pilot variant is a NEW file, the shipped v1 never edited in place (P6.10 D-10.1) | profile edits are versioned, reviewed changes — never silent flips |
| Control-authority mode ceilings | Mode is a DEPLOYMENT and PER-SITE setting (§21.4): simulation → read-only/shadow → operator-approved → (post-1.0 only) bounded automation. The v1.0.0 configuration ships with the command path disabled by default in code, config, and every shipped profile; enabling it requires the P6.14 four-condition gate (signed per-site change, signed enablement record + time-bounded token, command allowlist, site D6 record) | **no plugin, adapter, or optimizer can select or raise the mode** (the enablement state has no setter); bounded automation does not exist in the v1.0.0 configuration and is post-1.0 after separate qualification |
| Buffer bounds | `FLEXCORE_EDGE_BUFFER_CAPACITY` / `MAX_AGE_MS` / `OVERFLOW_POLICY` (exactly one accepted value, `drop-oldest-audited`) — bounded by construction, sized from the site's outage-duration × telemetry-rate assumption: `<PLACEHOLDER — measured by P9.18/P9.39, never assumed>` | never unbounded |
| TLS floors | Uplink default `tls13`; `tls12-floor` opt-in only per a signed site D6 record | never a code change — configuration only |
| Retention/residency | Per data class, from the customer contract (P9.10): `<PLACEHOLDER per class>` | no hard-coded universal period |

## 5. Degraded modes and the central-outage rule (§21.2)

- Offline edge behaviour: bounded store-and-forward (audited drop-of-oldest,
  in-order at-least-once upload on reconnect); **no new commands while
  disconnected** — the command channel is the outbound session itself.
- **The central-outage rule (verbatim intent, §21.2/§17.5): a central
  outage must not disable site-local protective controls or force an unsafe
  device state.** The local site EMS remains authoritative for fast local
  safety behaviour; FlexCore is supervisory and its absence degrades
  orchestration, never site safety.
- Adapter failure / credential expiry / stale data degrade health per the
  closed vocabulary and block what the degraded state forbids — never
  silently substituting values (`edge-agent-operations.md` §4, §6).

## 6. Backup/restore and disaster recovery (as designed; P10.11 record EMPTY)

The backup/restore model: PostgreSQL dump + NATS JetStream stream state +
durable command/audit records, restored into an ISOLATED environment;
post-restore verification of command/audit completeness, outbox/inbox
idempotency (no command re-issued by replay), schema-version match, and
scoped JetStream restore authorization (an unauthorized stream-name restore
must be refused). **The measured RPO/RTO are `<PLACEHOLDER>`** — P10.11's
run record is EMPTY and no restore has ever been performed; §21.2 forbids
setting RPO/RTO without customer and service requirements (P9.24 carries
the recorded customer requirements; the RC re-run is the human gate).

## 7. What this document does NOT establish

1. It does not describe a deployed system — no deployment, upgrade, rollback,
   migration, backup, or restore has ever happened anywhere.
2. Its upgrade section matches the P10.12 TEST DESIGN, not a tested path;
   its migration windows are P10.13 recorded positions; its backup/restore
   numbers are placeholders because P10.11 has not run.
3. The Helm chart it references is not published (P10.21 pending) — nothing
   may be deployed from this document until that publication exists.
4. It grants no field write authority: operator-approved mode requires the
   P10.15 qualification chain (VERDICT: NOT QUALIFIED); bounded automation
   does not exist in v1.0.0.
5. Every `<PLACEHOLDER>` must be filled with customer-authored values at
   real deployment time — a filled copy that inherits a placeholder is
   invalid; a placeholder that survives into an operating deployment is a
   finding, not a configuration.
6. The review-deployment of this guide (a second person following it in a
   clean environment) has NOT happened — it is the recorded open acceptance
   item for this subtask, not an exception claimed as satisfied.

## 8. Reviewer-verification procedure

1. Confirm the honest labels: this document claims DESIGN status in the
   header and §7, and every site-specific value is a `<PLACEHOLDER>` (no
   real IP, hostname, port, retention number, or RPO/RTO appears anywhere).
2. Trace §3 to `12_upgrade_rollback_rc.md` (verify-before-stage, refusal
   order, register semantics, override discipline — cited, not re-decided)
   and §3.3 to `13_schema_migration_compat.md` (windows as recorded
   positions; no downgrade migration).
3. Trace §2's zone/flow model to `05_site_network_design.md` §3–§6 (the
   F1 initiator fixed by code; the standing deny rules verbatim).
4. Confirm §4's mode-ceiling row states that no plugin can select or raise
   the mode and that bounded automation does not exist in the v1.0.0
   configuration.
5. Confirm §1 does not describe a Helm deployment as available — the chart
   is explicitly P10.21-pending.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the documentation is written; the review-deployment has
   not happened.
