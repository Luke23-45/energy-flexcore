<!-- P1.25 — Local developer Compose stack documentation -->

# FlexCore local developer stack (P1.25)

The §21.1 local-developer deployment: PostgreSQL 18, NATS 2.15.2
(JetStream), the control plane, and the worker, started with one command.

## Prerequisites

- Docker with Compose v2
- (Building the images) ~2 GB free disk for the Rust build stages

## Usage

```sh
docker compose -f deploy/compose/docker-compose.yml up --build
```

- Control plane: http://localhost:8080 (`/health`, `/ready`, `/metrics`,
  `/api/v1/openapi.json`)
- Worker probes: http://localhost:8081 (`/health`, `/ready`)
- NATS monitor: http://localhost:8222
- PostgreSQL: localhost:5432

**Migrations** run automatically at control-plane startup
(`FlexCoreStore::run_migrations`, embedded SQLx migrations — recorded
decision; no separate migrate service, no manual SQL).

**Authentication** (Phase 1 status): release images deny every API request
(401) — there is no production OIDC integration yet (P1.17). For local
exercising of the authenticated path, build the control-plane with
`--features dev-auth` and use the documented dev token format
(`Bearer dev tenant=<uuid> subject=<s> role=platform-admin`); see
`apps/control-plane/src/auth.rs`.

## Reset

```sh
docker compose -f deploy/compose/docker-compose.yml down -v
```

Removing the named volumes (`postgres-data`, `nats-data`) resets all state.

## Dev-only credentials

**P11.16 — the Compose secrets floor:** credentials no longer live in
`docker-compose.yml` or `.env`. Each Compose host reads its secret from
its own FILE under `./secrets/` (gitignored), mounted through the compose
`secrets:` blocks; create the two required files with `0600` permissions
before the first start (the exact procedure is in
[`secrets/README.md`](secrets/README.md)). The historical dev placeholder
values are allowed only inside those local files. Production secrets are
a secrets manager with dynamic short-lived credentials (the recorded
P11.16 Vault DEFAULT [S-74][S-75]; the SQLx pool credential-refresh
design is written at the pool seam in the postgres adapter) — this floor
is the Compose-shape minimum, never the production answer.

## Pinned image tags (re-verify at build time)

| Image | Tag | Note |
|---|---|---|
| postgres | `18.6` | latest minor at the 2026-11-07 cut-off (RESEARCH.md §6); next scheduled minor 12 Nov 2026 |
| nats | `nats:2.15.2` | exact patch at the cut-off (RESEARCH.md §5); `2.15.x` is not a valid Docker tag and never appears here. Re-check the latest 2.15 patch at build time and record the resolved tag here |
| rust | `rust:1.99.1` | matches `rust-toolchain.toml` (P1.02) |

## Smoke script

`./smoke.sh` brings the stack up, runs the Stage 1 workflow
(tenant/portfolio creation, cross-tenant denial assertions), and tears it
down. It requires the dev-auth-enabled control-plane build for the
authenticated calls (see the header comment in the script).
