// P10.04 — Authorization and tenant-isolation test suite (authz-crate
//         slice): the release-gate denial matrices over the recorded
//         grant tables (report §22.3 gate 3; §16.2; §17.3–§17.4; §5.7;
//         ledger `ledger/phase-10-v1-release-hardening/ledger_phase_10.md`
//         P10.04; record:
//         `ledger/phase-10-v1-release-hardening/04_authz_tenant_tests.md`).
//
// WHAT THIS FILE PROVES (exhaustively where the tables are closed):
//
// 1. THE RECORDED PHASE 4 GRANT TABLE IS EXACT. Over the FULL closed
//    matrix — 4 roles × 29 actions × 3 scope kinds (348 in-tenant
//    combinations) — `phase4_command_policies()` allows EXACTLY the
//    documented table (`crates/flexcore-authz/src/phase4_policies.rs`
//    header; PHASE4_POLICY_PREFIX = "policy:phase4") and denies every
//    other combination with `NoMatchingRule`:
//      - `command.create`   : Operator, TenantAdmin  @ tenant/site/asset
//      - `command.validate` : Operator, TenantAdmin  @ tenant/site/asset
//      - `command.approve`  : TenantAdmin ONLY       @ site/asset
//        (separation of duties by role, §17.3 control 2; NO tenant-level
//        approval grant; no Operator/Viewer/PlatformAdmin grant)
//      - `command.read`     : Operator, TenantAdmin, Viewer @ all three
//      - `feasibility.read` : Operator, TenantAdmin, Viewer @ all three
// 2. THE CROSS-TENANT DENIAL MATRIX. Every action × every role × every
//    scope kind (348 combinations) with a scope naming tenant B while the
//    context names tenant A denies with `ScopeDenied` and the canonical
//    evidence `authz:deny:scope-denied` — ZERO allows, for the recorded
//    set, the empty set, and any other set (the tenant match precedes
//    rule matching; no role bypasses it, §16.2 / §17.4 cross-tenant
//    leakage row; P0.16 rule 6: audited).
// 3. THE DENY-BY-DEFAULT PROOF. The empty `PolicySet` denies all 348
//    in-tenant combinations with `NoMatchingRule`; there is no default
//    scope (`UnknownScope`), no default context (`MissingContext`), and
//    no anonymous fallback. (The control-plane counterpart — the
//    `DenyAllResolver` that denies every UNAUTHENTICATED request when
//    `dev-auth` is off — is proven in
//    `apps/control-plane/tests/p10_04_deny_all_and_default_policies.rs`.)
// 4. THE NEVER-GRANTED SET REFUSES. `dispatch.authorize`,
//    `dispatch.issue`, `secret.read`, `network.any`, `command.issue`,
//    `command.auto_approve`, `alarm.write`, `alarm.acknowledge`,
//    `audit.event.write`, `telemetry.write` and the empty id are NOT on
//    the closed `Action` surface (§5.7: `dispatch.authorize` /
//    `dispatch.issue` are never granted to general plugins — the Phase 4
//    protected kernel): `Action::parse` returns `None` and
//    `evaluate_action_id` denies with `UnknownAction` for EVERY role,
//    even at full grant breadth — an untyped caller can only ever get a
//    reasoned deny.
// 5. SCOPE LEVELS DO NOT INHERIT and role grants are exact (pinned inside
//    the 348 matrix; the approve-at-tenant-kind refusal is called out).
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3 — the loop bans hold: no
// cargo command was executed; the human build/test gate owns the run:
// `cargo test -p flexcore-authz --test p10_04_authz_tenant_isolation`).
// Every expected value below is a literal copied from the recorded grant
// table or from the canonical deny evidence constants of
// `crates/flexcore-authz/src/policy.rs` — nothing is fabricated.
//
// Scope honesty: this file proves the AUTHZ-CRATE tables. The kernel and
// research tenant-scope mirrors and the control-plane `DenyAllResolver` /
// default REST policy sets are proven in their own additive P10.04 files
// (see the record for the full file list and the RLS/retention steps that
// need a deployed candidate — recorded as NOT EXECUTABLE HERE, P10.04
// steps 3 and 6).

use flexcore_authz::action::Action;
use flexcore_authz::decision::{AllowReason, DenyReason, EvidenceRef};
use flexcore_authz::phase4_policies::phase4_command_policies;
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_authz::scope::{AssetScope, Scope, ScopeKind, SiteScope, TenantScope};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{AssetId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};

// --- Fixtures ---------------------------------------------------------------------

const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a04";

/// Canonical deny evidence references (verbatim from
/// `crates/flexcore-authz/src/policy.rs`).
const EVIDENCE_SCOPE_DENIED: &str = "authz:deny:scope-denied";
const EVIDENCE_NO_MATCHING_RULE: &str = "authz:deny:no-matching-rule";
const EVIDENCE_UNKNOWN_ACTION: &str = "authz:deny:unknown-action";
const EVIDENCE_UNKNOWN_SCOPE: &str = "authz:deny:unknown-scope";
const EVIDENCE_MISSING_CONTEXT: &str = "authz:deny:missing-context";

/// The recorded Phase 4 grant-table prefix (the allow-evidence namespace).
const PHASE4_PREFIX: &str = "policy:phase4";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant_a() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_A))
}

fn tenant_b() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_B))
}

/// The ONLY public construction path (P0.16 rule 5): verified claims →
/// authenticated claims → tenant context. Tenant context is never built
/// from request data in these tests — the same rule the API middleware
/// follows.
fn context_for(tenant: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant,
        subject: "p10-04:test:principal".to_string(),
    }))
}

/// Every action on the closed surface (29 variants — this list mirrors
/// `Action::ALL` order as of Phase 5; a new variant must be added here AND
/// to the expected tables, or the matrices below stop covering it).
const ALL_ACTIONS: [Action; 29] = [
    Action::TenantCreate,
    Action::TenantRead,
    Action::PortfolioCreate,
    Action::SiteCreate,
    Action::AssetCreate,
    Action::AssetRead,
    Action::PortfolioRead,
    Action::SiteRead,
    Action::TenantDelete,
    Action::PortfolioDelete,
    Action::SiteDelete,
    Action::AssetDelete,
    Action::AssetUpdate,
    Action::AssetDeactivate,
    Action::TelemetryRead,
    Action::QuarantineRead,
    Action::FlexibilityEnvelopeRead,
    Action::FlexibilityEnvelopeCreate,
    Action::ScheduleRead,
    Action::SchedulePropose,
    Action::OptimizationRunRead,
    Action::OptimizationRunCreate,
    Action::CommandCreate,
    Action::CommandValidate,
    Action::CommandApprove,
    Action::CommandRead,
    Action::FeasibilityRead,
    Action::AuditEventRead,
    Action::AlarmRead,
];

const ALL_ROLES: [Role; 4] = [
    Role::PlatformAdmin,
    Role::TenantAdmin,
    Role::Operator,
    Role::Viewer,
];

const ALL_SCOPE_KINDS: [ScopeKind; 3] = [ScopeKind::Tenant, ScopeKind::Site, ScopeKind::Asset];

fn scope_of(kind: ScopeKind, tenant: TenantId) -> Scope {
    match kind {
        ScopeKind::Tenant => Scope::Tenant(TenantScope::new(tenant)),
        ScopeKind::Site => Scope::Site(SiteScope::new(tenant, SiteId::new(uuid(UUID_SITE)))),
        ScopeKind::Asset => Scope::Asset(AssetScope::new(tenant, AssetId::new(uuid(UUID_ASSET)))),
    }
}

/// The expected in-tenant allow table of `phase4_command_policies()`,
/// transcribed verbatim from the module header of
/// `crates/flexcore-authz/src/phase4_policies.rs` (the "real grant table"
/// the record cites).
fn phase4_grants(role: Role, action: Action, kind: ScopeKind) -> bool {
    use ScopeKind::{Asset, Site, Tenant};
    match (role, action, kind) {
        // command.create — Operator and TenantAdmin, at every scope kind.
        (Role::Operator, Action::CommandCreate, Tenant | Site | Asset) => true,
        (Role::TenantAdmin, Action::CommandCreate, Tenant | Site | Asset) => true,
        // command.validate — Operator and TenantAdmin, at every scope kind.
        (Role::Operator, Action::CommandValidate, Tenant | Site | Asset) => true,
        (Role::TenantAdmin, Action::CommandValidate, Tenant | Site | Asset) => true,
        // command.approve — TenantAdmin ONLY, at site and asset scope kinds
        // (separation of duties; NO tenant-level approval grant).
        (Role::TenantAdmin, Action::CommandApprove, Site | Asset) => true,
        // command.read — Operator, TenantAdmin, Viewer, at every scope kind.
        (Role::Operator, Action::CommandRead, Tenant | Site | Asset) => true,
        (Role::TenantAdmin, Action::CommandRead, Tenant | Site | Asset) => true,
        (Role::Viewer, Action::CommandRead, Tenant | Site | Asset) => true,
        // feasibility.read — Operator, TenantAdmin, Viewer, at every kind.
        (Role::Operator, Action::FeasibilityRead, Tenant | Site | Asset) => true,
        (Role::TenantAdmin, Action::FeasibilityRead, Tenant | Site | Asset) => true,
        (Role::Viewer, Action::FeasibilityRead, Tenant | Site | Asset) => true,
        _ => false,
    }
}

/// The expected evidence reference of a phase-4 allow
/// (`policy:phase4:<role-id>:<action-id>:<kind-id>` — the recorded format
/// of `phase4_command_policies`).
fn phase4_evidence(role: Role, action: Action, kind: ScopeKind) -> String {
    format!(
        "{PHASE4_PREFIX}:{}:{}:{}",
        role.id(),
        action.id(),
        kind.id()
    )
}

// --- 1. The recorded Phase 4 grant table is exact (348 combinations) ---------------

#[test]
fn the_phase4_grant_table_allows_exactly_the_documented_matrix() {
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    let mut allows = 0usize;
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            for kind in ALL_SCOPE_KINDS {
                let decision = set.evaluate(role, action, Some(&scope_of(kind, tenant_a())), Some(&ctx));
                let expected = phase4_grants(role, action, kind);
                assert_eq!(
                    decision.is_allowed(),
                    expected,
                    "grant table mismatch for role={} action={} kind={}: decision={decision}",
                    role.id(),
                    action.id(),
                    kind.id(),
                );
                if expected {
                    allows += 1;
                    // The allow evidence is the rule's reference, in the
                    // recorded `policy:phase4:<role>:<action>:<kind>` format.
                    assert_eq!(decision.allow_reason(), Some(AllowReason::PolicyMatch));
                    assert_eq!(
                        decision.evidence().as_str(),
                        phase4_evidence(role, action, kind),
                        "allow evidence for role={} action={} kind={}",
                        role.id(),
                        action.id(),
                        kind.id(),
                    );
                } else {
                    assert_eq!(decision.deny_reason(), Some(DenyReason::NoMatchingRule));
                    assert_eq!(decision.evidence().as_str(), EVIDENCE_NO_MATCHING_RULE);
                }
                // Decisions are never bare bools: the full value is present
                // either way (§17.3 control 12 — auditable).
                assert!(
                    !decision.evidence().as_str().is_empty(),
                    "every decision carries non-empty evidence"
                );
            }
        }
    }
    // The recorded table has exactly this many cells (counted from the
    // builder: create 2×3, validate 2×3, approve 1×2, read 3×3,
    // feasibility.read 3×3 = 6+6+2+9+9).
    assert_eq!(allows, 32, "the recorded phase-4 table grants exactly 32 cells");
}

// --- 2. The cross-tenant denial matrix (348 combinations, zero allows) ---------------

#[test]
fn every_action_times_wrong_tenant_scope_is_scope_denied_on_the_phase4_set() {
    // Scope names tenant B; the context is tenant A. The tenant match
    // precedes rule matching, so EVERY combination denies with
    // ScopeDenied — including the 32 cells that would allow in-tenant.
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    let mut denies = 0usize;
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            for kind in ALL_SCOPE_KINDS {
                let decision = set.evaluate(role, action, Some(&scope_of(kind, tenant_b())), Some(&ctx));
                assert!(!decision.is_allowed(), "cross-tenant allow for role={} action={} kind={}", role.id(), action.id(), kind.id());
                assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied));
                assert_eq!(decision.evidence().as_str(), EVIDENCE_SCOPE_DENIED);
                denies += 1;
            }
        }
    }
    assert_eq!(denies, 4 * 29 * 3, "the matrix is exhaustive: 348 refusals");
}

#[test]
fn cross_tenant_denial_holds_even_where_the_grant_would_allow_in_tenant() {
    // Focused re-proof of the strongest form: the SAME (role, action,
    // kind) triples that the grant table allows in-tenant are ScopeDenied
    // cross-tenant — breadth comes from rules, never from a bypass.
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    for (role, action, kind) in [
        (Role::TenantAdmin, Action::CommandApprove, ScopeKind::Asset),
        (Role::TenantAdmin, Action::CommandApprove, ScopeKind::Site),
        (Role::Operator, Action::CommandCreate, ScopeKind::Asset),
        (Role::TenantAdmin, Action::CommandValidate, ScopeKind::Tenant),
        (Role::Viewer, Action::CommandRead, ScopeKind::Site),
    ] {
        let in_tenant = set.evaluate(role, action, Some(&scope_of(kind, tenant_a())), Some(&ctx));
        assert!(in_tenant.is_allowed(), "control: in-tenant must allow");
        let cross = set.evaluate(role, action, Some(&scope_of(kind, tenant_b())), Some(&ctx));
        assert_eq!(cross.deny_reason(), Some(DenyReason::ScopeDenied));
        assert_eq!(cross.evidence().as_str(), EVIDENCE_SCOPE_DENIED);
    }
}

#[test]
fn cross_tenant_denial_is_set_independent_empty_set_denies_the_same() {
    // The denial is structural (tenant match before rules): the EMPTY set
    // produces the identical ScopeDenied verdict for every cross-tenant
    // combination — no policy configuration can widen tenancy.
    let set = PolicySet::new();
    let ctx = context_for(tenant_a());
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            for kind in ALL_SCOPE_KINDS {
                let decision =
                    set.evaluate(role, action, Some(&scope_of(kind, tenant_b())), Some(&ctx));
                assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied));
                assert_eq!(decision.evidence().as_str(), EVIDENCE_SCOPE_DENIED);
            }
        }
    }
}

// --- 3. The deny-by-default proof ----------------------------------------------------

#[test]
fn the_empty_policy_set_denies_every_in_tenant_combination() {
    // Deny-by-default: with no grants, EVERY well-formed in-tenant request
    // denies with NoMatchingRule. 4 roles × 29 actions × 3 kinds = 348.
    let set = PolicySet::new();
    let ctx = context_for(tenant_a());
    let mut denies = 0usize;
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            for kind in ALL_SCOPE_KINDS {
                let decision =
                    set.evaluate(role, action, Some(&scope_of(kind, tenant_a())), Some(&ctx));
                assert!(!decision.is_allowed());
                assert_eq!(decision.deny_reason(), Some(DenyReason::NoMatchingRule));
                assert_eq!(decision.evidence().as_str(), EVIDENCE_NO_MATCHING_RULE);
                denies += 1;
            }
        }
    }
    assert_eq!(denies, 348);
}

#[test]
fn missing_scope_and_missing_context_deny_with_their_named_reasons() {
    // No default scope exists (UnknownScope) and evaluation runs against a
    // claims-derived context (MissingContext) — both deny even for a role
    // with a real grant on a granted action.
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    let no_scope = set.evaluate(
        Role::TenantAdmin,
        Action::CommandApprove,
        None,
        Some(&ctx),
    );
    assert_eq!(no_scope.deny_reason(), Some(DenyReason::UnknownScope));
    assert_eq!(no_scope.evidence().as_str(), EVIDENCE_UNKNOWN_SCOPE);
    let no_context = set.evaluate(
        Role::TenantAdmin,
        Action::CommandApprove,
        Some(&scope_of(ScopeKind::Asset, tenant_a())),
        None,
    );
    assert_eq!(no_context.deny_reason(), Some(DenyReason::MissingContext));
    assert_eq!(no_context.evidence().as_str(), EVIDENCE_MISSING_CONTEXT);
}

// --- 4. The never-granted set refuses (§5.7) ------------------------------------------

#[test]
fn dispatch_authorize_and_the_other_never_granted_ids_are_not_on_the_surface() {
    // The §5.7 capability names and every write/execute id that must never
    // exist parse to NOTHING: `Action::parse` is the closed gate.
    let never_granted = [
        "dispatch.authorize",
        "dispatch.issue",
        "secret.read",
        "network.any",
        "command.issue",
        "command.auto_approve",
        "commands.approve",
        "alarm.write",
        "alarm.acknowledge",
        "audit.event.write",
        "audit.event.delete",
        "audit.event.export",
        "telemetry.write",
        "asset.execute",
        "",
        "Asset.Read",
        "asset_read",
        "asset.purge",
    ];
    for id in never_granted {
        assert_eq!(
            Action::parse(id),
            None,
            "{id:?} must not parse — it can never be granted"
        );
    }
}

#[test]
fn evaluate_action_id_denies_every_never_granted_id_for_every_role() {
    // An untyped caller (an API path segment) asking for a never-granted
    // id gets a reasoned `UnknownAction` deny for EVERY role, with full
    // scope and context supplied — grants cannot smuggle an unlisted id
    // through.
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    let never_granted = [
        "dispatch.authorize",
        "dispatch.issue",
        "secret.read",
        "network.any",
        "command.issue",
        "command.auto_approve",
        "alarm.write",
        "audit.event.write",
        "telemetry.write",
        "",
    ];
    for role in ALL_ROLES {
        for id in never_granted {
            let decision = set.evaluate_action_id(
                role,
                id,
                Some(&scope_of(ScopeKind::Asset, tenant_a())),
                Some(&ctx),
            );
            assert!(!decision.is_allowed(), "role={} id={id:?}", role.id());
            assert_eq!(decision.deny_reason(), Some(DenyReason::UnknownAction));
            assert_eq!(decision.evidence().as_str(), EVIDENCE_UNKNOWN_ACTION);
        }
    }
}

// --- 5. Role boundaries called out from the grant table ------------------------------

#[test]
fn command_approve_is_tenant_admin_only_and_never_tenant_scoped() {
    // The separation-of-duties default (§17.3 control 2; the grant table's
    // `command.approve` row): TenantAdmin at site/asset ONLY.
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    // The approver role at the granted kinds: allow, with the recorded
    // evidence format.
    for kind in [ScopeKind::Site, ScopeKind::Asset] {
        let decision =
            set.evaluate(Role::TenantAdmin, Action::CommandApprove, Some(&scope_of(kind, tenant_a())), Some(&ctx));
        assert!(decision.is_allowed(), "kind={}", kind.id());
        assert_eq!(
            decision.evidence().as_str(),
            format!("{PHASE4_PREFIX}:tenant-admin:command.approve:{}", kind.id())
        );
    }
    // The tenant-level approval grant does NOT exist (approvals are
    // resource/site/asset-scoped).
    let at_tenant =
        set.evaluate(Role::TenantAdmin, Action::CommandApprove, Some(&scope_of(ScopeKind::Tenant, tenant_a())), Some(&ctx));
    assert_eq!(at_tenant.deny_reason(), Some(DenyReason::NoMatchingRule));
    // No other role can approve at ANY kind — including PlatformAdmin
    // (breadth comes from rules, never from a bypass).
    for role in [Role::PlatformAdmin, Role::Operator, Role::Viewer] {
        for kind in ALL_SCOPE_KINDS {
            let decision =
                set.evaluate(role, Action::CommandApprove, Some(&scope_of(kind, tenant_a())), Some(&ctx));
            assert!(!decision.is_allowed(), "role={} kind={}", role.id(), kind.id());
            assert_eq!(decision.deny_reason(), Some(DenyReason::NoMatchingRule));
        }
    }
}

#[test]
fn scope_levels_do_not_inherit() {
    // A grant at one kind matches only that kind: TenantAdmin's
    // command.validate at tenant kind does not surface from an asset-scope
    // request... and vice versa. Pinned directly (the 348 matrix proves it
    // implicitly; this is the called-out case).
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    // approve is granted at site and asset but NOT tenant: the tenant-kind
    // request denies even though the same role/action allows at site.
    let site = set.evaluate(
        Role::TenantAdmin,
        Action::CommandApprove,
        Some(&scope_of(ScopeKind::Site, tenant_a())),
        Some(&ctx),
    );
    assert!(site.is_allowed());
    let tenant = set.evaluate(
        Role::TenantAdmin,
        Action::CommandApprove,
        Some(&scope_of(ScopeKind::Tenant, tenant_a())),
        Some(&ctx),
    );
    assert_eq!(tenant.deny_reason(), Some(DenyReason::NoMatchingRule));
}

// --- 6. Auditability of every decision ------------------------------------------------

#[test]
fn every_denial_in_the_matrix_carries_a_stable_named_reason() {
    // §17.3 control 12 / P0.16 rule 6: denials are auditable values, never
    // bare bools — reason ids are the stable audit keys.
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            for kind in ALL_SCOPE_KINDS {
                let decision =
                    set.evaluate(role, action, Some(&scope_of(kind, tenant_b())), Some(&ctx));
                let reason = decision.deny_reason().expect("deny carries a reason");
                assert!(!reason.id().is_empty());
                assert!(!decision.evidence().as_str().is_empty());
                assert!(decision.allow_reason().is_none());
            }
        }
    }
    // The reason ids are the recorded stable strings.
    assert_eq!(DenyReason::ScopeDenied.id(), "scope-denied");
    assert_eq!(DenyReason::NoMatchingRule.id(), "no-matching-rule");
    assert_eq!(DenyReason::UnknownAction.id(), "unknown-action");
    assert_eq!(DenyReason::UnknownScope.id(), "unknown-scope");
    assert_eq!(DenyReason::MissingContext.id(), "missing-context");
}

#[test]
fn evidence_refs_are_explicitly_constructed_values() {
    // EvidenceRef is a named newtype with no Default — a decision without
    // evidence cannot be built (compile-level; pinned here as the runtime
    // half: constructed refs carry their string).
    let reference = EvidenceRef::new(EVIDENCE_SCOPE_DENIED);
    assert_eq!(reference.as_str(), "authz:deny:scope-denied");
}
