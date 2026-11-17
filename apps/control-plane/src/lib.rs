//! Axum control-plane API; composition root (P1.17).
//!
//! Skeleton created by P1.03; implemented by its owning subtask in Phase 1.
//! This file carries no placeholder functions by design — code arrives with
//! the owning subtask in the same phase.
//!
//! The modules are declared here (library form) so the binary (`main.rs`)
//! and the integration tests (`tests/`) compose exactly one wiring: the
//! binary is a thin entry point over this library.
pub mod auth;
pub mod config;
// P3.28 — Phase 3 DTOs (envelopes, schedules, optimization runs) and the
// in-memory Phase 3 record-store fakes for the Test-variant backend.
pub mod dto_phase3;
pub mod phase3_fakes;
// P4.30 — Phase 4 DTOs (dispatch commands, feasibility checks; P4.36
// appended the audit-event DTOs) and the in-memory Phase 4 record-store
// fakes for the Test-variant backend (P4.36 appended the audit-events
// reader fake).
pub mod dto_phase4;
pub mod phase4_fakes;
pub mod dto;
// P5.31 — the alarm DTOs (the wire shapes of the /api/v1/alarms
// derived-on-read resource; mirrors flexcore-contracts::alarm).
pub mod dto_alarms;
pub mod encoding;
pub mod error;
// P11.11 — Idempotency-Key enforcement on command-submission POSTs
// (stored-response replay, 422/409 contract; key representation reuses the
// P1.09 validated envelope key type). Phase 11 wave 1, Agent B;
// WRITTEN-UNRUN; see the phase-11 record `records/23_p11_11_*.md`.
pub mod idempotency;
pub mod metrics;
pub mod metrics_route;
// P11.04 — OpenTelemetry tracing/logs wiring (OTLP export, parent-based
// sampling, the gate-transition span taxonomy over the frozen lifecycle,
// W3C propagation for the P11.05 header seam). WRITTEN-UNRUN; see the
// phase-11 record `records/04_p11_04_otel_tracing_rust.md`.
pub mod otel_telemetry;
pub mod openapi;
pub mod probes;
// P11.14 — per-tenant token-bucket rate limiting + per-tenant in-flight
// command-submission caps (principal classes: operator / edge-agent
// uplink / plugin; 429 + Retry-After contract; in-process, the
// Redis-backed multi-instance path is recorded, not built). Phase 11
// wave 1, Agent B; WRITTEN-UNRUN; see the phase-11 record
// `records/26_p11_14_*.md`.
pub mod rate_limit;
// P13.02 (Phase 13 wave 1, Agent A; WRITTEN-UNRUN) — the OIDC
// token-validation resolver: the PRODUCTION identity path that composes
// the LR-047 "future composition" (an `IdentityResolver` implementation
// over the P1.17 seam). DenyAllResolver stays the SHIPPED DEFAULT; the
// composition root opts in per deployment (no IdP exists in this repo —
// `main.rs` is untouched). Claims → `VerifiedClaims` →
// `AuthenticatedClaims::from_verified` mapping; the JWKS arrives through
// an injected port (no HTTP client here; every network behavior is
// UNRUN). Decision record: phase-13
// `records/02_p13_02_oidc_token_validation.md`.
pub mod oidc;
// P13.04 (Phase 13 wave 1, Agent A; WRITTEN-UNRUN) — the server-side
// step-up challenge protocol replacing the UI-only P5.26 flow (closes
// LR-048 as designed): single-use, time-bounded, purpose-bound
// challenges verified through an injected verifier (the P13-E-3 DEFAULT
// is IdP-delegated MFA; the shipped default verifier denies everything —
// fail closed). A failed/missing challenge is a recorded denial, never a
// wedged command. Decision record: phase-13
// `records/04_p13_04_step_up_mfa.md`.
pub mod stepup;
// P13.16 (Phase 13 wave 2, Agent A; WRITTEN-UNRUN) — session management:
// timeout policy (idle + absolute), the concurrent-session cap, the
// active-session list design surface, and revocation semantics enforced
// at the RESOLVER (a revoked session denies authentication on the next
// request regardless of token validity — never a UI-side sign-out). The
// shipped default remains `DenyAllResolver`; `main.rs`/`state.rs` are
// untouched (no IdP exists — composition is a deployment act, and the
// bare-trait path of the enforcing resolver fails closed). Decision
// record: phase-13 `records/42_p13_16_session_management.md`.
pub mod session;
// P13.09 (Phase 13 wave 1, Agent B; WRITTEN-UNRUN) — the outbound webhook
// service: Stripe-grade signing over the RAW body (5-minute tolerance,
// secret rolling with overlap) and the delivery service (book-before-
// attempt Postgres bookkeeping under a delivery idempotency key, bounded
// exponential-backoff retries, dead-letter table + depth gauge). No
// webhook is signed or delivered by this batch. Decision record: phase-13
// `records/21_p13_09_webhook_service.md`; migration 0020.
pub mod webhook_signing;
pub mod webhook_service;
// FRV1-R8-005 (final review) — the port implementation over the Postgres
// adapter's webhook-delivery store (migration 0020): the thin record/
// error mapping that joins the app-local `WebhookDeliveryStore` port to
// the adapter's storage half (the P13.09 decision record §5 step 1).
// WRITTEN-UNRUN.
pub mod webhook_delivery_pg;
// P13.10 (Phase 13 wave 1, Agent B; WRITTEN-UNRUN) — the API-versioning
// policy mechanics: `/api/v1/` confirmed CURRENT, the Deprecation/Sunset
// header design (RFC 8594 [S-84]; the Deprecation header is a draft, used
// advisory), the 6–12 month window bounds, and the pure OpenAPI
// breaking-change checker over the GENERATED spec. Decision record:
// phase-13 `records/22_p13_10_api_versioning_deprecation.md`.
pub mod api_versioning;
// P13.11 (Phase 13 wave 1, Agent B; WRITTEN-UNRUN) — the notification
// pipeline: provider-abstracted channel ports (P13-E-2 OPEN, DEFAULT
// email-first), a validated template registry, per-tenant severity
// preferences, and the event→notification mapping (alarm/audit-driven
// only; absence from the map = audit-only). NO channel is ever a dispatch
// path — the ports return receipts only. No notification has ever been
// sent. Decision record: phase-13
// `records/23_p13_11_notification_pipeline.md`.
pub mod notifications;
// P13.21 (Phase 13 wave 2, Agent A; WRITTEN-UNRUN) — the
// notification/audit interplay vocabulary: the closed, registered,
// VERSIONED disposition of every one of the 26 frozen events (Notify vs
// AuditOnly) as one DATA table — registered against the P13.11
// `EVENT_NOTIFICATION_MAP` (rows agree, pinned by test; the map itself is
// untouched). Absence-made-explicit: exactly the four record-23 alarm rows
// notify; every dispatch transition except DispatchFailed is audit-only.
// Notifications are never a dispatch path. Decision record: phase-13
// `records/47_p13_21_notification_audit_interplay.md`.
pub mod notification_vocab;
// P13.12 (Phase 13 wave 1, Agent B; WRITTEN-UNRUN) — the audit export
// service: signed export packs (SIEM-friendly JSONL) built over the
// SELECT-only `AuditEventReader` port, tenant-scoped by construction, with
// an integrity statement that INHERITS the P11.15 keyed chain and states
// its non-verification gap verbatim. No export has ever been produced.
// Decision record: phase-13 `records/24_p13_12_audit_export.md`.
pub mod audit_export;
pub mod routes;
// P3.28 — Phase 3 routes (flexibility envelopes, schedules, optimization
// runs; §15.2 resources of the Phase 3 line).
pub mod routes_phase2;
pub mod routes_phase3;
// P4.30 — Phase 4 routes (dispatch commands, feasibility checks; §15.2
// resources of the Phase 4 line, READ-ONLY — no execute-command endpoint).
// P4.36 appended the /api/v1/audit-events READ resource additively (same
// read-only discipline — the append-only trail has no update, delete, or
// export endpoint).
pub mod routes_phase4;
// P5.25 — Phase 5 routes (the tenant-scoped live-update SSE stream,
// /api/v1/live; §15.1/§21.3; READ-only, at-least-once delivery honesty).
// Also hosts the in-process LiveEventBus the composition root's NATS
// forwarder publishes into, and the forwarder task itself.
pub mod routes_live;
// P5.31 — the /api/v1/alarms READ resource (alarms derived on read from
// existing platform facts through flexcore-domain::alarm_rules; READ-only,
// no acknowledge endpoint, the §15.3 event vocabulary is not extended).
pub mod routes_alarms;
// P6.06 — the /api/v1/edge/enroll enrollment seam (additive path per the
// recorded P4.36/P5.31 rule; provisioning-token auth, NOT a role-gated
// caller; the fail-closed enrollment matrix; migration 0018 persistence
// behind flexcore-storage's EdgeEnrollmentStore). Also hosts the
// token-validator seam and the Test-backend fake store.
pub mod dto_edge;
pub mod routes_edge;
// P7.25 — the /api/v1/plugins plugin-inventory READ resource (the
// operator console's plugin seam, §20.1 item 10; READ-only, additive path
// per the recorded P4.36/P5.31/P6.06 rule — no activate/deactivate/
// upgrade/profile-switch endpoint exists, and the shipped default serves
// the RECORDED shipped inventory with every runtime fact an explicit gap).
pub mod dto_plugins;
pub mod routes_plugins;
// P8.27 — the /api/v1/research/status research-status READ resource (the
// operator console's research surfacing seam, §20.1 item 9; READ-only,
// additive path per the recorded P4.36/P5.31/P6.06/P7.25 rule — no
// promote/issue/authorize/fast-track/approve endpoint exists, and the
// shipped default serves the RECORDED Phase 8 research status with every
// run fact an explicit gap).
pub mod dto_research;
pub mod routes_research;
pub mod state;
