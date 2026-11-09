//! Neryva FlexCore — capability interfaces (ports) with no implementations
//! (P1.07).
//!
//! The port layer of the hexagonal architecture (report §6.2, §8.1, §9.1):
//! the capability contracts the application layer depends on and the
//! adapters implement. **Ports define interfaces, not implementations**: no
//! port trait in this crate mentions a concrete product, a wire protocol, a
//! schema, or a vendor type, and no implementation ships here — except the
//! [`fakes`] module, which is a set of documented **test doubles** (P1.07
//! step 5: contract tests proving the application runs without any
//! infrastructure; they must never be wired into a production deployment).
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`backend_error`] | P1.08 error-taxonomy input | The opaque backend-error value adapters report with (code + message; never raw storage or broker text) |
//! | [`state_change`] | report §6.3 step 1, §15.5; P1.10 | The state-change vocabulary shared by the write ports: entity names, `StateChangeOperation`, and the `StateChangeReceipt` a repository write returns |
//! | [`repository`] | P1.07 step 2; P0.16 rules 1/5; P1.12 §3 | One repository port per Stage 1 aggregate (Tenant, Portfolio, Site, Asset) with CRUD + tenant-scoped queries; every method that touches tenant-owned data takes the domain `TenantContext` |
//! | [`event_outbox`] | report §6.3 steps 1–3, §15.5; P1.12 §4 | The `EventOutbox` port: append one complete transactional write (state change + outbox record), read pending entries for the publisher, record attempts, mark delivered/failed |
//! | [`clock`] | P0.09 time model | Deterministic time for tests: `now_event_time` / `now_ingest_time` as the two distinct contracts timestamp types |
//! | [`id_generator`] | contracts `ids` rule | New `FlexUuid` values and typed entity identifiers; the version policy is the adapter's |
//! | [`telemetry_source`] | P2.11; report §9.1 | The `TelemetrySource` port: tenant-scoped `TelemetryQuery` (constructor requires the domain `TenantContext`), the P2.04 canonical `NormalizedTelemetry` record (Result-constructed — no partial records), subscribe/decode/source_id/health, and the structural provider rule (`ProviderDeclaration`: cadence, quality mapping, clock behaviour). Phase 2 provider set: exactly one — the simulator (P2.14) |
//! | [`telemetry_source_contract`] | P2.27; report §19.2 layer 3 | The reusable `TelemetrySource` provider contract suite: generic over the port, so every provider (the simulator now, future adapters later) passes the same checks — declaration, canonical schema, six-state quality closure, per-asset sequence monotonicity, event-time-on-clock, ingest-time-at-emission, and deterministic same-seed reproduction. No implementation ships here; the CI instantiation drives it against a documented in-memory stand-in (the ports crate cannot depend on the simulator) |
//! | [`telemetry_ingest`] | P2.09/P2.10 | The ingest-side write port: `TelemetryIngestStore` (batch insert with the P2.09 exact-vs-conflicting duplicate classification, quarantine evidence writes, latest-by-`event_time` current-state snapshot) and `IngestionInbox` (the P1.16 inbox pattern split into check/record halves for consumer-side idempotency) |
//! | [`telemetry_read`] | P2.12/P2.17/P2.18 | The `TelemetryReader` port — the read side of the canonical telemetry store: keyset-paginated time-range query (cursor decided once: the `(event_time, asset_id, quantity)` sort key), the P2.18 latest-by-`event_time` current state, and the P2.07 quarantine read. Tenant-scoped by construction. The Postgres implementation on the unit-of-work adapter is the recorded P2.06-adjacent gap; until it lands the only implementation is the test fake |
//! | [`tenant_context_source`] | P0.16 rule 5 | The `TenantContextSource` port — the documented integration point for the P1.06 claims boundary (no implementation can exist until the domain exposes a claims constructor) |
//! | [`fakes`] | P1.07 step 5 | In-memory test fakes: tenant-scoped repositories, outbox, fixed clock, counting ID generator, and the shared write journal used to evidence §6.3 ordering |
//!
//! # Rules this crate encodes
//!
//! - **Sync traits.** No port takes or returns a future, spawns a task, or
//!   names an async runtime. If a real adapter needs asynchrony, the adapter
//!   decides how to bridge it (threading, blocking hand-off, or an
//!   async-side wrapper around the sync trait) — recorded in the crate
//!   README. This keeps every port usable from the sync domain and
//!   application layers without polluting them with runtime types.
//! - **No vendor types.** A source-text test in this file asserts that no
//!   module names a database engine, message broker, HTTP framework, or
//!   async runtime; the same `grep` gate runs in CI (P1.28). Adapters own
//!   every concrete technology (P1.13 persistence, P1.15 broker).
//! - **Repository traits live here** (single-home rule, recorded by P1.10
//!   step 1): one home, one definition; the P1.30 boundary audit checks that
//!   flexcore-storage defines no repository trait. The append-only audit
//!   write port is the one recorded exception and it lives in
//!   flexcore-audit — this crate does not re-export it because the
//!   ports → audit edge is not in the allowed dependency set
//!   (`tools/check_dependency_rule.py`).
//! - **Tenant scoping is structural** (P0.16 rules 1 and 5): every port
//!   method that touches tenant-owned data takes the domain `TenantContext`.
//!   There is no method for tenant-owned data without one — a raw tenant-ID
//!   parameter path does not exist.
//!
//! # Scope and control status
//!
//! Capability contracts for the design baseline only. The entire 0.x line is
//! simulation-only; nothing in this crate touches a database, a broker, a
//! device, or a live grid, and nothing here authorises field control.
//! FlexCore is a supervisory orchestration layer and performs no primary
//! frequency response, synthetic inertia, relay protection, inverter
//! inner-loop control, or sub-cycle control.

pub mod backend_error;
pub mod clock;
pub mod event_outbox;
pub mod fakes;
pub mod id_generator;
pub mod repository;
pub mod state_change;
pub mod telemetry_ingest;
pub mod telemetry_read;
pub mod telemetry_source;
pub mod telemetry_source_contract;
pub mod tenant_context_source;

#[cfg(test)]
mod tests {
    /// The forbidden vendor identifiers, assembled from halves so this test
    /// module itself never contains a literal hit for the CI `grep` gate
    /// (P1.07 acceptance: "no port trait mentions a vendor type — grep
    /// check"; P1.28 runs the same check as a required gate).
    fn vendor_needles() -> Vec<String> {
        vec![
            format!("{}{}", "sq", "lx"),
            format!("{}{}", "na", "ts"),
            format!("{}{}", "ax", "um"),
            format!("{}{}", "post", "gres"),
            format!("{}{}", "tok", "io"),
        ]
    }

    // P2.27 note: this table must list EVERY source file of the crate. The
    // array length was 11 while twelve files were listed (a pre-existing
    // compile-blocking mismatch found while wiring telemetry_source_contract);
    // the length is now 13 with the new module added.
    const SOURCES: [(&str, &str); 13] = [
        ("lib.rs", include_str!("lib.rs")),
        ("backend_error.rs", include_str!("backend_error.rs")),
        ("clock.rs", include_str!("clock.rs")),
        ("event_outbox.rs", include_str!("event_outbox.rs")),
        ("fakes.rs", include_str!("fakes.rs")),
        ("id_generator.rs", include_str!("id_generator.rs")),
        ("repository.rs", include_str!("repository.rs")),
        ("state_change.rs", include_str!("state_change.rs")),
        ("telemetry_ingest.rs", include_str!("telemetry_ingest.rs")),
        ("telemetry_source.rs", include_str!("telemetry_source.rs")),
        (
            "telemetry_source_contract.rs",
            include_str!("telemetry_source_contract.rs"),
        ),
        ("telemetry_read.rs", include_str!("telemetry_read.rs")),
        ("tenant_context_source.rs", include_str!("tenant_context_source.rs")),
    ];

    #[test]
    fn no_port_source_mentions_a_vendor_type() {
        let needles = vendor_needles();
        for (name, source) in SOURCES.iter() {
            let lowered = source.to_lowercase();
            for needle in needles.iter() {
                assert!(
                    !lowered.contains(needle.as_str()),
                    "port source {name} must not mention vendor types (found {needle}); \
                     adapters own every concrete technology"
                );
            }
        }
    }

    #[test]
    fn every_port_trait_is_object_safe() {
        // The composition root holds ports as trait objects (Arc<dyn ...>);
        // object safety is therefore part of the contract. This test compiles
        // trait-object references for every port defined in the crate.
        use crate::clock::Clock;
        use crate::event_outbox::EventOutbox;
        use crate::id_generator::IdGenerator;
        use crate::repository::{AssetRepository, PortfolioRepository, SiteRepository, TenantRepository};
        use crate::telemetry_source::TelemetrySource;
        use crate::telemetry_read::TelemetryReader;
        use crate::telemetry_ingest::{IngestionInbox, TelemetryIngestStore};
        use crate::tenant_context_source::TenantContextSource;

        fn takes_dyn_clock(_: &dyn Clock) {}
        fn takes_dyn_ids(_: &dyn IdGenerator) {}
        fn takes_dyn_outbox(_: &dyn EventOutbox) {}
        fn takes_dyn_tenants(_: &dyn TenantRepository) {}
        fn takes_dyn_portfolios(_: &dyn PortfolioRepository) {}
        fn takes_dyn_sites(_: &dyn SiteRepository) {}
        fn takes_dyn_assets(_: &dyn AssetRepository) {}
        fn takes_dyn_telemetry(_: &dyn TelemetrySource) {}
        fn takes_dyn_telemetry_read(_: &dyn TelemetryReader) {}
        fn takes_dyn_telemetry_ingest(_: &dyn TelemetryIngestStore) {}
        fn takes_dyn_ingest_inbox(_: &dyn IngestionInbox) {}
        fn takes_dyn_context(_: &dyn TenantContextSource) {}

        // Referencing the functions keeps them compiled; the assertions hold
        // because the mere declarations above fail to compile if any trait
        // stops being object-safe.
        let _ = takes_dyn_clock;
        let _ = takes_dyn_ids;
        let _ = takes_dyn_outbox;
        let _ = takes_dyn_tenants;
        let _ = takes_dyn_portfolios;
        let _ = takes_dyn_sites;
        let _ = takes_dyn_assets;
        let _ = takes_dyn_telemetry;
        let _ = takes_dyn_telemetry_ingest;
        let _ = takes_dyn_ingest_inbox;
        let _ = takes_dyn_context;
    }
}
