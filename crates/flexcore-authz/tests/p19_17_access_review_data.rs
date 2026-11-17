// P19.17 — the ACCESS-REVIEW DATA suite (integration level), WRITTEN-UNRUN
// (conventions §1 item 3): never compiled or executed — execution is the
// human build/test gate.
//
// What this file is, honestly:
//
// The P19.11 access-review process (`docs/phases-11-20/phase-19-security-
// certification-compliance/records/22_p19_11_access_review_process.md`)
// is IdP-pending (runtime IdP UNDEPLOYED; P13-E-1 OPEN; mechanics
// PENDING-P13-RUNTIME). The DATA the review's grant-reconciliation step
// (M-2) would reconcile against, however, already has a written surface:
// the P13.05 additive role model (`crates/flexcore-authz`, decision
// record `docs/phases-11-20/phase-13-enterprise-identity-integration/
// records/05_p13_05_role_model.md`) and the recorded grant sets
// (`phase4_command_policies`, `phase13_policies`, `phase16_market_policies`).
// This suite characterizes that surface BEHAVIORALLY over the public
// `PolicySet::evaluate` / `evaluate_action_id` entries — the same
// evaluation provenance as the P1.23/P13.19 suites (contexts built ONLY
// through the domain's public claims path; a bypass is a compile error).
//
// It is the grant-surface inventory an access review consumes: which role
// can do what, at which scope kind, citing which rule reference — pinned
// exactly as the recorded tables state, including the deliberate
// ABSENCES (SiteApprover's zero administrative rights, LR-049;
// apikey.* restricted to tenant-scope administration; the market surface
// with zero dispatch reach; the never-granted set structurally
// unreachable for every principal class).
//
// What this file is NOT: no review has run, no principal exists, no
// grant was issued or revoked, and no attestation is claimed. There is
// NO review-evidence API in this workspace (the P19.11 §5 evidence
// template is EMPTY by design) — this file therefore tests the
// authorization surface data ONLY and references no nonexistent review
// machinery (P19.17 hard-ban 8). The never-granted set's single home is
// `flexcore_kernel::manifest::NEVER_GRANTED_PERMISSIONS` (the authz
// crate cannot depend on the kernel; the four ids are asserted here as
// the deny outcomes their structural absence produces, mirroring the
// test-local restatement recorded in `phase16_market_policies`).

use flexcore_authz::action::Action;
use flexcore_authz::decision::{AuthorizationDecision, DenyReason};
use flexcore_authz::phase13_policies::{PHASE13_POLICY_PREFIX, phase13_policies};
use flexcore_authz::phase16_market_policies::{PHASE16_MARKET_POLICY_PREFIX, phase16_market_policies};
use flexcore_authz::phase4_policies::{PHASE4_POLICY_PREFIX, phase4_command_policies};
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_authz::scope::{AssetScope, Scope, ScopeKind, SiteScope, TenantScope};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{AssetId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};

// --- Fixtures -----------------------------------------------------------------

const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a04";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant_a() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_A))
}

fn tenant_b() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_B))
}

/// The single public claims path (P0.16 rule 5) — the same construction
/// the API middleware uses; review data never constructs a context any
/// other way.
fn context_for(tenant: TenantId, subject: &str) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant,
            subject: subject.to_string(),
        },
    ))
}

fn scopes_for_tenant(tenant: TenantId) -> [Scope; 3] {
    [
        Scope::Tenant(TenantScope::new(tenant)),
        Scope::Site(SiteScope::new(tenant, SiteId::new(uuid(UUID_SITE)))),
        Scope::Asset(AssetScope::new(tenant, AssetId::new(uuid(UUID_ASSET)))),
    ]
}

/// The five P13.05 roles — the review's human-principal vocabulary (flat,
/// no hierarchy, no implication).
const ALL_ROLES: [Role; 5] = [
    Role::PlatformAdmin,
    Role::TenantAdmin,
    Role::Operator,
    Role::Viewer,
    Role::SiteApprover,
];

/// The closed action surface the review reconciles against. Every variant
/// of `Action`, listed explicitly (a new variant must be ADDED here by a
/// reviewed change — the closed-enum extension rule).
const ALL_ACTIONS: [Action; 39] = [
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
    Action::ApiKeyCreate,
    Action::ApiKeyRotate,
    Action::ApiKeyRevoke,
    Action::ApiKeyRead,
    Action::MarketPriceRead,
    Action::MarketBidCompose,
    Action::MarketGrantIssue,
    Action::MarketGrantRevoke,
    Action::MarketGrantRead,
    Action::MarketSettlementRead,
];

fn evaluate(
    policies: &PolicySet,
    role: Role,
    action: Action,
    scope: &Scope,
    ctx: &TenantContext,
) -> AuthorizationDecision {
    policies.evaluate(role, action, Some(scope), Some(ctx))
}

fn assert_denied_with(decision: &AuthorizationDecision, expected: DenyReason, what: &str) {
    assert!(
        !decision.is_allowed(),
        "expected a DENY ({expected:?}) for {what}; got an allow — the recorded review data broke"
    );
    assert_eq!(
        decision.deny_reason(),
        Some(expected),
        "every denial carries its named reason (§10.1) — {what}"
    );
}

fn assert_allowed_citing(decision: &AuthorizationDecision, prefix: &str, what: &str) {
    assert!(
        decision.is_allowed(),
        "expected an ALLOW for {what}; got: {:?}",
        decision.deny_reason()
    );
    let AuthorizationDecision::Allow(_, evidence) = decision else {
        unreachable!("is_allowed() was true")
    };
    assert!(
        evidence.as_str().starts_with(prefix),
        "the allow for {what} must cite a {prefix} rule reference (auditable allow evidence per §17.3); got: {}",
        evidence.as_str()
    );
}

// --- The P4 grant set: the command surface, exactly as recorded --------------------

#[test]
fn phase4_operator_and_tenant_admin_create_validate_and_read_at_every_scope_kind() {
    let policies = phase4_command_policies();
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    for scope in &scopes_for_tenant(tenant_a()) {
        for action in [Action::CommandCreate, Action::CommandValidate, Action::CommandRead] {
            let what = format!("{} at {}", action.id(), scope.kind().id());
            assert_allowed_citing(
                &evaluate(&policies, Role::Operator, action, scope, &ctx),
                PHASE4_POLICY_PREFIX,
                &format!("operator {what}"),
            );
            assert_allowed_citing(
                &evaluate(&policies, Role::TenantAdmin, action, scope, &ctx),
                PHASE4_POLICY_PREFIX,
                &format!("tenant-admin {what}"),
            );
        }
        assert_allowed_citing(
            &evaluate(&policies, Role::Operator, Action::FeasibilityRead, scope, &ctx),
            PHASE4_POLICY_PREFIX,
            "operator feasibility.read",
        );
    }
}

#[test]
fn phase4_viewer_reads_but_never_creates_validates_or_approves() {
    let policies = phase4_command_policies();
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    for scope in &scopes_for_tenant(tenant_a()) {
        for action in [Action::CommandRead, Action::FeasibilityRead] {
            assert_allowed_citing(
                &evaluate(&policies, Role::Viewer, action, scope, &ctx),
                PHASE4_POLICY_PREFIX,
                &format!("viewer {} at {}", action.id(), scope.kind().id()),
            );
        }
        for action in [Action::CommandCreate, Action::CommandValidate, Action::CommandApprove] {
            assert_denied_with(
                &evaluate(&policies, Role::Viewer, action, scope, &ctx),
                DenyReason::NoMatchingRule,
                &format!("viewer {} at {}", action.id(), scope.kind().id()),
            );
        }
    }
}

#[test]
fn phase4_command_approve_is_site_and_asset_scoped_for_tenant_admin_only() {
    // The separation-of-duties row: approval at resource scope, NO
    // tenant-level approval grant for ANYONE (the review's finding rule:
    // a tenant-wide approver is a discrepancy by definition).
    let policies = phase4_command_policies();
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    let scopes = scopes_for_tenant(tenant_a());
    assert_allowed_citing(
        &evaluate(&policies, Role::TenantAdmin, Action::CommandApprove, &scopes[1], &ctx),
        PHASE4_POLICY_PREFIX,
        "tenant-admin command.approve at site scope",
    );
    assert_allowed_citing(
        &evaluate(&policies, Role::TenantAdmin, Action::CommandApprove, &scopes[2], &ctx),
        PHASE4_POLICY_PREFIX,
        "tenant-admin command.approve at asset scope",
    );
    assert_denied_with(
        &evaluate(&policies, Role::TenantAdmin, Action::CommandApprove, &scopes[0], &ctx),
        DenyReason::NoMatchingRule,
        "tenant-admin command.approve at TENANT scope",
    );
    for role in ALL_ROLES {
        if role == Role::TenantAdmin {
            continue;
        }
        for scope in &scopes {
            assert_denied_with(
                &evaluate(&policies, role, Action::CommandApprove, scope, &ctx),
                DenyReason::NoMatchingRule,
                &format!("{} command.approve at {}", role.id(), scope.kind().id()),
            );
        }
    }
}

#[test]
fn phase4_platform_admin_and_site_approver_hold_nothing() {
    // PlatformAdmin's breadth comes from OTHER grant sets (registry /
    // configuration), never from the command surface. SiteApprover (the
    // P13.05 role) holds NOTHING in the Phase 4 set — its approval duty is
    // recorded in the Phase 13 set, which this probe confirms is the ONLY
    // place it lives.
    let policies = phase4_command_policies();
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    for role in [Role::PlatformAdmin, Role::SiteApprover] {
        for action in ALL_ACTIONS {
            for scope in &scopes_for_tenant(tenant_a()) {
                assert_denied_with(
                    &evaluate(&policies, role, action, scope, &ctx),
                    DenyReason::NoMatchingRule,
                    &format!("{} {} at {} (phase-4 set)", role.id(), action.id(), scope.kind().id()),
                );
            }
        }
    }
}

// --- The P13 grant set: the approval-duty role and the apikey surface --------------

#[test]
fn phase13_site_approver_holds_exactly_the_approval_duty_at_resource_scope() {
    // The LR-049 separation, pinned as REVIEW DATA: SiteApprover allows
    // `command.approve` at site and asset scope within its tenant — and
    // NOTHING ELSE on the whole action surface, at any scope kind, in any
    // phase-13 grant: no registry write, no delete, no API-key
    // administration, no configuration change.
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:approver-1");
    let scopes = scopes_for_tenant(tenant_a());
    for scope in &scopes[1..] {
        assert_allowed_citing(
            &evaluate(&policies, Role::SiteApprover, Action::CommandApprove, scope, &ctx),
            PHASE13_POLICY_PREFIX,
            &format!("site-approver command.approve at {}", scope.kind().id()),
        );
    }
    // No tenant-level approval grant (the Phase 4 rule stands).
    assert_denied_with(
        &evaluate(&policies, Role::SiteApprover, Action::CommandApprove, &scopes[0], &ctx),
        DenyReason::NoMatchingRule,
        "site-approver command.approve at TENANT scope",
    );
    // Nothing else, anywhere on the surface.
    for action in ALL_ACTIONS {
        if action == Action::CommandApprove {
            continue;
        }
        for scope in &scopes {
            assert_denied_with(
                &evaluate(&policies, Role::SiteApprover, action, scope, &ctx),
                DenyReason::NoMatchingRule,
                &format!("site-approver {} at {} (phase-13 set)", action.id(), scope.kind().id()),
            );
        }
    }
}

#[test]
fn phase13_apikey_administration_is_admin_roles_at_tenant_scope_only() {
    // The P13.08 surface: apikey.* for PlatformAdmin and TenantAdmin at
    // TENANT scope only. The review reconciles machine credentials against
    // exactly this shape — an API key administered at site/asset scope, or
    // by an operational/read-only principal, is a discrepancy by definition.
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:admin-1");
    let apikey_actions = [
        Action::ApiKeyCreate,
        Action::ApiKeyRotate,
        Action::ApiKeyRevoke,
        Action::ApiKeyRead,
    ];
    let scopes = scopes_for_tenant(tenant_a());
    for action in apikey_actions {
        assert_allowed_citing(
            &evaluate(&policies, Role::PlatformAdmin, action, &scopes[0], &ctx),
            PHASE13_POLICY_PREFIX,
            &format!("platform-admin {} at tenant scope", action.id()),
        );
        assert_allowed_citing(
            &evaluate(&policies, Role::TenantAdmin, action, &scopes[0], &ctx),
            PHASE13_POLICY_PREFIX,
            &format!("tenant-admin {} at tenant scope", action.id()),
        );
        // Not at site or asset scope — not even for the granting roles.
        for scope in &scopes[1..] {
            assert_denied_with(
                &evaluate(&policies, Role::PlatformAdmin, action, scope, &ctx),
                DenyReason::NoMatchingRule,
                &format!("platform-admin {} at {}", action.id(), scope.kind().id()),
            );
            assert_denied_with(
                &evaluate(&policies, Role::TenantAdmin, action, scope, &ctx),
                DenyReason::NoMatchingRule,
                &format!("tenant-admin {} at {}", action.id(), scope.kind().id()),
            );
        }
        // Not for the non-administrative principal classes, at any scope.
        for role in [Role::Operator, Role::Viewer, Role::SiteApprover] {
            for scope in &scopes {
                assert_denied_with(
                    &evaluate(&policies, role, action, scope, &ctx),
                    DenyReason::NoMatchingRule,
                    &format!("{} {} at {}", role.id(), action.id(), scope.kind().id()),
                );
            }
        }
    }
}

#[test]
fn phase13_set_grants_nothing_else_to_any_role() {
    // The phase-13 set is EXACTLY the two recorded rows: SiteApprover
    // command.approve (site/asset) and apikey.* (tenant, admin roles).
    // Every other (role, action, scope-kind) triple denies — the review
    // can cite this file for the set's full extent.
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            // Skip the two recorded rows; they are pinned by their own tests.
            let is_recorded_row = (action == Action::CommandApprove
                && role == Role::SiteApprover)
                || (matches!(
                    action,
                    Action::ApiKeyCreate | Action::ApiKeyRotate | Action::ApiKeyRevoke | Action::ApiKeyRead
                ) && matches!(role, Role::PlatformAdmin | Role::TenantAdmin));
            if is_recorded_row {
                continue;
            }
            for scope in &scopes_for_tenant(tenant_a()) {
                assert_denied_with(
                    &evaluate(&policies, role, action, scope, &ctx),
                    DenyReason::NoMatchingRule,
                    &format!("{} {} at {} (phase-13 full-extent)", role.id(), action.id(), scope.kind().id()),
                );
            }
        }
    }
}

// --- The P16 grant set: the market surface with zero dispatch reach ----------------

#[test]
fn phase16_market_record_reads_are_visible_to_all_four_data_roles_at_tenant_scope() {
    let policies = phase16_market_policies();
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    let scopes = scopes_for_tenant(tenant_a());
    for action in [Action::MarketPriceRead, Action::MarketSettlementRead] {
        for role in [Role::PlatformAdmin, Role::TenantAdmin, Role::Operator, Role::Viewer] {
            assert_allowed_citing(
                &evaluate(&policies, role, action, &scopes[0], &ctx),
                PHASE16_MARKET_POLICY_PREFIX,
                &format!("{} {} at tenant scope", role.id(), action.id()),
            );
        }
        // Tenant scope ONLY — no site/asset market reads (the recorded
        // table has no such row; an absence is the deny-by-default posture).
        for role in ALL_ROLES {
            for scope in &scopes[1..] {
                assert_denied_with(
                    &evaluate(&policies, role, action, scope, &ctx),
                    DenyReason::NoMatchingRule,
                    &format!("{} {} at {}", role.id(), action.id(), scope.kind().id()),
                );
            }
        }
    }
}

#[test]
fn phase16_bid_composition_is_operator_and_tenant_admin_only() {
    // Bid composition is operational work (the composer's own gates —
    // operator-approved mode plus an ACTIVE per-market grant record —
    // apply on every call and are NOT modeled on this RBAC surface).
    // PlatformAdmin keeps administration, not composition duty; Viewer and
    // SiteApprover get none.
    let policies = phase16_market_policies();
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    let scopes = scopes_for_tenant(tenant_a());
    assert_allowed_citing(
        &evaluate(&policies, Role::Operator, Action::MarketBidCompose, &scopes[0], &ctx),
        PHASE16_MARKET_POLICY_PREFIX,
        "operator market.bid.compose at tenant scope",
    );
    assert_allowed_citing(
        &evaluate(&policies, Role::TenantAdmin, Action::MarketBidCompose, &scopes[0], &ctx),
        PHASE16_MARKET_POLICY_PREFIX,
        "tenant-admin market.bid.compose at tenant scope",
    );
    for role in [Role::PlatformAdmin, Role::Viewer, Role::SiteApprover] {
        for scope in &scopes {
            assert_denied_with(
                &evaluate(&policies, role, Action::MarketBidCompose, scope, &ctx),
                DenyReason::NoMatchingRule,
                &format!("{} market.bid.compose at {}", role.id(), scope.kind().id()),
            );
        }
    }
}

#[test]
fn phase16_grant_administration_is_admin_roles_at_tenant_scope_only() {
    let policies = phase16_market_policies();
    let ctx = context_for(tenant_a(), "user:admin-1");
    let scopes = scopes_for_tenant(tenant_a());
    for action in [Action::MarketGrantIssue, Action::MarketGrantRevoke, Action::MarketGrantRead] {
        for role in [Role::PlatformAdmin, Role::TenantAdmin] {
            assert_allowed_citing(
                &evaluate(&policies, role, action, &scopes[0], &ctx),
                PHASE16_MARKET_POLICY_PREFIX,
                &format!("{} {} at tenant scope", role.id(), action.id()),
            );
        }
        for role in [Role::Operator, Role::Viewer, Role::SiteApprover] {
            for scope in &scopes {
                assert_denied_with(
                    &evaluate(&policies, role, action, scope, &ctx),
                    DenyReason::NoMatchingRule,
                    &format!("{} {} at {}", role.id(), action.id(), scope.kind().id()),
                );
            }
        }
    }
}

#[test]
fn phase16_site_approver_holds_no_market_authority_anywhere() {
    // The P16.19 posture the review scope carries: the approval-duty role
    // carries NO market authority — an approval duty never widened into a
    // market grant.
    let policies = phase16_market_policies();
    let ctx = context_for(tenant_a(), "user:approver-1");
    for action in ALL_ACTIONS {
        for scope in &scopes_for_tenant(tenant_a()) {
            assert_denied_with(
                &evaluate(&policies, Role::SiteApprover, action, scope, &ctx),
                DenyReason::NoMatchingRule,
                &format!("site-approver {} at {} (phase-16 set)", action.id(), scope.kind().id()),
            );
        }
    }
}

// --- The never-granted set: structurally unreachable, for every principal ----------

#[test]
fn the_never_granted_ids_do_not_parse_for_any_role_in_any_recorded_set() {
    // `dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`:
    // none exists on the `Action` surface, none is grantable by any
    // principal class (human, service account, API key, machine identity —
    // they all evaluate as one of the five roles on this surface). The
    // string-evaluation entry denies with UnknownAction — the structural
    // refusal, asserted across every recorded grant set.
    let never_granted = ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"];
    let sets = [
        ("phase4", phase4_command_policies()),
        ("phase13", phase13_policies()),
        ("phase16", phase16_market_policies()),
    ];
    for role in ALL_ROLES {
        let ctx = context_for(tenant_a(), &format!("probe:{}", role.id()));
        for scope in &scopes_for_tenant(tenant_a()) {
            for (set_name, policies) in &sets {
                for id in never_granted {
                    let decision =
                        policies.evaluate_action_id(role, id, Some(scope), Some(&ctx));
                    assert_denied_with(
                        &decision,
                        DenyReason::UnknownAction,
                        &format!("{id} as {} at {} ({set_name} set)", role.id(), scope.kind().id()),
                    );
                }
            }
        }
    }
}

// --- Tenant isolation and the fail-closed evaluation entries ------------------------

#[test]
fn cross_tenant_scopes_deny_even_for_platform_admin() {
    // No role, including PlatformAdmin, bypasses the tenant match — breadth
    // comes from rule grants, never from a bypass (the recorded modelling
    // note). The review treats a cross-tenant allow as an impossible value.
    let policies = phase13_policies();
    let ctx_a = context_for(tenant_a(), "user:admin-1");
    let foreign_scopes = scopes_for_tenant(tenant_b());
    for action in [Action::CommandApprove, Action::ApiKeyCreate, Action::ApiKeyRead] {
        for scope in &foreign_scopes {
            assert_denied_with(
                &evaluate(&policies, Role::PlatformAdmin, action, scope, &ctx_a),
                DenyReason::ScopeDenied,
                &format!("platform-admin {} on tenant-B scope from tenant-A context", action.id()),
            );
            assert_denied_with(
                &evaluate(&policies, Role::TenantAdmin, action, scope, &ctx_a),
                DenyReason::ScopeDenied,
                &format!("tenant-admin {} on tenant-B scope from tenant-A context", action.id()),
            );
        }
    }
}

#[test]
fn missing_scope_and_missing_context_deny_with_their_named_reasons() {
    // There is no default scope and no default context: the review data
    // cannot be evaluated "unscoped" into an allow. A granted triple that
    // arrives without its scope or context STILL denies — auditable, never
    // a panic or an invented value.
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:approver-1");
    let scope = Scope::Site(SiteScope::new(tenant_a(), SiteId::new(uuid(UUID_SITE))));

    let no_scope = policies.evaluate(Role::SiteApprover, Action::CommandApprove, None, Some(&ctx));
    assert_denied_with(&no_scope, DenyReason::UnknownScope, "a granted triple with no scope");

    let no_context =
        policies.evaluate(Role::SiteApprover, Action::CommandApprove, Some(&scope), None);
    assert_denied_with(&no_context, DenyReason::MissingContext, "a granted triple with no context");

    let neither = policies.evaluate(Role::SiteApprover, Action::CommandApprove, None, None);
    assert_denied_with(&neither, DenyReason::UnknownScope, "a granted triple with neither");
}

#[test]
fn an_unknown_action_id_denies_before_any_rule_is_consulted() {
    // Anything outside the known surface — the review's catch-all: an id
    // that does not parse is denied regardless of scope, context, or rules.
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    let scope = Scope::Tenant(TenantScope::new(tenant_a()));
    for id in ["dispatch.authorize", "market.dispatch.issue", "totally.unknown", ""] {
        assert_denied_with(
            &policies.evaluate_action_id(Role::PlatformAdmin, id, Some(&scope), Some(&ctx)),
            DenyReason::UnknownAction,
            &format!("the unknown id '{id}'"),
        );
    }
}

#[test]
fn every_allow_in_every_recorded_set_cites_its_phase_rule_reference() {
    // The access-review evidence trail: each allow's `EvidenceRef` names
    // the recorded rule (policy:phase4 / policy:phase13 /
    // policy:phase16-market), so a reconciliation can point at the row it
    // matched. A prefix-less allow would be unreviewable.
    let ctx = context_for(tenant_a(), "user:review-probe-1");
    let scopes = scopes_for_tenant(tenant_a());

    let phase4 = phase4_command_policies();
    assert_allowed_citing(
        &evaluate(&phase4, Role::Operator, Action::CommandCreate, &scopes[0], &ctx),
        PHASE4_POLICY_PREFIX,
        "phase-4 sample allow",
    );

    let phase13 = phase13_policies();
    assert_allowed_citing(
        &evaluate(&phase13, Role::SiteApprover, Action::CommandApprove, &scopes[1], &ctx),
        PHASE13_POLICY_PREFIX,
        "phase-13 sample allow",
    );

    let phase16 = phase16_market_policies();
    assert_allowed_citing(
        &evaluate(&phase16, Role::Operator, Action::MarketPriceRead, &scopes[0], &ctx),
        PHASE16_MARKET_POLICY_PREFIX,
        "phase-16 sample allow",
    );
}
