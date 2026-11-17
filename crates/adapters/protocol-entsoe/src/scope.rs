// P15.04 — The ingest-only authority scope.
//
// Prices are INGESTED DATA for later phases. This module records the
// adapter's authority scope as a closed, validated declaration — the
// same shape discipline as the Phase 14 protocol adapters' scope
// declarations (P14.01/P14.07), applied to a DATA-ingestion adapter:
//
// - the adapter's role is INGEST-ONLY (it fetches published price
//   documents and hands them to the platform's price port);
// - its control scope is NONE: there is no bid, offer, order, dispatch,
//   or device-write vocabulary anywhere in this crate's surface
//   (structural — the traits and types cannot express one);
// - its resource class is MARKET DATA (ingested records), never a
//   dispatch input in this phase (Phase 16 consumes forecasts and price
//   records; nothing here bids — the ADR-011 boundary is unchanged);
// - no market qualification, conformance, or certification claim is
//   made (the honesty rules §7.3).
//
// Control status: a scope RECORD. Nothing here authorizes, issues,
// dispatches, or bids.

/// The role vocabulary (closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriceAdapterRole {
    /// Fetch published price documents and deliver them to the price
    /// port (READ-ONLY ingestion; the only role this adapter declares).
    IngestOnly,
}

impl PriceAdapterRole {
    /// The canonical spelling (records).
    pub const fn as_str(self) -> &'static str {
        match self {
            PriceAdapterRole::IngestOnly => "ingest-only",
        }
    }
}

/// The control-scope vocabulary (closed). The adapter declares NONE —
/// there is no control surface to declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriceAdapterControlScope {
    /// No control authority of any kind (the only member; the enum exists
    /// so the absence of control is a DECLARED value, not an omission).
    None,
}

impl PriceAdapterControlScope {
    /// The canonical spelling (records).
    pub const fn as_str(self) -> &'static str {
        match self {
            PriceAdapterControlScope::None => "none",
        }
    }
}

/// The resource-class vocabulary (closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriceAdapterResourceClass {
    /// Published market data, ingested as records with provenance.
    MarketData,
}

impl PriceAdapterResourceClass {
    /// The canonical spelling (records).
    pub const fn as_str(self) -> &'static str {
        match self {
            PriceAdapterResourceClass::MarketData => "market-data",
        }
    }
}

/// The recorded authority statement (pinned by tests).
pub const PRICE_INGEST_AUTHORITY_STATEMENT: &str = "the ENTSO-E price adapter is INGEST-ONLY \
     with control scope NONE: prices are INGESTED DATA for later phases, never bid inputs \
     and never dispatch inputs (ADR-010 extension; the ADR-011 boundary is unchanged). \
     The adapter has no bid, offer, order, dispatch, or device-write vocabulary anywhere \
     in its surface. No market qualification, conformance, or certification claim is \
     made.";

/// The validated scope declaration of this adapter (the one declaration
/// the crate carries; constructed by [`price_ingest_scope`], which always
/// succeeds because the vocabulary has exactly one honest member per
/// class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceAdapterScope {
    /// The role (closed; always ingest-only).
    pub role: PriceAdapterRole,
    /// The control scope (closed; always none).
    pub control_scope: PriceAdapterControlScope,
    /// The resource class (closed; always market data).
    pub resource_class: PriceAdapterResourceClass,
}

impl PriceAdapterScope {
    /// The rendered scope line (records, audit details).
    pub fn as_record(&self) -> String {
        format!(
            "role:{}|control:{}|resource:{}",
            self.role.as_str(),
            self.control_scope.as_str(),
            self.resource_class.as_str()
        )
    }
}

/// The adapter's scope declaration (deterministic; the closed
/// vocabularies admit exactly one honest member each).
pub fn price_ingest_scope() -> PriceAdapterScope {
    PriceAdapterScope {
        role: PriceAdapterRole::IngestOnly,
        control_scope: PriceAdapterControlScope::None,
        resource_class: PriceAdapterResourceClass::MarketData,
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// literal copied from this module's constants. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scope_declares_ingest_only_with_no_control() {
        let scope = price_ingest_scope();
        assert_eq!(scope.role.as_str(), "ingest-only");
        assert_eq!(scope.control_scope.as_str(), "none");
        assert_eq!(scope.resource_class.as_str(), "market-data");
        assert_eq!(
            scope.as_record(),
            "role:ingest-only|control:none|resource:market-data"
        );
    }

    #[test]
    fn the_authority_statement_carries_the_boundary() {
        assert!(PRICE_INGEST_AUTHORITY_STATEMENT.contains("INGEST-ONLY"));
        assert!(PRICE_INGEST_AUTHORITY_STATEMENT.contains("never bid inputs"));
        assert!(PRICE_INGEST_AUTHORITY_STATEMENT.contains("never dispatch inputs"));
        assert!(PRICE_INGEST_AUTHORITY_STATEMENT.contains("no market qualification"));
    }
}
