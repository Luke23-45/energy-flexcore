// P1.11 — Roles, the allow-only `PolicySet`, and deny-by-default evaluation
//         against the domain `TenantContext`.
//
// Ledger P1.11 steps 4–5: evaluation runs against the domain tenant context
// (P0.16 rule 5) and every path that is not an explicit grant denies, with a
// reason (unknown action, unknown scope, missing context, cross-tenant
// scope, no matching rule).
//
// Evaluation order (fixed, and tested):
//
// 1. the action id must be on the known surface → else
//    `DenyReason::UnknownAction` (in `evaluate_action_id`, the entry for
//    untyped callers);
// 2. the request must name an explicit scope → else `DenyReason::UnknownScope`
//    (there is no default scope);
// 3. a tenant context must be supplied → else `DenyReason::MissingContext`;
// 4. the scope's tenant must equal the context's tenant → else
//    `DenyReason::ScopeDenied` (audited per P0.16 rule 6);
// 5. an allow rule must match (role, action, scope-kind) exactly → else
//    `DenyReason::NoMatchingRule`; a match allows with the **rule's
//    reference** as evidence.
//
// Steps 2–5 live in the private `decide` core; the public `evaluate` is the
// claims-bound entry that projects the domain `TenantContext` onto its
// tenant id (read-only) and forwards. See the test-module note on why the
// core is exercised directly by the tests.

use crate::action::Action;
use crate::decision::{AllowReason, AuthorizationDecision, DenyReason, EvidenceRef};
use crate::scope::{Scope, ScopeKind};
use flexcore_domain::tenant_context::TenantContext;

/// Canonical deny evidence references, stable for audit correlation.
const EVIDENCE_UNKNOWN_ACTION: &str = "authz:deny:unknown-action";
const EVIDENCE_UNKNOWN_SCOPE: &str = "authz:deny:unknown-scope";
const EVIDENCE_MISSING_CONTEXT: &str = "authz:deny:missing-context";
const EVIDENCE_SCOPE_DENIED: &str = "authz:deny:scope-denied";
const EVIDENCE_NO_MATCHING_RULE: &str = "authz:deny:no-matching-rule";

/// A principal role on the Phase 1 RBAC surface (§17.3 control 2: RBAC plus
/// resource/site/asset-scoped authorization).
///
/// Roles are flat: no hierarchy, no implication. A grant to one role never
/// authorizes another role; every (role, action, scope-kind) triple needs
/// its own explicit grant. Role assignment/resolution happens in the
/// application layer from real authentication — never from a request field
/// (P0.16 rule 5); this crate consumes a role as an explicit evaluation
/// input and derives none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Platform administrator (tenant registry, platform configuration).
    PlatformAdmin,
    /// Administrator within one tenant.
    TenantAdmin,
    /// Operational user: reads and day-to-day operations within grants.
    Operator,
    /// Read-only user.
    Viewer,
}

impl Role {
    /// Stable lowercase id, suitable for audit records and rule references.
    pub fn id(self) -> &'static str {
        match self {
            Role::PlatformAdmin => "platform-admin",
            Role::TenantAdmin => "tenant-admin",
            Role::Operator => "operator",
            Role::Viewer => "viewer",
        }
    }
}

/// One explicit allow rule: this role may perform this action at this scope
/// level, referenced by a stable rule reference (the allow evidence).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PolicyRule {
    role: Role,
    action: Action,
    scope_kind: ScopeKind,
    reference: EvidenceRef,
}

/// A set of explicit allow rules with **deny-by-default** evaluation.
///
/// The empty set (the `Default`) denies everything. Capability exists only
/// where a rule was added via [`PolicySet::grant`]. Deny rules and
/// deny-beats-allow priority are **not modelled in Phase 1** — denial is
/// structural, and [`DenyReason::ExplicitDeny`] is reserved vocabulary (see
/// `decision`). The set is protected platform configuration: plugins never
/// supply or modify it (P0.17; report §5.7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicySet {
    rules: Vec<PolicyRule>,
}

impl PolicySet {
    /// An empty policy set: denies everything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Grants `role` the ability to perform `action` at scope level
    /// `scope_kind`, with a stable rule reference used as the allow
    /// evidence. No other role, action, or scope level is affected.
    pub fn grant(
        &mut self,
        role: Role,
        action: Action,
        scope_kind: ScopeKind,
        reference: EvidenceRef,
    ) -> &mut Self {
        self.rules.push(PolicyRule {
            role,
            action,
            scope_kind,
            reference,
        });
        self
    }

    /// Evaluates a request: `role` wants to perform `action` on `scope`,
    /// authenticated as the tenant in `context`.
    ///
    /// Both `scope` and `context` are explicit parameters with **no
    /// defaults**: a missing scope denies (`UnknownScope`) and a missing
    /// context denies (`MissingContext`). The `Option` exists so those
    /// missing cases produce a reasoned, auditable
    /// [`AuthorizationDecision`] instead of a panic or an invented value —
    /// never so a caller can "opt out" of scoping.
    pub fn evaluate(
        &self,
        role: Role,
        action: Action,
        scope: Option<&Scope>,
        context: Option<&TenantContext>,
    ) -> AuthorizationDecision {
        // Read-only projection of the claims-derived context onto the value
        // evaluation needs. The context itself is never constructed here —
        // its only constructor is the domain claims constructor (P0.16
        // rule 5); see the test-module note.
        self.decide(role, action, scope, context.map(TenantContext::tenant_id))
    }

    /// Evaluation entry for untyped callers: the action arrives as its
    /// canonical id (e.g. an API path segment). An id outside the known
    /// Phase 1 surface denies with [`DenyReason::UnknownAction`] —
    /// deny-by-default for anything the surface does not know, regardless
    /// of scope, context, or rules.
    pub fn evaluate_action_id(
        &self,
        role: Role,
        action_id: &str,
        scope: Option<&Scope>,
        context: Option<&TenantContext>,
    ) -> AuthorizationDecision {
        match Action::parse(action_id) {
            Some(action) => self.evaluate(role, action, scope, context),
            None => AuthorizationDecision::deny(
                DenyReason::UnknownAction,
                EvidenceRef::new(EVIDENCE_UNKNOWN_ACTION),
            ),
        }
    }

    /// The evaluation core, parameterised on the context's tenant id so the
    /// in-crate tests can exercise every branch. Private on purpose: the
    /// public surface only exposes `evaluate`/`evaluate_action_id`, which
    /// require the claims-derived domain `TenantContext`.
    fn decide(
        &self,
        role: Role,
        action: Action,
        scope: Option<&Scope>,
        context_tenant: Option<flexcore_domain::ids::TenantId>,
    ) -> AuthorizationDecision {
        // 2. An explicit scope is required; there is no default scope.
        let Some(scope) = scope else {
            return AuthorizationDecision::deny(
                DenyReason::UnknownScope,
                EvidenceRef::new(EVIDENCE_UNKNOWN_SCOPE),
            );
        };
        // 3. A tenant context is required (P0.16 rule 5).
        let Some(context_tenant) = context_tenant else {
            return AuthorizationDecision::deny(
                DenyReason::MissingContext,
                EvidenceRef::new(EVIDENCE_MISSING_CONTEXT),
            );
        };
        // 4. Tenant match: cross-tenant requests deny (P0.16 rule 6; the
        //    §17.4 cross-tenant-leakage threat row). No role bypasses this.
        if scope.tenant() != context_tenant {
            return AuthorizationDecision::deny(
                DenyReason::ScopeDenied,
                EvidenceRef::new(EVIDENCE_SCOPE_DENIED),
            );
        }
        // 5. Exact rule match — role, action, and scope-kind must all be
        //    granted; scope levels do not inherit. First match in grant
        //    order wins; anything else denies.
        for rule in &self.rules {
            if rule.role == role && rule.action == action && rule.scope_kind == scope.kind() {
                return AuthorizationDecision::allow(
                    AllowReason::PolicyMatch,
                    rule.reference.clone(),
                );
            }
        }
        AuthorizationDecision::deny(
            DenyReason::NoMatchingRule,
            EvidenceRef::new(EVIDENCE_NO_MATCHING_RULE),
        )
    }
}

#[cfg(test)]
mod tests {
    // `super::*` brings in the parent's imports (`Action`, `Role`,
    // `ScopeKind`, `Scope`, the decision types) per the usual in-crate test
    // pattern; the site/asset scope structs are named explicitly.
    use super::*;
    use crate::scope::{AssetScope, SiteScope};
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::{AssetId, SiteId, TenantId};

    const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8b";
    const UUID_OTHER_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";
    const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8d";
    const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8e";

    fn tenant() -> TenantId {
        TenantId::new(FlexUuid::parse(UUID_TENANT).unwrap())
    }

    fn other_tenant() -> TenantId {
        TenantId::new(FlexUuid::parse(UUID_OTHER_TENANT).unwrap())
    }

    fn asset_scope(t: TenantId) -> Scope {
        Scope::Asset(AssetScope::new(t, AssetId::new(FlexUuid::parse(UUID_ASSET).unwrap())))
    }

    fn site_scope(t: TenantId) -> Scope {
        Scope::Site(SiteScope::new(
            t,
            SiteId::new(FlexUuid::parse(UUID_SITE).unwrap()),
        ))
    }

    fn granted_set() -> PolicySet {
        let mut set = PolicySet::new();
        set.grant(
            Role::TenantAdmin,
            Action::AssetRead,
            ScopeKind::Asset,
            EvidenceRef::new("policy:tenant-admin:asset.read:asset"),
        );
        set
    }

    // --- Deny-by-default, one test per ledger P1.11 step 5 case ----------

    #[test]
    fn empty_policy_set_denies_everything() {
        // A well-formed, in-tenant request against the empty (default) set:
        // still denied — no rule, no capability.
        let decision = PolicySet::new().decide(
            Role::TenantAdmin,
            Action::AssetRead,
            Some(&asset_scope(tenant())),
            Some(tenant()),
        );
        assert_eq!(decision.deny_reason(), Some(DenyReason::NoMatchingRule));
        assert_eq!(decision.evidence().as_str(), EVIDENCE_NO_MATCHING_RULE);
    }

    #[test]
    fn unknown_action_denies() {
        // The §5.7 capability names are not on the Phase 1 surface; an
        // untyped caller asking for one gets a reasoned deny before scope,
        // context, or rules are even considered.
        let decision = PolicySet::new().evaluate_action_id(
            Role::PlatformAdmin,
            "dispatch.authorize",
            Some(&asset_scope(tenant())),
            None,
        );
        assert_eq!(decision.deny_reason(), Some(DenyReason::UnknownAction));
        assert_eq!(decision.evidence().as_str(), EVIDENCE_UNKNOWN_ACTION);
    }

    #[test]
    fn missing_scope_denies_no_default_scope_exists() {
        // Known action, no scope: denied regardless of session state —
        // there is no default scope anywhere in this crate.
        let decision =
            PolicySet::new().evaluate(Role::TenantAdmin, Action::AssetRead, None, None);
        assert_eq!(decision.deny_reason(), Some(DenyReason::UnknownScope));
        assert_eq!(decision.evidence().as_str(), EVIDENCE_UNKNOWN_SCOPE);
    }

    #[test]
    fn missing_context_denies() {
        // Explicit scope but no tenant context: denied — evaluation runs
        // against the claims-derived domain TenantContext (P0.16 rule 5),
        // and without one there is nothing to match tenancy against.
        let decision = PolicySet::new().evaluate(
            Role::TenantAdmin,
            Action::AssetRead,
            Some(&asset_scope(tenant())),
            None,
        );
        assert_eq!(decision.deny_reason(), Some(DenyReason::MissingContext));
        assert_eq!(decision.evidence().as_str(), EVIDENCE_MISSING_CONTEXT);
    }

    #[test]
    fn cross_tenant_scope_denies_and_is_auditable() {
        // The scope names another tenant than the context: denied even
        // though an identical in-tenant request would be allowed (same
        // role/action/kind as `allow_path_carries_the_rule_reference`).
        let decision = granted_set().decide(
            Role::TenantAdmin,
            Action::AssetRead,
            Some(&asset_scope(other_tenant())),
            Some(tenant()),
        );
        assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied));
        assert_eq!(decision.evidence().as_str(), EVIDENCE_SCOPE_DENIED);
    }

    #[test]
    fn platform_admin_gets_no_tenant_match_bypass() {
        // Breadth comes from rules, never from a bypass: PlatformAdmin with
        // a cross-tenant scope denies exactly like everyone else.
        let mut set = PolicySet::new();
        set.grant(
            Role::PlatformAdmin,
            Action::SiteRead,
            ScopeKind::Site,
            EvidenceRef::new("policy:platform-admin:site.read:site"),
        );
        let decision = set.decide(
            Role::PlatformAdmin,
            Action::SiteRead,
            Some(&site_scope(other_tenant())),
            Some(tenant()),
        );
        assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied));
    }

    // --- Allow path -------------------------------------------------------

    #[test]
    fn allow_path_carries_the_rule_reference_as_evidence() {
        let decision = granted_set().decide(
            Role::TenantAdmin,
            Action::AssetRead,
            Some(&asset_scope(tenant())),
            Some(tenant()),
        );
        assert!(decision.is_allowed());
        assert_eq!(decision.allow_reason(), Some(AllowReason::PolicyMatch));
        assert_eq!(decision.evidence().as_str(), "policy:tenant-admin:asset.read:asset");
    }

    #[test]
    fn grants_are_exact_in_role_action_and_scope_kind() {
        let mut set = PolicySet::new();
        set.grant(
            Role::TenantAdmin,
            Action::AssetRead,
            ScopeKind::Asset,
            EvidenceRef::new("policy:tenant-admin:asset.read:asset"),
        );
        // Same action at another scope kind: not covered — scope levels do
        // not inherit.
        let wrong_kind = set.decide(
            Role::TenantAdmin,
            Action::AssetRead,
            Some(&site_scope(tenant())),
            Some(tenant()),
        );
        assert_eq!(wrong_kind.deny_reason(), Some(DenyReason::NoMatchingRule));
        // Same action and kind for a role without the grant: denied.
        let wrong_role = set.decide(
            Role::Viewer,
            Action::AssetRead,
            Some(&asset_scope(tenant())),
            Some(tenant()),
        );
        assert_eq!(wrong_role.deny_reason(), Some(DenyReason::NoMatchingRule));
        // A different action for the granted role: denied.
        let wrong_action = set.decide(
            Role::TenantAdmin,
            Action::SiteRead,
            Some(&site_scope(tenant())),
            Some(tenant()),
        );
        assert_eq!(wrong_action.deny_reason(), Some(DenyReason::NoMatchingRule));
    }

    #[test]
    fn first_matching_rule_wins_deterministically() {
        let mut set = PolicySet::new();
        set.grant(
            Role::Operator,
            Action::AssetRead,
            ScopeKind::Asset,
            EvidenceRef::new("policy:first"),
        );
        set.grant(
            Role::Operator,
            Action::AssetRead,
            ScopeKind::Asset,
            EvidenceRef::new("policy:second"),
        );
        let decision = set.decide(
            Role::Operator,
            Action::AssetRead,
            Some(&asset_scope(tenant())),
            Some(tenant()),
        );
        assert_eq!(decision.evidence().as_str(), "policy:first");
    }

    #[test]
    fn public_evaluate_forwards_the_missing_context_case_to_the_core() {
        // The public entry with no context denies with MissingContext — the
        // same reasoning the core applies for every deny case (verified
        // branch-by-branch above).
        let decision = granted_set().evaluate(
            Role::TenantAdmin,
            Action::AssetRead,
            Some(&asset_scope(tenant())),
            None,
        );
        assert_eq!(decision.deny_reason(), Some(DenyReason::MissingContext));
    }

    // --- Tenant-context provenance (P0.16 rule 5) -------------------------
    //
    // `PolicySet::evaluate` takes `Option<&flexcore_domain::tenant_context::
    // TenantContext>`. That type is defined in flexcore-domain (P1.06); its
    // ONLY public constructor is
    // `TenantContext::from_authenticated_claims(AuthenticatedClaims)`, and
    // `AuthenticatedClaims` has no public constructor anywhere and private
    // fields. Constructing a tenant context from an untrusted string, a
    // bare `TenantId`, or a request field is therefore a **compile error**,
    // not a runtime check that can be forgotten. Mirroring the domain
    // crate's tenant-context tests, the attempted bypasses below are
    // compile-level facts — if uncommented, this crate would not compile:
    //
    //   // 1. Raw string tenant — no `From<String>` impl exists:
    //   let ctx = TenantContext::from("acme".to_string());
    //   // error[E0277]: the trait bound `TenantContext: From<String>` is
    //   // not satisfied
    //
    //   // 2. Bare tenant ID — no `From<TenantId>` impl exists:
    //   let ctx = TenantContext::from(tenant());
    //   // error[E0277]: the trait bound `TenantContext: From<TenantId>` is
    //   // not satisfied
    //
    //   // 3. Struct literal — fields are private to flexcore-domain:
    //   let ctx = TenantContext { tenant_id, subject };
    //   // error[E0451]: fields `tenant_id` and `subject` of struct
    //   // `TenantContext` are private
    //
    //   // 4. Fabricated claims — no public constructor, private fields:
    //   let c = AuthenticatedClaims { tenant_id, subject: "x".into() };
    //   // error[E0451] / error[E0433]: not constructible or nameable here
    //
    //   // 5. Defaulted context — no `Default` impl exists:
    //   let ctx = TenantContext::default();
    //   // error[E0277]: the trait bound `TenantContext: Default` is not
    //   // satisfied
    //
    // Consequence for these tests (compile-level fact, same approach as the
    // domain tests): this crate cannot fabricate a `TenantContext` — not
    // even under `#[cfg(test)]`. The positive claims→context construction
    // path is therefore proven where the fields are visible
    // (flexcore-domain/src/tenant_context.rs tests), and THIS crate proves
    // its own mechanics through the private `decide` core, which the public
    // `evaluate` forwards to with
    // `context.map(TenantContext::tenant_id)` — a read-only projection that
    // adds no construction path. `evaluate` itself is additionally pinned
    // for the reachable missing-context branch above. If a later subtask
    // adds the application-layer claims wiring (ledger P1.11 note in
    // flexcore-domain), an integration test in that layer can drive the
    // full public path end to end.

    #[test]
    fn decision_values_are_never_bare_bools() {
        // API-level companion to the decision module's negative test: the
        // evaluator's return type is `AuthorizationDecision`, and every
        // outcome above carries reason + evidence. `is_allowed` is used
        // here strictly as control flow; the full value is what a caller
        // must audit.
        let allow = granted_set().decide(
            Role::TenantAdmin,
            Action::AssetRead,
            Some(&asset_scope(tenant())),
            Some(tenant()),
        );
        if allow.is_allowed() {
            assert_eq!(allow.evidence().as_str(), "policy:tenant-admin:asset.read:asset");
        } else {
            panic!("granted request must allow");
        }
    }
}
