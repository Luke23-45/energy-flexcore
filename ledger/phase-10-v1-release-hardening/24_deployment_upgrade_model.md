<!-- P10.24 — Deployment and upgrade model documentation (decision record).
     DESIGN DOCUMENTATION ONLY: the additive ops document
     docs/operations/deployment-upgrade-model.md describes the DESIGN the
     release claims — no deployment exists, every site-specific value is a
     PLACEHOLDER, the Helm chart is P10.21-pending, and the referenced tests
     (P10.11/P10.12/P10.13) are all WRITTEN-UNRUN with EMPTY records. The
     review-deployment of the guide is a HUMAN GATE — NOT DONE, recorded as
     the open acceptance item, never assumed. -->
# P10.24 — Deployment and Upgrade Model Documentation (Design Doc Delivered — Review-Deploy NOT DONE)

Date: 2026-11-14. Subtask: P10.24 (`ledger_phase_10.md` §P10.24). Owner
role: Platform engineering. Report trace: §21.1–§21.2; §25 (pilot release:
"deployment/upgrade model"). Inputs per the ledger: P10.01 scope freeze
(`01_release_scope_freeze.md`); P10.11–P10.13 test results — **all three
records are EMPTY run records (WRITTEN-UNRUN)** (`11_backup_restore_rc.md`,
`12_upgrade_rollback_rc.md`, `13_schema_migration_compat.md`); P10.21
artifacts — **do not exist** (the Helm chart is unpublished). Concurrent
siblings cited by expected paths per ENTRY_NOTE wave-4. Deliverable: the
ADDITIVE operations document
`docs/operations/deployment-upgrade-model.md` (no existing docs file
modified) + this record.

## 1. Honest status — the plain statement

**The deployment and upgrade model is DOCUMENTED AS DESIGN and no
deployment of any kind exists.** No environment was stood up, no component
was installed, no upgrade or rollback was performed, no migration ran, no
backup was taken, no restore happened, and the guide was NOT followed by a
second person in a clean environment (ledger step 6 — the review-deployment
is a human gate, recorded as the open acceptance item, not as an exception
claimed as satisfied). The document describes the model the release CLAIMS;
its "tested path" references point at P10.11/P10.12/P10.13 records whose
run cells are all EMPTY — acceptance criterion (b) ("the upgrade/rollback
procedure matches the P10.12 tested path") is met as a DESIGN match only,
stated as such in the document itself. The ledger's acceptance criteria
(a)–(d): (a) documented at the design level — the §21.1 modes are
documented with the pilot-mode cell a PLACEHOLDER; (b) design-match only —
UNMET as a tested match; (c) documented (degraded modes and the
central-outage rule are in §5 of the doc, verbatim intent); (d) review-
deploy — NOT DONE. None is claimed beyond the design status.

## 2. What the document contains (§ references are to the ops document)

| Section | Content | Design source (cited, not re-decided) |
|---|---|---|
| §1 Deployment modes (§21.1) | the five modes and which the pilot claims; pilot-mode selection `<PLACEHOLDER>` | report §21.1; P10.01 (site cells EMPTY) |
| §2 Deployment topology | the P9.05 five-zone model with membership `<PLACEHOLDER>`; flow table F1–F4 with F1's initiator fixed by the built edge code; standing deny rules | `ledger/phase-09-pilot-qualification/05_site_network_design.md` §3–§6 |
| §2.1–§2.3 | database/broker setup (migration set, RLS rule, retention `<PLACEHOLDER>` per class), edge enrolment (summary, cites `docs/operations/edge-agent-operations.md` §3), health/readiness probes | P10.13 §2; §16.2; P9.33; P10.12 §3.1 |
| §3 Upgrade model | rolling upgrades (probe-gated, drain rules); edge verify-before-stage + the append-only rollback register + never-silent below-floor override + **no downgrade migration** (UP-only runtime migrator); health-gated rollout with the no-automatic-rollback-wiring gap carried | `12_upgrade_rollback_rc.md` §2–§4 (cited, not re-specified); `13_schema_migration_compat.md` §2/§4 |
| §4 Configuration model | references-only secrets; versioned additive profiles; mode ceilings (mode is a deployment/per-site setting no plugin can raise; command path disabled by default; bounded automation does not exist in v1.0.0); buffer bounds; TLS floors; retention/residency placeholders | P6.09/P6.10/P6.14; §21.4; P9.10 |
| §5 Degraded modes + the central-outage rule | bounded store-and-forward; no new commands while disconnected; a central outage must not disable site-local protective controls or force an unsafe device state | §21.2/§17.5; `edge-agent-operations.md` §4/§6 |
| §6 Backup/restore/DR | the model as designed; RPO/RTO `<PLACEHOLDER>` (P10.11 EMPTY; §21.2 forbids setting them without customer requirements) | `11_backup_restore_rc.md`; P9.24 |
| §7 Non-establishment | the honest limits, including that no Helm chart is published and the review-deploy is NOT DONE | — |

## 3. Every site-specific value is a PLACEHOLDER — the rule the document enforces

The document contains `<PLACEHOLDER>` for: zone membership, flow
protocols/ports, pilot-mode selection, probe endpoints, retention values
per data class, secret-store reference, buffer sizing inputs, TLS-floor
deviations, and RPO/RTO. A filled copy that inherits a placeholder is
invalid, and a placeholder surviving into an operating deployment is a
finding (§7 item 5 of the doc). No real IP, hostname, port, or numeric
deployment value appears anywhere — verified against the document text at
writing time.

## 4. Findings (recorded, not fixed)

- **F-P10.24-1:** the P10.12/P10.13/P10.11 records this model cites are
  EMPTY run records — the "tested path" the guide references is a test
  DESIGN, never an execution. The guide says so in §3 and §7; a future
  filled P10.12 record changes the tested-path claim, not this document's
  design content (any mismatch then re-opens this subtask).
- **F-P10.24-2:** the Helm chart is documented by reference only — it is
  P10.21's publication deliverable and does not exist. The document
  forbids deploying from it until the chart is published (§1, §7 item 3);
  the ledger's named risk ("a Helm chart documented but not published") is
  therefore structural, not latent.
- **F-P10.24-3:** the review-deployment (a second person following the
  guide in a clean environment) has NOT happened — recorded as the open
  acceptance item with its dependency (P10.21 artifacts + a candidate
  build). It is not waived and not excused; it is open.
- **F-P10.24-4:** the upgrade SOURCE for any real upgrade does not exist
  (no prior release, no pilot build — P10.12 §3.1); the upgrade model is
  therefore entirely forward-looking design at this date.
- **F-P10.24-5 (carried):** PostgreSQL 18 is an UNVERIFIED pin (P10.13 —
  nothing installed); the OpenEMS REST hop is `http://` today (a recorded
  open item); edge journal encryption at rest is a DEPLOYMENT REQUIREMENT
  not implemented (P6.18 D-8) — all carried into the document's honest
  basis via their cited records.

## 5. Reviewer-verification procedure

1. Confirm §1's plain statement stands verbatim; the document's header and
   §7 carry the DESIGN-not-deployed label; no executed-deployment claim
   exists anywhere.
2. Confirm every site-specific value in the document is a `<PLACEHOLDER>`
   (§3's list; grep the document for real-looking allocations — none).
3. Trace §3 to `12_upgrade_rollback_rc.md` §2–§4 and §3.3 to
   `13_schema_migration_compat.md` — cited, not re-specified; the
   no-downgrade-migration statement matches the UP-only migrator position.
4. Trace §2's zone/flow model to `05_site_network_design.md` §3–§6 (F1
   initiator fixed; standing denies intact).
5. Confirm the document does not present the Helm chart as available
   (P10.21-pending, stated in §1 and §7).
6. Confirm the review-deploy gap is stated in §7 item 6 of the document
   and in §1 of this record — present, open, not waived.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the documentation is written; the review-deployment has
   not happened.
8. Confirm no existing `docs/operations/` file was modified — the document
   is additive (`git status` shows only new files + the ledger tick).

## 6. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record or the doc):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) PRECONDITIONS for the review-deploy (ledger step 6): the P10.18
#     candidate exists; P10.21 has published the Helm chart; a clean
#     environment is available. None holds today.

# (c) SECOND-PERSON REVIEW-DEPLOY: follow docs/operations/
#     deployment-upgrade-model.md in a clean environment; record every
#     divergence between the document and the actual deployment; compare
#     the resulting deployment against the P10.12 staging result.
# (d) File the review-deploy record as the P10.24 evidence named in
#     CHECKLIST.md; re-issue the document (versioned) where the deploy
#     diverged — never patch silently.
```

Status: **WRITTEN-UNRUN** — the deployment/upgrade model documentation is
delivered as an additive design document; no deployment, upgrade,
rollback, migration, backup, restore, or review-deploy has happened; every
site-specific value is a PLACEHOLDER and stays one until a real customer
deployment fills it; the review-deployment is the open human gate.
