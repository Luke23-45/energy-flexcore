# P13.20 — IdP integration lab environment specification (Keycloak compose profile + realm/client/claims config drafts; written, never run)

| Field | Value |
|---|---|
| Subtask | P13.20 — Keycloak compose profile specification (written, not run; no image pulls claimed), realm/client/claims configuration as checked-in config drafts; executes the P13-E-1 DEFAULT with the dependence recorded (E-1 stays OPEN) |
| Wave / agent | Phase 13 wave 2, Agent A |
| Status | WRITTEN-UNRUN through and through: **no image was pulled, no container was started, no realm was imported, no IdP has ever issued a token, no JWKS was fetched, no login has ever occurred.** The compose profile and the realm draft are CHECKED-IN CONFIG DRAFTS — the human `docker compose` run is the gate. Every evidence cell is EMPTY/UNRUN. |
| Sources | [S-77] (enterprise SSO + Keycloak, gap analysis §11 — the Tier 2 basis); record 01 (the P13-E-1 registration: OIDC-first, Keycloak the reference IdP and lab target — consumed, E-1 stays OPEN); record 02 (the resolver the lab exercises; the `JwksSource` port's real implementation lands here as the deployment artifact); record 03 (the group→role vocabulary the realm roles carry verbatim); record 04 §5 (P13-E-3: IdP-delegated MFA primary, TOTP fallback); record 42 (the session-timeout recommendations the realm session settings mirror); record 45 §3 (the SCIM scenarios this lab fills); P11.16 (the secrets floor — the file-per-host pattern, consumed never re-decided); P1.25 (the compose conventions: exact pins, secrets floor, no `latest`); the developer compose stack (`deploy/compose/docker-compose.yml`) as the shape |
| Files touched | `deploy/compose/idp-lab/docker-compose.idp-lab.yml` (NEW — the compose profile draft), `deploy/compose/idp-lab/realm-flexcore-lab.json` (NEW — the realm/client/claims config draft; JSON syntax validated locally, content UNVERIFIED against a live importer) |

## 1. The compose profile (what the draft specifies)

| Element | Value | Honest state |
|---|---|---|
| Image | `quay.io/keycloak/keycloak:26.8.1` — EXACT pin, never `latest` (the P1.25 discipline) | the version verified against keycloak.org on 2026-11-14 (26.8.1 released 2026-10-13, the current 26.8 patch line; Keycloak documents backwards compatibility across minor releases from 26.0 — the compat statement is the project's, cited, not tested). The image was NOT pulled; pulling is the human run's first act |
| Start command | `start-dev --import-realm` | a DEV-LAB posture, explicitly NOT the production identity posture (no HA, no TLS termination, dev-file persistence); the production posture is the P11.16 Vault DEFAULT, consumed never re-decided |
| Host port | 8081 (the control plane owns 8080 in the developer stack — no collision) | draft only |
| Bootstrap admin | `flexcore_lab_admin` (a name, not a secret) + the password as a MOUNTED SECRET FILE (`KC_BOOTSTRAP_ADMIN_PASSWORD_FILE`), per the P11.16 file-per-host floor — never an environment value, never `.env`, never compose-inline | the secret file is gitignored and does not exist yet; creating it with 0600 is the human step |
| Realm import | `realm-flexcore-lab.json` mounted read-only into the documented import directory | the import has never happened; re-import discipline (the importer does not overwrite an existing realm of the same name) is a runbook step (§4), not an automatic behavior |
| Health | `KC_HEALTH_ENABLED` on the management interface | the exact management port is an ASSUMPTION (§6) — verified at the first run |
| Persistence | the dev-file H2 volume (`idp-lab-data`) | lab state survives restarts; NOT a production identity store; the production store decision is the human owner's (the E-1 budget line) |

## 2. The realm/client/claims draft (the config contract the lab exercises)

| Element | Draft value | Consumed record |
|---|---|---|
| Realm | `flexcore-lab` | record 01 (per-tenant SSO connections are tenant data — the LAB realm is the lab tenant's connection) |
| Realm roles | EXACTLY the five closed role ids: `platform-admin`, `tenant-admin`, `operator`, `viewer`, `site-approver` — the names ARE the ids the P13.02 mapper consumes (ONE vocabulary, record 03 §3) | records 02/03/05 |
| Client | `flexcore-control-plane`: confidential (`publicClient: false`), standard flow only, NO implicit flow, NO direct access grants, NO service accounts — the confidential-client architecture of records 01/02 | record 02 |
| Client secret | DELIBERATELY OMITTED from the draft — the operator sets it at realm-import/lab-setup time (never checked in, the same discipline as the compose secrets floor) | P11.16 |
| Redirect URI | `http://localhost:8080/api/v1/auth/callback` — the SPECIFIED-NOT-IMPLEMENTED callback endpoint of record 43 §1 (the draft points at the designed surface; no route serves it yet) | record 43 |
| Web origin | `http://localhost:5173` (the vite dev console) | the console conventions |
| Token claims | `groups` (group-membership mapper, no full path) → the role ids; `flexcore_tenant_id` (hardcoded-claim mapper, value = the lab tenant UUID `0192a5e0-0000-7e2f-9a1b-3c4d5e6f8b10`, access token only) | record 02 §2 (the verified-issuer rule: the tenant claim is honored from the verified issuer — the lab exercises exactly that mapping) |
| Token/session lifetimes | access token 15 min; SSO idle 30 min; SSO max 12 h; refresh revocation on | the REGISTERED RECOMMENDATIONS of records 02/03/42 mirrored as lab values — mirrored, not re-decided |
| OTP policy | a TOTP policy present (the P13-E-3 fallback path's lab shape) | record 04 §5 (IdP-delegated MFA primary; the lab's OTP config exists so the fallback is exercisable, nothing is enrolled) |
| Lab users | five users, one per role, temporary passwords, each named `lab-<role>` | the lab fixtures for the record 45 §3 scenarios |

The draft was never imported; the mapper/attribute field spellings are
the Keycloak realm-import conventions as documented — content-level
correctness is verified only at the first import (ASSUMPTION, §6).

## 3. The P13-E-1 dependence (recorded, never closed)

This subtask executes the P13-E-1 DEFAULT (Keycloak as the reference IdP
and lab target) exactly as conventions §4 directs: the subtask says so,
the record 01 registration is cited, and **P13-E-1 stays OPEN** — the
human owner closes it with the budget/vendor decision. What this draft
implements is the DEFAULT's lab shape; a commercial IdP choice would
change the lab TARGET (this compose profile is replaced), not the
resolver design, the vocabulary, or the session semantics — records
02/03/42 hold regardless. The same discipline applies to P13-E-3: the
realm's OTP policy is the fallback's lab shape; the DEFAULT (IdP-delegated
MFA) is what the lab verifier composition implements, and E-3 stays OPEN.

## 4. The lab runbook (the human steps; every one UNRUN)

| # | Step | Gate |
|---|---|---|
| R-1 | Create `deploy/compose/idp-lab/secrets/idp_lab_bootstrap_admin_password.txt` (0600) | human |
| R-2 | Set the `flexcore-control-plane` client secret at lab setup (the secrets floor; never checked in) | human |
| R-3 | `docker compose -f deploy/compose/idp-lab/docker-compose.idp-lab.yml up` (the FIRST image pull and container start — nothing here has ever run) | human |
| R-4 | Verify the realm imported (roles/clients/users present); correct any draft drift found | human |
| R-5 | Compose the control plane against the lab issuer (the record 02 resolver; `main.rs`/wiring is a deployment act — no wiring exists in the repo) | human |
| R-6 | Run the record 45 §3 scenario tables (SC-1..SC-8) and fill their Result cells | human |
| R-7 | Exercise the P13.17 console flow against the lab (the first real login; the first session created/revoked at the resolver) | human |
| R-8 | Record the resolved image tag and any draft corrections in this record's amendments | human |

## 5. What the lab proves and what it never proves

The lab run provides EVIDENCE (first real token, first JWKS rotation, the
record 03 §4 deprovisioning window measured, the record 02 leeway against
real clock skew). It proves nothing beyond its runs: a finite lab pass is
a test outcome, never a guarantee (conventions §5); no SSO/SCIM/MFA
CONFORMANCE is claimed by any lab result — certification remains the
human-gated process (conventions §7 item 3).

## 6. ASSUMPTION / UNRUN register

1. **UNRUN** — no image pulled, no container started, no realm imported,
   no token issued, no JWKS fetched, no login, no scenario run. All
   evidence cells EMPTY.
2. **VERIFIED (this batch)** — the pinned version exists and is current
   as of 2026-11-14 per keycloak.org (26.8.1, released 2026-10-13). The
   verification is of the VERSION's existence, not of the image pull or
   any runtime behavior.
3. **ASSUMPTION** — the realm-draft field spellings and the env-var names
   (`KC_BOOTSTRAP_ADMIN_USERNAME`/`KC_BOOTSTRAP_ADMIN_PASSWORD_FILE`,
   `KC_HOSTNAME`, `KC_HEALTH_ENABLED`) follow the documented Keycloak
   conventions; the exact health-port convention is UNVERIFIED-AT-A-
   DISTANCE (not fetched this batch) — verified at the first run (R-4).
4. **ASSUMPTION** — the env-substitution-free draft (no client secret in
   the file) is deliberate; the operator-set secret (R-2) avoids
   checking in any material.
5. **OPEN** — P13-E-1 and P13-E-3 stay OPEN (§3); the lab is the
   DEFAULT's evidence-gathering environment, not a decision.

## 7. Handoff notes

1. **Human gates**: the runbook (§4) is the gate list; the lab fills the
   record 45 §3 cells and record 02 §5's "UNRUN by design" network half.
2. **P13.22 (close)**: the E-1/E-3 dependences here are the register's
   inputs; the compose/realm drafts are the checked-in artifacts to
   reference.
3. **The deploy/compose/README.md and the developer stack are untouched**
   (the lab profile is a separate directory; no existing compose file was
   edited).
