# P11.16 — Secrets management

| Field | Value |
|---|---|
| Subtask | P11.16 — the Vault DEFAULT recommendation (dynamic Postgres credentials TTL 1h, NATS creds, short-lived PKI later) [S-74][S-75]; the Compose file-per-host restricted-permissions floor; the SQLx pool credential-refresh design written at the pool seam |
| Wave / agent | Phase 11 wave 2, Agent A |
| Status | WRITTEN-UNRUN throughout: the Compose floor is written config (never pulled, built, or started — the loop bans forbid docker commands); the Vault DEFAULT is a RECOMMENDATION (nothing deployed); the credential-refresh design is written text at the seam, not deployed code. |
| Sources | [S-74] (Vault database secrets engine — dynamic credentials with TTL), [S-75] (HashiCorp: why short-lived credentials and how to adopt them); gap analysis §5 Tier 0 row 8 |
| Files touched | `deploy/compose/docker-compose.yml` (secrets blocks, POSTGRES_PASSWORD_FILE, entrypoint shims, header note), `deploy/compose/secrets/README.md` (new), `deploy/compose/README.md` (credentials section rewritten), `.gitignore` (additive secret-file ignore), `crates/adapters/postgres/src/store.rs` (the written credential-refresh design block) |

## 1. The Vault DEFAULT recommendation (registered, not deployed)

The recorded DEFAULT for production deployments — a RECOMMENDATION for the
operator, never a deployment performed here:

| Secret class | Recommended mechanism [S-74][S-75] | Status |
|---|---|---|
| Postgres credentials | Dynamic database credentials with a ~1h TTL (the database secrets engine issues per-workload credentials; leases expire and renew — a leaked credential self-invalidates) | RECOMMENDATION — no Vault exists in this repo |
| NATS credentials | Short-lived NATS credentials issued per workload (same engine-class pattern) | RECOMMENDATION — the dev NATS stays unauthenticated and is NOT faked into looking secured |
| PKI / service certs | Short-lived certificates (later; the mTLS material the edge already carries is provisioned by deployment today) | RECOMMENDATION, explicitly deferred |

What an operator must accept to act on this: standing up the secrets
manager, the per-workload issuance policy, and the pool-refresh behavior
(§3). Nothing in this batch deploys or configures any of it.

## 2. The Compose floor (the minimum, not the production answer)

`docker-compose.yml` no longer carries credentials:

- Top-level `secrets:` with `file:` sources — one file PER HOST
  (`postgres_server_password.txt` for the server, `postgres_app_password.txt`
  for the applications), never `.env`.
- The `postgres` service consumes `POSTGRES_PASSWORD_FILE` (the official
  image's `*_FILE` convention) instead of an environment value.
- The `control-plane` and `worker` services mount `postgres_app_password`
  and compose `FLEXCORE_DATABASE_URL` in an entrypoint shim
  (`sh -c` … `exec` the binary, PID 1 preserved): the credential-embedding
  URL never appears in the compose file, `.env`, or `docker inspect`
  environment output.
- Restricted permissions are a HUMAN step (Compose `file:` secrets do not
  enforce host-side file modes): the procedure in
  `secrets/README.md` requires `chmod 600`, and `.gitignore` excludes
  `deploy/compose/secrets/*.txt` so no credential can be committed.
- Honest limitation recorded in `secrets/README.md`: the shim composes the
  URL with `$(cat ...)` — a password with shell-special characters is out
  of the floor's shape; production uses the Vault DEFAULT, not static
  files.

## 3. The SQLx pool credential-refresh design (WRITTEN, NOT DEPLOYED)

Recorded as a design block at the pool seam (`store.rs`, on
`FlexCoreStore`): the pool is built once from a static URL today; the
refresh design anchors THERE because every port call already reaches the
pool through this struct. Four points: (1) the pool field becomes an
atomically swappable container; (2) a supervision task on the store's
runtime acquires the new credentials at a configured fraction of the TTL,
builds a second pool, swaps only on a successful connect, and drains the
old pool with a grace overlap so in-flight units of work finish; (3)
renewal failure fails LOUD through the composition root's health surface —
a silently expiring pool is a hidden outage; (4) the Vault client and
lease semantics stay OUT of the adapter (the composition root injects a
credential-provider function; the adapter stays credential-source
agnostic). No behavior claim attaches to this design — it names no
implemented code path.

## 4. ASSUMPTION / UNRUN register

1. **UNRUN** — every compose change, the shim, the `*_FILE` consumption,
   and the gitignore behavior. The first human `docker compose up` is the
   gate; the shim's `$$` escaping and the `sh -c` composition are verified
   only there.
2. **ASSUMPTION** — the official `postgres` image honors
   `POSTGRES_PASSWORD_FILE` (documented image convention, not executed
   here).
3. **ASSUMPTION** — the Vault mechanism descriptions in §1 are the [S-74]
   [S-75]-documented behavior of THOSE tools; no FlexCore deployment or
   configuration of them exists.
4. **NOT-STARTED** — the Vault DEFAULT, NATS credentials, and PKI are
   operator decisions on the recommendation; nothing is scheduled to run.

## 5. Handoff notes

1. **Orchestrator**: the compose file is shared-with-history (wave 1
   added the Collector gateway service); this batch's edits are confined
   to the secrets floor and the two app services' credential handling —
   the Collector service block is untouched.
2. **Human gate**: creating the two local secret files (`chmod 600`) is
   required before the first compose start; the README procedure covers
   it.
3. **A later wave implementing the refresh design** starts from the
   store.rs design block; it touches the pool field's type, which every
   port call flows through — dependency-rule checker unaffected (adapter
   crate).
