<!-- P11.16 — the Compose secrets floor documentation -->

# Compose secrets floor (P11.16)

The Compose stack reads every credential from a FILE in this directory,
mounted through the `docker-compose.yml` `secrets:` blocks — never from
`.env`, never from an environment value in the compose file. Each host
gets its OWN file (the file-per-host floor): the Postgres server's
bootstrap password and the applications' database password are distinct
files even though a dev placeholder may hold the same value in both.

## Required files (gitignored)

| File | Mounted by | Consumed as |
|---|---|---|
| `postgres_server_password.txt` | `postgres` | `POSTGRES_PASSWORD_FILE` (official image `*_FILE` convention) |
| `postgres_app_password.txt` | `control-plane`, `worker` | composed into `FLEXCORE_DATABASE_URL` by the service entrypoint shim |

Create them before the first `docker compose up`:

```sh
cd deploy/compose/secrets
printf 'flexcore_dev_password' > postgres_server_password.txt
printf 'flexcore_dev_password' > postgres_app_password.txt
chmod 600 postgres_server_password.txt postgres_app_password.txt
```

The values above are the historical DEV-ONLY placeholder — allowed here
and nowhere else. The dev password never travels in `docker-compose.yml`,
in `.env`, or in `docker inspect` environment output (the app services
compose the URL in-process and exec the binary; the password exists only
in the mounted file and in the server's initialized credential).

## Restricted permissions

The Compose `secrets:` mechanism mounts the file inside the container;
the HOST-SIDE permissions are a human deployment step, not something
Compose enforces for `file:`-backed secrets:

- The secret files are created `0600` (owner read/write only) — the
  `chmod 600` above is part of the procedure, not optional.
- The files are gitignored (`.gitignore`: `deploy/compose/secrets/*.txt`)
  so no credential can be committed; the P11.17 scanner watches for
  violations and every scan run is a human gate.
- A password containing shell-special characters would break the app
  services' entrypoint shim (`$(cat ...)` composition); that shim is the
  Compose-floor shape. PRODUCTION does not use this floor: the recorded
  P11.16 DEFAULT is a secrets manager with dynamic short-lived
  credentials (Vault database secrets engine, ~1h TTL [S-74][S-75]),
  with the SQLx pool credential-refresh design (written at the pool seam
  in `crates/adapters/postgres/src/store.rs`) consuming them. The floor
  is the Compose-shape minimum, never the production answer.

## What this floor deliberately does NOT do

- No Vault deployment, no dynamic credential issuance — the Vault
  DEFAULT is a RECOMMENDATION registered in the P11.16 decision record;
  deploying it is an operator decision.
- No NATS credentials — the dev NATS stays unauthenticated; NATS
  credential deployment belongs to the same Vault DEFAULT work.
- No rotation — nothing in this directory rotates anything; rotation is
  the production design's concern (the store.rs credential-refresh
  design), never a property of static files.

All of the above is WRITTEN, NEVER RUN in this program (the loop bans
forbid docker commands); the first human `docker compose up` is the gate
that exercises it.
