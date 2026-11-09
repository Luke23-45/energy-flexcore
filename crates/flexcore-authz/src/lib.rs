// P1.11 — flexcore-authz crate root: authorization decision types, scope
//         types, and deny-by-default policy evaluation.
//
//! Neryva FlexCore — authorization crate (P1.11; report §8.1
//! `flexcore-authz`, §17.3 control 2, §16.2; ledger P1.11; P0.16–P0.17).
//!
//! This crate holds authorization **policy types and their evaluation** for
//! the Phase 1 API surface (portfolio/site/asset registry actions). It is a
//! type-and-evaluation crate only: no identity federation, no policy
//! storage, no decision cache, no dispatch involvement.
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`decision`] | Ledger P1.11 step 2 | `AuthorizationDecision` — allow/deny always carrying a reason and an evidence reference; never a bare bool |
//! | [`scope`] | Ledger P1.11 step 3 | `TenantScope` / `SiteScope` / `AssetScope` (typed over domain IDs) and the `Scope` sum; a decision is always evaluated against an explicit scope |
//! | [`action`] | Ledger P1.11 step 5 | The `Action` vocabulary of the Phase 1 surface, with canonical ids and a rejecting parse |
//! | [`policy`] | Ledger P1.11 steps 4–5 | `Role`, the allow-only `PolicySet`, and `PolicySet::evaluate` — deny-by-default against the domain `TenantContext` |
//! | [`phase4_policies`] | P4.13 | The recorded Phase 4 `command.*` grant set (creation, validation, approval, read) — the same deny-by-default `PolicySet`, extended additively |
//!
//! # The deny-by-default rule
//!
//! Everything that is not explicitly granted is denied, with a reason. An
//! unknown action denies ([`decision::DenyReason::UnknownAction`]), a
//! missing scope denies ([`decision::DenyReason::UnknownScope`]), a missing
//! tenant context denies ([`decision::DenyReason::MissingContext`]), a
//! cross-tenant scope denies ([`decision::DenyReason::ScopeDenied`]), and a
//! request matching no allow rule denies
//! ([`decision::DenyReason::NoMatchingRule`]). There is no default scope and
//! no default allow anywhere in this crate. Tests prove each case (ledger
//! P1.11 step 5 / acceptance criteria).
//!
//! # The decision-carries-reason rule
//!
//! [`policy::PolicySet::evaluate`] returns
//! [`decision::AuthorizationDecision`], never `bool`. Every decision carries
//! a reason and a [`decision::EvidenceRef`]. There is deliberately no
//! `From<AuthorizationDecision> for bool` and no `Into<bool>`:
//! [`decision::AuthorizationDecision::is_allowed`] is a control-flow
//! convenience only, and callers must log/audit the full decision — reason
//! and evidence — before acting on it (§17.3 control 12; P0.16 rule 6; the
//! P1.10 `AuditSink` is the evidence sink).
//!
//! # Boundary: policy types and evaluation — not the dispatch authorizer
//!
//! The protected kernel — **final dispatch authorization** and dispatch
//! state transitions — is a **later phase (Phase 4)**. `dispatch.authorize`
//! is **never granted to general plugins** and `dispatch.issue` is owned by
//! the protected dispatch service and protocol adapter under a signed
//! authorization token (report §5.7 permission table; P0.17 threat-model
//! freeze; P0.19 plugin model: a plugin may propose work, it must not grant
//! itself authority). Nothing in this crate may be presented as the dispatch
//! authorizer, and this evaluation is protected platform behavior, not a
//! plugin-replaceable or plugin-configurable seam (P0.17). The `Action`
//! surface here is the Phase 1 registry surface only.
//!
//! # Scope and control status
//!
//! Vocabulary and evaluation for the design baseline only. The entire 0.x
//! line is simulation-only; nothing in this crate authorises field control
//! or device writes. Control-authority modes (simulation, read-only/shadow,
//! operator-approved, bounded automation) are deployment and per-site
//! settings; a plugin can never select them (report §5.7).

pub mod action;
pub mod decision;
pub mod phase4_policies;
pub mod policy;
pub mod scope;
