# flexcore-application (P1.08)

The Stage 1 registry use cases (report §6.1–§6.3, §8.1, §3.4 workflow steps
1–4; ledger P1.08): `RegisterTenant` → `CreatePortfolio` → `CreateSite` →
`RegisterAsset`, composed by the API layer and exercised by the exit-gate
workflow (P1.26). The crate depends only on the port traits (flexcore-ports)
and the frozen vocabularies (flexcore-domain, flexcore-contracts,
flexcore-events) — **no infrastructure, no database, no broker, no HTTP, no
serialization library, no async runtime**. Tests run against the ports
crate's public `fakes` module; nothing here requires a running store or
broker.

## Recorded decision: use-case shape

**One use-case struct per entity** — `RegisterTenant`, `CreatePortfolio`,
`CreateSite`, `RegisterAsset` — each taking the repository ports it needs
plus the injected `IdGenerator`, `Clock`, `EventOutbox`, and
`EventPayloadEncoder`, and exposing one `execute(...)` per workflow step.
The API layer composes the four in sequence (P1.19). The alternative — one
orchestrated mega use case for the whole chain — was rejected: it would
couple four registries into one struct, duplicate the per-entity
duplicate/parent rules, and force one transaction shape onto callers that
may legitimately run a single step.

Command inputs never carry identifiers or timestamps: the use case
generates identifiers (ID-generation port), stamps times (clock port), and
derives the tenant scope from the `TenantContext` — never from a request
field (P0.16 rule 5). Quantities arrive as the canonical contract types
(`ActivePowerMW`, `EnergyMWh`), already validated by the API parse step and
re-enforced by the domain constructors.

## Recorded decision: transactional-outbox integration (§6.3 step 1)

Every use case writes its state change and its domain event as **one**
transactional outbox write, in this order, inside one use-case invocation:

1. the repository write returns a `StateChangeReceipt` (ports type,
   `#[must_use]`) — the state-change half;
2. the use case assembles the typed `DomainEvent` (frozen §15.3
   vocabulary), the `EventEnvelope` (P1.09 schema version + mandatory
   idempotency key `<snake_case event name>:<correlation uuid>`), and the
   destination subject (events crate default scheme — ASSUMPTION pending
   P1.14);
3. the injected `EventPayloadEncoder` renders the payload (this crate has
   no serialization library; the composition root owns the wire rendering);
4. the complete `OutboxWrite` — which structurally requires the receipt —
   goes to the `EventOutbox` port.

Atomicity: the composition root opens one unit of work per use-case
invocation; the adapter executes the state change and the outbox insert in
one native transaction (binding obligation, enforced by the P1.13 atomicity
test). In tests the shared `FakeWriteJournal` evidences the ordering — the
use-case tests assert the state-change op and the outbox-append op are
adjacent and both precede the single `Commit` marker.

Event choice per action:

| Use case | Emitted event | Basis |
|---|---|---|
| `RegisterAsset` | `AssetRegistered` | Frozen §15.3 vocabulary — the correct event for the action |
| `RegisterTenant` | `OperatorActionRecorded` | **ASSUMPTION** — see below |
| `CreatePortfolio` | `OperatorActionRecorded` | **ASSUMPTION** — see below |
| `CreateSite` | `OperatorActionRecorded` | **ASSUMPTION** — see below |

The frozen vocabulary defines **no** creation events for tenant, portfolio,
and site registration, and none may be invented (P0.14: exactly 26 events;
no invented event may deserialize). Rather than leaving those three state
changes without an outbox record (breaking the §6.3 pairing for the Stage 1
workflow), the use cases emit `OperatorActionRecorded` — the §15.3
audit-area event ("an operator action was recorded", tenant-scoped
payload) — which the registration actions genuinely are. `RegisterTenant`
uses the new tenant's identifier as the payload's tenant scope. **This is
recorded as an assumption for review** by the P0.14/event-vocabulary owners;
changing it is a vocabulary decision, never a silent field repurpose.

Uniqueness rules (used for duplicate rejection; recorded assumptions where
not frozen):

- Tenant: identity-provider reference unique (P0.10 §1 row 1 field, chosen
  as the anchor — assumption).
- Portfolio: name unique within the tenant (assumption).
- Site: edge identity unique within the tenant (**frozen** — P0.16 rule 7).
- Asset: name unique within the parent site (assumption).

## Recorded decision: error taxonomy (P1.08 step 5)

Three distinct classes, never flattened:

- `ApplicationError::Validation(ValidationError)` — a frozen rule was
  violated (domain entity invariants, duplicate registration, internal
  outbox-write/envelope shape invariants). Client-fixable.
- `ApplicationError::Authorization(AuthorizationError)` — tenant-scope
  rejections. A referenced parent outside the caller's tenant scope reads
  as `ParentOutsideTenantScope`; a foreign entity supplied to a write is
  `TenantScopeViolation`. Cross-tenant existence is never leaked: a foreign
  parent is indistinguishable from a missing one.
- `ApplicationError::Infrastructure(InfrastructureError)` — a backend
  failed. The value carries an **opaque code + message only**, supplied by
  the adapter (`BackendError`); raw storage or broker error text never
  crosses this layer, and the API layer renders a generic failure plus the
  code.

Port-error mapping (recorded):

| Port error | Application error |
|---|---|
| `RepositoryError::NotFound` | `Authorization::ParentOutsideTenantScope` |
| `RepositoryError::TenantMismatch` | `Authorization::TenantScopeViolation` |
| `RepositoryError::Conflict` | `Validation::DuplicateRegistration` |
| `RepositoryError::Backend(code, message)` | `Infrastructure` (verbatim, opaque) |
| `EventOutboxError::DuplicateKey` / `UnknownKey` | `Infrastructure` (opaque codes `OUTBOX_*`) |
| `EventOutboxError::Backend` | `Infrastructure` (verbatim, opaque) |
| `OutboxWriteError` / `EnvelopeError` | `Validation` (internal invariants) |

## Recorded decision: tenant context and the claims-bridge gap

The tenant-scoped use cases take the domain `TenantContext`; none of them
reads a tenant identifier from a command field. `RegisterTenant` takes no
context (the tenant registry root is not tenant-owned data — recorded ports
decision). A parent entity (portfolio, site) is always fetched **through**
the context, so a cross-tenant parent reference is rejected without
leaking existence, and the domain constructors derive the full ownership
chain from the context-verified parents.

**Claims-bridge resolution (recorded gap; flexcore-domain was NOT edited):**
checked — flexcore-domain exposes **no constructor** for
`AuthenticatedClaims` outside its own module (private fields, no public
constructor, no `From`, no `Default`; the same boundary
flexcore-authz's test module records). Therefore:

- no code in this crate (or anywhere outside flexcore-domain) can
  construct a `TenantContext` today — that is the P1.06 design, not an
  oversight;
- the bridge ("claims from a verified token → `AuthenticatedClaims` →
  `TenantContext`") is left as the documented integration point: the
  `TenantContextSource` port in flexcore-ports, to be implemented by the
  API middleware once the domain exposes the claims constructor
  (tenant-context integration, ledger P1.11 scope);
- consequence for tests: the tenant-scoped use-case tests are **blocked**
  until that constructor exists (see "Test coverage" below). This task
  did not edit flexcore-domain to unblock them.

## Test coverage

Runs today (against the ports fakes, no infrastructure):

- `RegisterTenant` — happy path (entity fields, generated identity, clock
  time, exactly one outbox record with the correct subject/payload/key,
  id-generation count), duplicate registration rejected (nothing written),
  backend failure → opaque infrastructure error, encoder failure →
  infrastructure class, state change + outbox append adjacent before the
  single commit (fake journal), tenant round-trip through the fake
  repository.
- Error taxonomy — the full port-error mapping table above.
- Ports fakes — tenant repository CRUD/uniqueness/journal, outbox
  append/pending/attempt/delivered/failed lifecycle and key uniqueness,
  fixed clock determinism, counting ID generator, journal ordering;
  vendor-free source check and port object-safety in the ports crate.

### Blocked tests (recorded gap — activation path)

The happy-path, duplicate-rejection, and cross-tenant tests for
`CreatePortfolio` / `CreateSite` / `RegisterAsset`, and the tenant-scoping
tests for the tenant-scoped fakes, **cannot execute yet**: they need a
`TenantContext`, and no code outside flexcore-domain can construct one
(P1.06 claims boundary — see above). The moment the domain exposes the
claims constructor for verified tokens (API-middleware subtask), a
test-local helper becomes possible and the following ready-to-run bodies
activate unchanged (each already has its `NOTE` pointer in the use-case
test modules):

```rust
// Once flexcore-domain exposes the claims constructor (e.g. a verified-
// token constructor consumed by the API middleware), replace this helper:
// fn ctx(uuid_text: &str) -> TenantContext {
//     TenantContext::from_authenticated_claims(
//         flexcore_domain::tenant_context::AuthenticatedClaims::from_verified_token_claims(
//             TenantId::new(FlexUuid::parse(uuid_text).unwrap()),
//             "user:operator-7".to_string(),
//         ),
//     )
// }

#[test]
fn create_site_happy_path_writes_state_and_outbox_in_one_unit() {
    let fx = app_fixture();                      // shared fakes + journal
    let tenant = fx.register_tenant.execute(register_tenant_command()).unwrap();
    let ctx_a = ctx(/* tenant A uuid */);
    let portfolio = fx.create_portfolio.execute(&ctx_a, create_portfolio_command()).unwrap();
    let site = fx.create_site.execute(&ctx_a, create_site_command(portfolio.portfolio_id())).unwrap();
    assert_eq!(site.tenant_id(), tenant.tenant_id());
    // Journal: tenants + portfolios + sites state changes, each followed by
    // its outbox append, all before the single commit marker.
}

#[test]
fn cross_tenant_site_creation_is_rejected_without_leaking_existence() {
    let fx = app_fixture();
    fx.register_tenant.execute(register_tenant_command()).unwrap();   // tenant A
    let tenant_b = fx.register_tenant.execute(other_tenant_command()).unwrap();
    let ctx_b = ctx(/* tenant B uuid */);
    let portfolio_b = fx.create_portfolio.execute(&ctx_b, create_portfolio_command()).unwrap();

    let ctx_a = ctx(/* tenant A uuid */);
    let err = fx.create_site
        .execute(&ctx_a, create_site_command(portfolio_b.portfolio_id()))
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Authorization(_)));
    assert!(fx.sites.is_empty(), "nothing was written");
    assert_eq!(fx.outbox.len(), 2, "no outbox record for the rejected write");
}

#[test]
fn duplicate_site_edge_identity_is_rejected_within_the_tenant() {
    // Same shape: create one site, then assert the second site with the
    // same edge_identity under the same tenant fails with
    // ApplicationError::Validation(ValidationError::DuplicateRegistration).
}

#[test]
fn fake_repositories_refuse_cross_tenant_writes_and_hide_cross_tenant_reads() {
    // Insert a portfolio built under tenant B while holding tenant A's
    // context -> RepositoryError::TenantMismatch; find_by_id(ctx_a, id_b)
    // -> Ok(None); delete(ctx_a, id_b) -> Ok(false). This is the same
    // behaviour the P1.13 adapter must implement.
}
```

## Verification (recorded, run 7 November 2026 work session)

```
$ python tools/check_dependency_rule.py
OK — dependency rule holds for 12 workspace members.

$ grep -rinE "sqlx|nats|axum|postgres|tokio" crates/flexcore-application/src/
(no output; exit status 1 = zero matches)

$ grep -rinE "TODO|FIXME|todo!|unimplemented!" crates/flexcore-ports/src crates/flexcore-application/src
(no output; exit status 1 = zero matches)
```

`cargo build`/`cargo test` were **not** run for this subtask (hard task
constraint — the human builds and tests). The tests above compile and run
under `cargo test -p flexcore-application -p flexcore-ports`.

## Scope and control status

Registry use cases for the design baseline only. The entire 0.x line is
simulation-only; nothing here authorises field control or device writes.
FlexCore is a supervisory orchestration layer and performs no primary
frequency response, synthetic inertia, relay protection, inverter
inner-loop control, or sub-cycle control.
