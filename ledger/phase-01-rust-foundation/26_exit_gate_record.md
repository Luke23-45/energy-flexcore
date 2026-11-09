# P1.26 — Stage 1 exit-gate demonstration record

Date: 2026-11-07. Status: **NOT RUN — runbook prepared.**

## Honest status

The §23 Stage 1 exit gate requires one end-to-end non-control workflow
through API → application use case → database → event publication,
demonstrated from a clean state. The implementation agent **did not run
it**: the loop rules ban building, running services, and executing tests —
all of which this demonstration requires. Everything below is the runbook
the human (or CI) executes; nothing here is evidence of a passing run, and
no acceptance criterion of P1.26 is ticked on the strength of this file.

## The workflow (P1.26 step 1, precisely defined)

1. `docker compose -f deploy/compose/docker-compose.yml up --build`
   (P1.25 stack: postgres 18.6, nats 2.15.2, control plane, worker).
2. Wait for `/ready` on the control plane (migrations applied) and the worker.
3. `POST /api/v1/tenants` → 201; verify the `tenants` row in PostgreSQL
   and an `OperatorActionRecorded` outbox entry (state + outbox in one
   transaction, §6.3 step 1).
4. `POST /api/v1/portfolios` (token scoped to the new tenant) → 201; verify
   the row and the outbox entry.
5. `POST /api/v1/sites` → 201; `POST /api/v1/assets` → 201; verify rows +
   outbox entries; verify the `AssetRegistered` event was published to the
   JetStream REGISTRY stream and consumed by the worker (inbox record in
   `flexcore_worker_inbox`).
6. Tenant scoping: a second tenant's token reading the first tenant's
   asset → 404 (no existence leak).
7. Outbox path under outage: stop NATS → run the workflow (state + outbox
   written, delivery pending) → start NATS → the publisher drains the
   backlog, events consumed, zero lost entries.
8. Record: commands, outputs, timings, and versions — toolchain
   (rust-toolchain.toml), postgres (`SELECT version()`), nats server
   version, workspace version.

## Automation

`deploy/compose/smoke.sh` automates steps 1–6 (and is wired into CI by
P1.27 for the dev-auth-enabled build); steps 7–8 are the outage variant to
run once manually per release. The negative/positive unit and integration
evidence lives in the crate test suites (see
`23_24_test_coverage_note.md` and each adapter's `tests/`).

## Acceptance criteria — ALL OPEN until the run is executed

- [ ] End-to-end on a clean Compose stack with zero manual SQL/NATS steps — NOT RUN
- [ ] Entities verifiable in PostgreSQL; events verifiable in JetStream/inbox — NOT RUN
- [ ] Cross-tenant read denied in the end-to-end run — NOT RUN
- [ ] NATS-outage variant passes with no lost events — NOT RUN
- [ ] Run record lists exact toolchain/Postgres/NATS/workspace versions — NOT RUN
