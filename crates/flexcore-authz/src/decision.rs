// P1.11 — Authorization decision values: allow/deny, always carrying a
//         reason and an evidence reference.
//
// Ledger P1.11 step 2: "Implement the authorization decision type:
// allow/deny (+ reason/evidence reference); decisions are explicit values,
// never booleans without a reason."
//
// The type system carries that rule, not a convention:
//
// - `PolicySet::evaluate` (see the `policy` module) returns
//   [`AuthorizationDecision`], never `bool`. There is deliberately **no**
//   `From<AuthorizationDecision> for bool`, no `Into<bool>`, and no
//   `as_bool`: collapsing a decision to a bare bool would discard the
//   reason and evidence that make it auditable (§17.3 control 12: audit
//   logging for all control-plane actions; P0.16 rule 6: audit cross-tenant
//   access denials). The negative-test comment in this module's tests
//   records the compile-level fact, mirroring the domain crate's
//   tenant-context approach.
// - [`AuthorizationDecision::is_allowed`] exists for **control flow only**.
//   It does not replace the decision value: callers must log/audit the full
//   value — reason and evidence — before acting on it. The P1.10
//   `flexcore-audit` `AuditSink` is the evidence sink.
// - Every allow carries an [`AllowReason`] (in Phase 1: an explicit policy
//   rule matched) and every deny a [`DenyReason`], plus an [`EvidenceRef`]
//   pointing at the rule or canonical deny reference that produced it.

use std::fmt;

/// A reference to the evidence that produced a decision: the matched policy
/// rule's reference for allows, a canonical deny reference for denies.
///
/// A string newtype (not a bare `String`) so decision evidence is always
/// deliberately named; there is no `From<String>` and no `Default` — the
/// reference is supplied explicitly by the policy layer or the caller.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EvidenceRef(String);

impl EvidenceRef {
    /// Wraps the given evidence reference string.
    pub fn new(reference: impl Into<String>) -> Self {
        Self(reference.into())
    }

    /// The reference string, suitable for audit records.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EvidenceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why an evaluation **allowed** a request.
///
/// In Phase 1 there is exactly one allow source: an explicit allow rule in
/// the policy set matched the (role, action, scope-kind) request. Implicit
/// scope-derived grants (owner bypasses, hierarchy inheritance) are
/// deliberately not modelled — see the `policy` module. New allow sources
/// extend this enum; nothing allows by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllowReason {
    /// The request matched an explicit allow rule; the decision's
    /// [`EvidenceRef`] is that rule's reference.
    PolicyMatch,
}

impl AllowReason {
    /// Stable lowercase id, suitable for audit records.
    pub fn id(self) -> &'static str {
        match self {
            AllowReason::PolicyMatch => "policy-match",
        }
    }
}

/// Why an evaluation **denied** a request.
///
/// Deny-by-default (ledger P1.11 step 5): every path through evaluation
/// that does not end in an explicit rule match ends in one of these reasons.
/// Cross-tenant denials are audited per P0.16 rule 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DenyReason {
    /// The requested action is outside the known Phase 1 surface
    /// (`Action::parse` found no match). Nothing outside the surface is
    /// ever evaluated against rules.
    UnknownAction,
    /// The request did not name an explicit scope. There is no default
    /// scope: a scopeless request denies regardless of session state.
    UnknownScope,
    /// No tenant context was supplied. Evaluation runs against the domain
    /// `TenantContext` (P0.16 rule 5); without one there is nothing to
    /// match scope tenancy against, so the request denies.
    MissingContext,
    /// The scope's tenant differs from the context's tenant (cross-tenant
    /// request). Audited per P0.16 rule 6; the §17.4 threat table names
    /// cross-tenant leakage as a controlled threat.
    ScopeDenied,
    /// A well-formed, in-tenant request matched no allow rule in the policy
    /// set (including the empty set: deny everything).
    NoMatchingRule,
    /// Reserved deny vocabulary for an explicitly configured deny rule
    /// (deny-beats-allow priority). **Not modelled in Phase 1**: the
    /// `PolicySet` is allow-only and this reason is never produced by the
    /// Phase 1 evaluator — it exists so deny-beats-allow semantics, when a
    /// later phase introduces deny rules, do not silently reuse an
    /// unrelated reason. Recorded per ledger P1.11 ("if not modelled,
    /// state that").
    ExplicitDeny,
}

impl DenyReason {
    /// Stable lowercase id, suitable for audit records.
    pub fn id(self) -> &'static str {
        match self {
            DenyReason::UnknownAction => "unknown-action",
            DenyReason::UnknownScope => "unknown-scope",
            DenyReason::MissingContext => "missing-context",
            DenyReason::ScopeDenied => "scope-denied",
            DenyReason::NoMatchingRule => "no-matching-rule",
            DenyReason::ExplicitDeny => "explicit-deny",
        }
    }
}

/// The outcome of an authorization evaluation: **allow or deny, always with
/// a reason and an evidence reference** (ledger P1.11 step 2).
///
/// This is an explicit value, not a bool. Convert it with
/// [`AuthorizationDecision::is_allowed`] only for control flow, and audit
/// the whole value (reason + evidence) before acting on it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AuthorizationDecision {
    /// The request is allowed; the reason and evidence say **which rule**
    /// allowed it.
    Allow(AllowReason, EvidenceRef),
    /// The request is denied; the reason and evidence say **why**.
    Deny(DenyReason, EvidenceRef),
}

impl AuthorizationDecision {
    /// Constructs an allow decision with its reason and evidence reference.
    pub fn allow(reason: AllowReason, evidence: EvidenceRef) -> Self {
        Self::Allow(reason, evidence)
    }

    /// Constructs a deny decision with its reason and evidence reference.
    pub fn deny(reason: DenyReason, evidence: EvidenceRef) -> Self {
        Self::Deny(reason, evidence)
    }

    /// Control-flow convenience: `true` for [`AuthorizationDecision::Allow`].
    ///
    /// This does **not** replace the decision: the reason and evidence must
    /// still be logged/audited by the caller. There is deliberately no
    /// `From<AuthorizationDecision> for bool` / `Into<bool>` impl — see the
    /// module header and the negative-test comment in the tests.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow(..))
    }

    /// The allow reason, if this decision is an allow.
    pub fn allow_reason(&self) -> Option<AllowReason> {
        match self {
            Self::Allow(reason, _) => Some(*reason),
            Self::Deny(..) => None,
        }
    }

    /// The deny reason, if this decision is a deny.
    pub fn deny_reason(&self) -> Option<DenyReason> {
        match self {
            Self::Allow(..) => None,
            Self::Deny(reason, _) => Some(*reason),
        }
    }

    /// The evidence reference carried by either outcome.
    pub fn evidence(&self) -> &EvidenceRef {
        match self {
            Self::Allow(_, evidence) | Self::Deny(_, evidence) => evidence,
        }
    }
}

impl fmt::Display for AuthorizationDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow(reason, evidence) => {
                write!(f, "allow (reason={}, evidence={})", reason.id(), evidence)
            }
            Self::Deny(reason, evidence) => {
                write!(f, "deny (reason={}, evidence={})", reason.id(), evidence)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULE_REF: &str = "policy:tenant-admin:asset.read";

    #[test]
    fn allow_carries_reason_and_evidence() {
        let decision =
            AuthorizationDecision::allow(AllowReason::PolicyMatch, EvidenceRef::new(RULE_REF));
        assert!(decision.is_allowed());
        assert_eq!(decision.allow_reason(), Some(AllowReason::PolicyMatch));
        assert_eq!(decision.deny_reason(), None);
        assert_eq!(decision.evidence().as_str(), RULE_REF);
    }

    #[test]
    fn deny_carries_reason_and_evidence() {
        let decision = AuthorizationDecision::deny(
            DenyReason::ScopeDenied,
            EvidenceRef::new("authz:deny:scope-denied"),
        );
        assert!(!decision.is_allowed());
        assert_eq!(decision.allow_reason(), None);
        assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied));
        assert_eq!(decision.evidence().as_str(), "authz:deny:scope-denied");
    }

    #[test]
    fn is_allowed_is_control_flow_only_and_the_value_stays_intact() {
        let allow =
            AuthorizationDecision::allow(AllowReason::PolicyMatch, EvidenceRef::new(RULE_REF));
        // The convenience does not consume or degrade the value.
        if allow.is_allowed() {
            assert_eq!(allow.evidence().as_str(), RULE_REF);
            assert_eq!(
                allow.to_string(),
                format!("allow (reason=policy-match, evidence={RULE_REF})")
            );
        }
        let deny = AuthorizationDecision::deny(
            DenyReason::NoMatchingRule,
            EvidenceRef::new("authz:deny:no-matching-rule"),
        );
        assert_eq!(
            deny.to_string(),
            "deny (reason=no-matching-rule, evidence=authz:deny:no-matching-rule)"
        );
    }

    #[test]
    fn deny_reasons_have_stable_audit_ids() {
        assert_eq!(DenyReason::UnknownAction.id(), "unknown-action");
        assert_eq!(DenyReason::UnknownScope.id(), "unknown-scope");
        assert_eq!(DenyReason::MissingContext.id(), "missing-context");
        assert_eq!(DenyReason::ScopeDenied.id(), "scope-denied");
        assert_eq!(DenyReason::NoMatchingRule.id(), "no-matching-rule");
        assert_eq!(DenyReason::ExplicitDeny.id(), "explicit-deny");
    }

    // --- Bypass attempt (negative test) ------------------------------------
    //
    // "No bare bool": the compiler is the oracle, so this is documentation
    // plus the API shape (same approach as the domain crate's tenant-context
    // negative tests). Every attempted bypass below is a **compile error**;
    // if any line were uncommented, the crate would stop compiling:
    //
    //   // 1. No `From<AuthorizationDecision> for bool` exists:
    //   let b: bool = AuthorizationDecision::deny(
    //       DenyReason::MissingContext,
    //       EvidenceRef::new("authz:deny:missing-context"),
    //   ).into();
    //   // error[E0277]: the trait bound `AuthorizationDecision: Into<bool>`
    //   // is not satisfied
    //
    //   // 2. No `as_bool` / `to_bool` accessor exists on the type:
    //   let b = decision.as_bool();
    //   // error[E0599]: no method named `as_bool` found
    //
    //   // 3. `evaluate` returns `AuthorizationDecision`, never `bool` — an
    //    // API-level fact verified by inspection of `policy::PolicySet`:
    //   //   pub fn evaluate(...) -> AuthorizationDecision
    //
    // Runtime note (recorded deliberately): trait-impl *absence* is not
    // observable at runtime on stable Rust (no negative bounds, no
    // specialization), so "no Into<bool>" cannot be asserted by an
    // executing test. What is pinned executably above is that the decision
    // value retains reason and evidence through `is_allowed` use, and that
    // both outcomes require an `EvidenceRef` at construction — a decision
    // without evidence cannot be built at all.
}
