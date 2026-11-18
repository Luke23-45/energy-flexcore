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

## NATS TLS + source authentication (Patch B, P2.07)

The NATS server runs from [`nats-server.conf`](nats-server.conf) (TLS on
the client listener, per-user authorization, JetStream encryption at
rest) instead of CLI flags. Before the first start:

1. Copy [`.env.example`](.env.example) to `.env` and fill in real values
   (JetStream encryption key + the three NATS user passwords — generation
   commands are in the file). The stack fails fast without them.
2. Provision the server TLS material under `./secrets/nats-certs/`
   (`server-cert.pem`, `server-key.pem` `0600`, `ca.pem`; the server cert
   SAN must cover `nats`). No certificate or key is ever committed.
3. Read the `VERIFY-BUILD` list at the top of `nats-server.conf` — the
   TLS 1.3 minimum-version key and the JetStream key spelling
   (`key:` vs `encryption_key`) are both UNVERIFIED against the pinned
   2.15.2 server; confirm with `nats-server -t -c nats-server.conf`
   before trusting the file. Clients must also trust the CA (the worker
   enforces TLS with `require_tls(true)` + a `tls://` URL, but no custom
   root/client certificate is wired in code yet — see
   `apps/worker/src/main.rs`).

The worker runs in dev-key source-auth mode in this stack
(`FLEXCORE_SOURCE_AUTH_MODE: dev-key`): the simulator signs its frames
with an ephemeral in-process keypair and the worker verifies them
against it. Enforced mode (registry keys only) is the default when the
variable is unset and is the only mode past local development.

## Observability profile (FRV1-R8-001, final review)

The shipped P11.09 dashboards/datasource/rules tree
([`observability/`](observability/)) is wired to live consumers by the
INERT `observability` Compose profile — it starts nothing unless you
request it (the recorded INERT-until-trigger pattern):

```sh
docker compose -f deploy/compose/docker-compose.yml --profile observability up
```

This adds Prometheus (scrape config + the four P11.03/P11.13 rule files:
[`observability/prometheus/prometheus.yml`](observability/prometheus/prometheus.yml)),
Grafana (the shipped datasource/dashboard provisioning mounts), and the
NATS JetStream exporter the DLQ-depth alert rules read
(`jetstream_stream_messages` — the exported family names remain UNVERIFIED
against a live exporter, the recorded P11.13 caveat; confirm at the first
run). Endpoints: Prometheus http://localhost:9090, Grafana
http://localhost:3000, exporter http://localhost:7777 — all loopback-only.
The metrics-store selection decision P11-E-2 stays OPEN (the profile runs
Prometheus provisionally; see the config file's header note).

## Pinned image tags (re-verify at build time)

| Image | Tag | Note |
|---|---|---|
| postgres | `18.6` | latest minor at the 2026-11-07 cut-off (RESEARCH.md §6); next scheduled minor 12 Nov 2026 |
| nats | `nats:2.15.2` | exact patch at the cut-off (RESEARCH.md §5); `2.15.x` is not a valid Docker tag and never appears here. Re-check the latest 2.15 patch at build time and record the resolved tag here |
| rust | `rust:1.99.1` | matches `rust-toolchain.toml` (P1.02) |
| otel-collector | `otel/opentelemetry-collector-contrib:0.165.0` | release index checked 2026-11-14 (in the compose file) |
| prometheus | `prom/prometheus:v3.16.0` | release index checked 2026-11-18 (FRV1-R8-001); ASSUMPTION — re-verify at the first build |
| grafana | `grafana/grafana:13.3.1` | release index checked 2026-11-18 (FRV1-R8-001); ASSUMPTION — re-verify at the first build |
| nats-exporter | `natsio/prometheus-nats-exporter:0.20.3` | release index checked 2026-11-18 (FRV1-R8-001); ASSUMPTION — re-verify at the first build |

## Smoke script

`./smoke.sh` brings the stack up, runs the Stage 1 workflow
(tenant/portfolio creation, cross-tenant denial assertions), and tears it
down. It requires the dev-auth-enabled control-plane build for the
authenticated calls (see the header comment in the script).
