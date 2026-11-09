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
pub mod dto;
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
pub mod state;
