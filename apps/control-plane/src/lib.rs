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
pub mod metrics;
pub mod metrics_route;
pub mod openapi;
pub mod probes;
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
