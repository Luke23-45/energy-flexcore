//! P15.04 — ENTSO-E Transparency Platform price-signal ingestion adapter
//!         models (crates/adapters/protocol-entsoe).
//!
//! Decision record:
//! `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
//! 04_p15_04_price_ingestion_entsoe.md` (registers P15-E-2 OPEN with
//! DEFAULT). Source: [S-111] (ENTSO-E Transparency Platform + API guide,
//! gap analysis §11 Track E); plan: phase 15 plan §2 row P15.04.
//!
//! What this crate IS (the honest scope):
//!
//! - **Model types** for the A44 day-ahead price document ([`document`]):
//!   the document identity, time series, periods, and points — the
//!   shapes the day-ahead price publication carries ([S-111]; every
//!   wire-level detail UNVERIFIED until checked against the live API
//!   guide).
//! - **The document extraction seam** ([`xml_seam`]): a minimal internal
//!   XML node type plus a pure extraction from a parsed A44 document into
//!   validated price points. NO XML parser library (the CIM/CGMES
//!   precedent, P14.12): the composition root's parser fills the node
//!   tree; the extraction logic is real, deterministic, testable code.
//! - **The API-token HUMAN GATE boundary** ([`token`]): the registered
//!   token is a HUMAN GATE input — scheme-prefixed REFERENCE only
//!   (UNSIGNED until a human registers it); token MATERIAL is
//!   structurally impossible (there is no field a key could hide in; a
//!   reference without a scheme prefix is rejected and its value never
//!   echoed — the P7.05/P8.32 secret-reference rule).
//! - **The REST transport SEAM** ([`transport`]): the query-parameter
//!   assembly for the A44 request (UNVERIFIED parameter names and
//!   endpoint shape per [S-111]) as a port trait plus an unwired
//!   implementation that answers every request with the named
//!   `SeamNotWired` refusal. NO HTTP client, NO TLS stack: no request is
//!   ever sent in this program (conventions §1 loop bans; the first live
//!   call is a human/lab gate after P15-E-2 closes).
//! - **The ingest-only authority scope** ([`scope`]): prices are
//!   INGESTED DATA for later phases — the adapter has no bid, offer,
//!   order, or dispatch vocabulary anywhere in its surface (structural,
//!   not conventional; the ADR-010 extension and the ADR-011 boundary).
//!
//! Standard-detail honesty: the A44 document-type code, the element
//! names, the query parameter names, the period format, and the
//! resolution codes are cited from [S-111] and carry `UNVERIFIED` labels
//! naming the implementation-time check against the live Transparency
//! Platform API guide. A mismatch at that gate is a recorded code or
//! record change — never a silent reinterpretation.
//!
//! Control status: READ-ONLY data ingestion for the simulation-only 0.x
//! line. Nothing here authorizes, issues, dispatches, or bids; a price
//! series is an input to later-phase analytics (Phase 16 consumes it;
//! nothing here bids), never a command, a FlexCore accuracy claim, or a
//! market action. No conformance, qualification, or certification claim
//! is made.

pub mod document;
pub mod scope;
pub mod token;
pub mod transport;
pub mod xml_seam;
