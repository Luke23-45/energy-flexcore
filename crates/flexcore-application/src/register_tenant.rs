// P1.08 — `RegisterTenant`: the first use case of the Stage 1 registry
//         workflow (report §10.1 row 1; §3.4 workflow step 1; ledger P1.08
//         step 2).
//
// Recorded shape decision (crate README): one use-case struct per entity.
// `RegisterTenant` takes the ports it needs (tenant repository, outbox, ID
// generator, clock, payload encoder) and **no `TenantContext`**: the tenant
// registry root is not tenant-owned data — it *is* the tenant (recorded
// ports decision; P1.12 §4.1). Platform-level tenant administration is
// gated by application authorization (P1.11), not by a context parameter.
//
// Flow (every step in one use-case invocation, inside the composition
// root's unit of work):
//
// 1. duplicate check on the identity-provider reference (the recorded
//    uniqueness anchor for tenants, P0.10 §1 row 1);
// 2. domain construction via `Tenant::try_new` (frozen invariants — no
//    validation is duplicated here);
// 3. repository insert -> `StateChangeReceipt`;
// 4. one transactional outbox write pairing the receipt with the event.
//
// Event choice (recorded assumption, crate README): the frozen §15.3
// vocabulary defines no tenant-creation event and none may be invented
// (P0.14), so the use case emits `OperatorActionRecorded` (the §15.3
// audit-area event — an operator action was recorded) with the new tenant's
// identifier as the payload's tenant scope.

use std::sync::Arc;

use crate::error::{ApplicationError, ValidationError};
use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::entities::Tenant;
use flexcore_events::events::{DomainEvent, OperatorActionRecordedPayload};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::TenantRepository;
use flexcore_ports::state_change::entities;

/// The registration input. Identifiers and timestamps are **not** input:
/// the use case generates the identifier (ID-generation port) and stamps
/// the creation time (clock port), and the tenant scope comes from
/// authenticated context — never from a request field (P0.16 rule 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterTenantCommand {
    /// The tenant name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// The identity-provider reference (P0.10 §1 row 1; uniqueness anchor).
    pub identity_provider_reference: String,
    /// The policy profile (P0.10 §1 row 1).
    pub policy_profile: String,
}

/// Registers a tenant (report §10.1 row 1). Composed by the API layer;
/// tested against the ports fakes (P1.08 acceptance).
pub struct RegisterTenant {
    tenants: Arc<dyn TenantRepository>,
    outbox: Arc<dyn EventOutbox>,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    encoder: Arc<dyn EventPayloadEncoder>,
}

impl RegisterTenant {
    /// Composes the use case from its ports.
    pub fn new(
        tenants: Arc<dyn TenantRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        encoder: Arc<dyn EventPayloadEncoder>,
    ) -> Self {
        Self {
            tenants,
            outbox,
            ids,
            clock,
            encoder,
        }
    }

    /// Registers the tenant: duplicate rejection on the identity-provider
    /// reference, domain construction, one repository insert, one outbox
    /// write — in that order, inside the caller's unit of work.
    pub fn execute(&self, command: RegisterTenantCommand) -> Result<Tenant, ApplicationError> {
        // Duplicate registration rejected: the identity-provider reference
        // is the uniqueness anchor (recorded assumption).
        if self
            .tenants
            .find_by_identity_provider_reference(&command.identity_provider_reference)?
            .is_some()
        {
            return Err(ValidationError::DuplicateRegistration {
                entity: entities::TENANTS,
                key: command.identity_provider_reference.clone(),
            }
            .into());
        }

        let tenant_id = self.ids.new_tenant_id();
        let created_at: EventTime = self.clock.now_event_time();
        let tenant = Tenant::try_new(
            tenant_id,
            command.name,
            command.description,
            command.identity_provider_reference,
            command.policy_profile,
            created_at,
        )?;

        let state_change = self.tenants.insert(&tenant)?;

        // One transactional outbox write for the state change (§6.3 step 1).
        // Recorded assumption: no tenant-creation event exists in the
        // frozen vocabulary; the audit-area operator-action event is used.
        let correlation_id: FlexUuid = self.ids.new_uuid();
        let event = DomainEvent::OperatorActionRecorded(OperatorActionRecordedPayload {
            correlation_id,
            // The registered tenant is the payload's tenant scope (the
            // record is about that tenant; recorded assumption).
            tenant_id: tenant.tenant_id(),
            event_time: created_at,
        });
        emit(
            state_change,
            event,
            correlation_id,
            created_at,
            self.encoder.as_ref(),
            self.outbox.as_ref(),
        )?;

        Ok(tenant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::InfrastructureError;
    use flexcore_events::envelope::EventEnvelope;
    use flexcore_ports::backend_error::BackendError;
    use flexcore_ports::event_outbox::OutboxWrite;
    use flexcore_ports::fakes::{
        CountingIdGenerator, FakeEventOutbox, FakeJournalOp, FakeTenantRepository, FakeWriteJournal,
        FixedClock,
    };
    use flexcore_ports::repository::RepositoryError;
    use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt, entities};

    /// Test encoder: deterministic marker string (this crate has no
    /// serialization library — the wire rendering is the composition
    /// root's, P1.19).
    struct PassthroughEncoder;

    impl EventPayloadEncoder for PassthroughEncoder {
        fn encode(&self, envelope: &EventEnvelope) -> Result<String, ApplicationError> {
            Ok(format!("payload-for-{}", envelope.idempotency_key.as_str()))
        }
    }

    struct FailingEncoder;

    impl EventPayloadEncoder for FailingEncoder {
        fn encode(&self, _envelope: &EventEnvelope) -> Result<String, ApplicationError> {
            Err(ApplicationError::Infrastructure(InfrastructureError::new(
                "E_ENCODE".to_string(),
                "payload rendering failed".to_string(),
            )))
        }
    }

    /// A repository whose backend fails on insert (opaque backend error).
    struct FailingTenantRepository;

    impl TenantRepository for FailingTenantRepository {
        fn insert(&self, _tenant: &Tenant) -> Result<StateChangeReceipt, RepositoryError> {
            Err(RepositoryError::Backend(BackendError::new(
                "E_STORE_DOWN".to_string(),
                "backend unavailable".to_string(),
            )))
        }
        fn update(&self, _tenant: &Tenant) -> Result<StateChangeReceipt, RepositoryError> {
            Err(RepositoryError::Backend(BackendError::new(
                "E_STORE_DOWN".to_string(),
                "backend unavailable".to_string(),
            )))
        }
        fn find_by_id(
            &self,
            _tenant_id: flexcore_domain::ids::TenantId,
        ) -> Result<Option<Tenant>, RepositoryError> {
            Err(RepositoryError::Backend(BackendError::new(
                "E_STORE_DOWN".to_string(),
                "backend unavailable".to_string(),
            )))
        }
        fn find_by_identity_provider_reference(
            &self,
            _identity_provider_reference: &str,
        ) -> Result<Option<Tenant>, RepositoryError> {
            Ok(None)
        }
        fn list_all(&self) -> Result<Vec<Tenant>, RepositoryError> {
            Ok(Vec::new())
        }
        fn delete(&self, _tenant_id: flexcore_domain::ids::TenantId) -> Result<bool, RepositoryError> {
            Err(RepositoryError::Backend(BackendError::new(
                "E_STORE_DOWN".to_string(),
                "backend unavailable".to_string(),
            )))
        }
    }

    struct Fixture {
        journal: FakeWriteJournal,
        tenants: Arc<FakeTenantRepository>,
        outbox: Arc<FakeEventOutbox>,
        ids: Arc<CountingIdGenerator>,
        use_case: RegisterTenant,
    }

    fn fixture() -> Fixture {
        let journal = FakeWriteJournal::new();
        let tenants = Arc::new(FakeTenantRepository::new(journal.clone()));
        let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
        let ids = Arc::new(CountingIdGenerator::new());
        let clock = Arc::new(FixedClock::at_unix_millis(1_000));
        let use_case = RegisterTenant::new(
            tenants.clone(),
            outbox.clone(),
            ids.clone(),
            clock,
            Arc::new(PassthroughEncoder),
        );
        Fixture {
            journal,
            tenants,
            outbox,
            ids,
            use_case,
        }
    }

    fn command() -> RegisterTenantCommand {
        RegisterTenantCommand {
            name: "Acme Energy".to_string(),
            description: None,
            identity_provider_reference: "idp:acme".to_string(),
            policy_profile: "profile:default".to_string(),
        }
    }

    #[test]
    fn happy_path_registers_the_tenant_and_writes_one_outbox_record() {
        let fx = fixture();
        let tenant = fx.use_case.execute(command()).expect("registration succeeds");

        // The created entity carries generated identity and the clock time.
        assert_eq!(tenant.name(), "Acme Energy");
        assert_ne!(tenant.tenant_id().uuid(), FlexUuid::NIL);
        assert_eq!(tenant.created_at().unix_millis(), 1_000);
        assert_eq!(fx.ids.generated_count(), 2, "tenant id + correlation id");

        // Exactly one outbox record, paired with the tenants insert.
        assert_eq!(fx.outbox.len(), 1);
        let writes = fx.outbox.writes();
        let write: &OutboxWrite = &writes[0];
        assert_eq!(write.state_change().entity(), entities::TENANTS);
        assert_eq!(write.state_change().operation(), StateChangeOperation::Insert);
        assert_eq!(write.state_change().tenant_scope(), None);
        assert_eq!(
            write.destination_subject(),
            "flexcore.v1.audit.operator_action_recorded",
            "the destination subject comes from the events crate's default scheme"
        );
        assert_eq!(
            write.payload(),
            format!("payload-for-{}", write.idempotency_key()),
            "the payload is exactly what the encoder rendered"
        );
        assert!(
            write
                .idempotency_key()
                .starts_with("operator_action_recorded:"),
            "the key derives from the frozen event name: {}",
            write.idempotency_key()
        );
    }

    #[test]
    fn duplicate_registration_is_rejected_and_writes_nothing() {
        let fx = fixture();
        fx.use_case.execute(command()).expect("first registration succeeds");

        let err = fx
            .use_case
            .execute(command())
            .expect_err("duplicate registration must be rejected");
        assert_eq!(
            err,
            ApplicationError::Validation(ValidationError::DuplicateRegistration {
                entity: entities::TENANTS,
                key: "idp:acme".to_string(),
            })
        );
        assert_eq!(fx.tenants.len(), 1, "no second tenant was stored");
        assert_eq!(fx.outbox.len(), 1, "no second outbox record was written");
    }

    #[test]
    fn the_state_change_and_the_outbox_append_are_registered_in_one_commit() {
        // P1.08 acceptance: the outbox record is written in the same unit
        // of work as the state change — the shared fake journal evidences
        // the ordering (state change, outbox append, one commit marker the
        // simulated composition root records after the use case returns).
        let fx = fixture();
        fx.use_case.execute(command()).expect("registration succeeds");
        fx.journal.record_commit();

        let ops = fx.journal.ops();
        assert_eq!(ops.len(), 3, "state change, outbox append, commit");
        assert_eq!(
            ops[0],
            FakeJournalOp::StateChange {
                entity: entities::TENANTS.to_string(),
                operation: StateChangeOperation::Insert,
                tenant_scope: None,
            },
            "the state change is recorded first"
        );
        assert_eq!(
            ops[1],
            FakeJournalOp::OutboxAppend {
                idempotency_key: fx.outbox.writes()[0].idempotency_key().to_string(),
                entity: entities::TENANTS.to_string(),
                tenant_scope: None,
            },
            "the outbox append follows, naming the paired state change"
        );
        assert_eq!(ops[2], FakeJournalOp::Commit, "one commit covers both");
    }

    #[test]
    fn a_backend_failure_maps_to_an_opaque_infrastructure_error() {
        let journal = FakeWriteJournal::new();
        let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
        let clock = Arc::new(FixedClock::at_unix_millis(1_000));
        let use_case = RegisterTenant::new(
            Arc::new(FailingTenantRepository),
            outbox.clone(),
            Arc::new(CountingIdGenerator::new()),
            clock,
            Arc::new(PassthroughEncoder),
        );

        let err = use_case.execute(command()).expect_err("backend fails");
        let ApplicationError::Infrastructure(infra) = err else {
            panic!("backend failures are infrastructure class")
        };
        assert_eq!(infra.code(), "E_STORE_DOWN", "opaque code passes through");
        assert_eq!(infra.message(), "backend unavailable", "human-safe message only");
        assert_eq!(outbox.len(), 0, "nothing was appended");
    }

    #[test]
    fn an_encoder_failure_maps_to_the_infrastructure_class() {
        let journal = FakeWriteJournal::new();
        let tenants = Arc::new(FakeTenantRepository::new(journal.clone()));
        let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
        let use_case = RegisterTenant::new(
            tenants.clone(),
            outbox.clone(),
            Arc::new(CountingIdGenerator::new()),
            Arc::new(FixedClock::at_unix_millis(1_000)),
            Arc::new(FailingEncoder),
        );

        let err = use_case.execute(command()).expect_err("encoding fails");
        let ApplicationError::Infrastructure(infra) = err else {
            panic!("encoder failures are infrastructure class")
        };
        assert_eq!(infra.code(), "E_ENCODE");

        // Fake limitation, recorded: the fakes do not emulate rollback, so
        // the already-inserted row remains visible here. In production the
        // composition root's unit of work rolls the whole invocation back,
        // and the adapter's atomicity test (P1.13) proves all-or-nothing.
        assert_eq!(tenants.len(), 1);
        assert_eq!(outbox.len(), 0, "no outbox record was appended");
    }

    // NOTE (recorded gap, see the crate README "blocked tests"): the
    // happy-path, duplicate, and cross-tenant tests for the tenant-scoped
    // use cases (CreatePortfolio / CreateSite / RegisterAsset) cannot
    // execute yet: no code outside flexcore-domain can construct an
    // `AuthenticatedClaims` (private fields, no public constructor — P1.06
    // deliberate boundary), so no test in this crate can obtain a
    // `TenantContext` to pass them. The same boundary is recorded in
    // flexcore-authz's test module and in the ports crate's
    // `tenant_context_source`. `RegisterTenant` is fully exercised here
    // precisely because it takes no context.

    #[test]
    fn the_registered_tenant_round_trips_through_the_fake_repository() {
        let fx = fixture();
        let tenant = fx.use_case.execute(command()).expect("registration succeeds");
        let fetched = fx
            .tenants
            .find_by_id(tenant.tenant_id())
            .expect("find succeeds")
            .expect("tenant stored");
        assert_eq!(fetched, tenant);
        let by_idp = fx
            .tenants
            .find_by_identity_provider_reference("idp:acme")
            .expect("find succeeds")
            .expect("tenant stored");
        assert_eq!(by_idp.identity_provider_reference(), "idp:acme");
    }
}
