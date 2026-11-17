// P16.22 — The additive market event vocabulary v2 (the P16-E-2 PROPOSAL
//         artifact; decision record: phase-16
//         `records/64_p16_22_market_v2_vocabulary_and_market_actions.md`).
//
// THE FREEZE, RESTATED: `crate::events` carries EXACTLY the 26 frozen
// §15.3 events (`events::EVENT_COUNT == 26`,
// `events::FROZEN_EVENT_NAMES`); it is never edited in place, no market
// event is added to it, and none may be. Market events enter ONLY as THIS
// separately versioned vocabulary module (the P16-E-2 DEFAULT:
// "additive v2 event vocabulary versioned separately from the frozen 26 +
// registered market.* actions"). The E-decision is REGISTERED OPEN —
// closer: the human owner; nothing here closes it, and no producer is
// wired: no code in this program emits any v2 event yet. The module is
// the proposal's typed shape so the decision can be reviewed against
// concrete names and payloads.
//
// Versioning discipline (mirroring the P1.09 rules on a new version):
//
// - **Separate enum, separate tag, separate subject scheme.** The v2 enum
//   is internally tagged `"market_event_type"` (the frozen
//   `DomainEvent`'s `"event_type"` tag is untouched) and its subjects use
//   the `flexcore.v2.market.<slug>` prefix (the frozen
//   `flexcore.v1.<area>.<event_slug>` scheme is untouched).
// - **Schema version constant** (`MARKET_EVENTS_V2_SCHEMA_VERSION`): a
//   breaking change to any payload is a NEW version of the whole v2
//   vocabulary, never a silent field repurpose.
// - **Events are facts, commands are imperatives** (the P0.14 rule,
//   carried to v2): every event below records that a record-level action
//   HAPPENED (a bid record was composed, a grant was issued, a statement
//   was ingested, a reconciliation completed, a dispute opened/resolved).
//   No v2 event name may be used as a command name, and no v2 event
//   carries imperative semantics — `MarketBidComposed` is NOT a bid and
//   never submits anything.
// - **No money figures.** Events carry references, labels, counts, and
//   wire-faithful price TEXT only where the event is about an ingested
//   price record (provenance, verbatim from the P16.07 record layer) —
//   never a computed settlement number (plan §8 criterion 4).
// - **Zero dispatch reach.** No v2 event touches the dispatch lifecycle;
//   the frozen dispatch events (§15.3) remain the ONLY lifecycle facts.
//
// Wire-format note (ASSUMPTION for review, mirroring the P1.09
// assumptions note): the v2 payload field selection below is the
// proposal's; adjusting a field is a v2-schema-version change recorded in
// the decision review, never a silent repurpose.

use serde::{Deserialize, Serialize};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;

/// The market-events-v2 schema version (a breaking change is a NEW
/// version, never a silent edit).
pub const MARKET_EVENTS_V2_SCHEMA_VERSION: u32 = 1;

/// The v2 proposal status statement (pinned by tests): the vocabulary is
/// the P16-E-2 proposal, REGISTERED OPEN, with the human owner as closer;
/// no producer is wired and nothing here closes the decision.
pub const MARKET_EVENTS_V2_PROPOSAL_STATEMENT: &str = "The market event \
     vocabulary v2 is the P16-E-2 PROPOSAL (REGISTERED OPEN; closer: the \
     human owner): an ADDITIVE vocabulary versioned separately from the \
     frozen 26 events (never an edit of events.rs). No producer is wired \
     in this program; no v2 event has ever been emitted; market adapters \
     reach dispatch only through the command lifecycle (the P14.16 \
     command-relay-only rule) and no v2 event carries imperative \
     semantics.";

/// The disjointness statement (pinned by tests): every v2 event name is
/// disjoint from the frozen 26; the frozen vocabulary stays byte-identical.
pub const MARKET_EVENTS_V2_DISJOINT_STATEMENT: &str = "Every market v2 \
     event name is DISJOINT from the frozen 26 §15.3 event names, and the \
     frozen vocabulary (events::FROZEN_EVENT_NAMES, EVENT_COUNT == 26) \
     stays byte-identical — the v2 names are additive and versioned \
     separately, never an in-place extension.";

/// The number of proposed v2 market events.
pub const MARKET_V2_EVENT_COUNT: usize = 8;

/// The proposed v2 event names, PascalCase (the human-readable contract
/// names; the wire tag is the SCREAMING_SNAKE rendering, as in the frozen
/// vocabulary).
pub const MARKET_V2_EVENT_NAMES: [&str; MARKET_V2_EVENT_COUNT] = [
    "MarketBidComposed",
    "MarketGrantIssued",
    "MarketGrantRevoked",
    "MarketPriceRecordIngested",
    "MarketStatementIngested",
    "MarketReconciliationCompleted",
    "MarketDisputeOpened",
    "MarketDisputeResolved",
];

/// `MarketBidComposed` v2 payload — an aggregation-level bid RECORD was
/// composed (P16.06). A fact about composition; never a bid, never a
/// submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketBidComposedV2Payload {
    pub correlation_id: FlexUuid,
    /// The tenant reference (recorded data — the grant's tenant).
    pub tenant_ref: String,
    /// The market-structure name the bid was composed for (the P16.06
    /// `MarketStructureDeclaration::name()` spelling).
    pub market_structure_name: String,
    /// The composed bid record's reference (the internal record identity;
    /// no bid was submitted — none can be).
    pub bid_record_ref: String,
    /// The bid window's interval count.
    pub interval_count: usize,
    /// The number of contributing assets (ascending/deduplicated in the
    /// record; the count here is provenance only).
    pub contributing_asset_count: usize,
    pub event_time: EventTime,
}

/// `MarketGrantIssued` v2 payload — an explicit per-market bid grant was
/// issued (the P16.06 `MarketBidGrant` record).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketGrantIssuedV2Payload {
    pub correlation_id: FlexUuid,
    /// The tenant reference the grant is scoped to.
    pub tenant_ref: String,
    /// The grant record's reference.
    pub grant_ref: String,
    /// The market-structure name the grant covers.
    pub market_structure_name: String,
    /// The grant validity end, END-EXCLUSIVE (unix millis — recorded
    /// data; the composer re-checks it against the injected clock).
    pub valid_until_unix_ms: i64,
    pub event_time: EventTime,
}

/// `MarketGrantRevoked` v2 payload — a per-market grant was revoked (the
/// supersede model: the revoked grant's row is retained; this fact names
/// it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketGrantRevokedV2Payload {
    pub correlation_id: FlexUuid,
    pub tenant_ref: String,
    /// The revoked grant record's reference (retained, never deleted).
    pub grant_ref: String,
    pub event_time: EventTime,
}

/// `MarketPriceRecordIngested` v2 payload — one settlement-adjacent price
/// record was ingested (P16.07). The price is carried as WIRE-FAITHFUL
/// DECIMAL TEXT (provenance, verbatim from the record layer — never a
/// float, never a computed figure).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketPriceRecordIngestedV2Payload {
    pub correlation_id: FlexUuid,
    /// The price record's reference (the publication identity).
    pub record_ref: String,
    /// The market segment label (the closed P16.07 vocabulary spelling:
    /// `day-ahead` / `real-time`).
    pub segment_label: String,
    pub interval_start: EventTime,
    pub interval_end: EventTime,
    /// The price as published (wire-faithful decimal text, verbatim).
    pub price_text: String,
    /// The currency code as published (verbatim, currency as DATA).
    pub currency_code: String,
    /// The delivery area as published (verbatim).
    pub area: String,
    pub event_time: EventTime,
}

/// `MarketStatementIngested` v2 payload — one settlement statement record
/// was ingested (P16.09). No amount travels on the event: line amounts
/// live in the statement records (decimal text + currency as data), and
/// no settlement number is claimed anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketStatementIngestedV2Payload {
    pub correlation_id: FlexUuid,
    /// The statement record's reference.
    pub statement_ref: String,
    /// The declared source-format label (the closed P16.09 vocabulary
    /// spelling: `pjm-manual-29` / `caiso-sqmd` / `ebgl-isp`).
    pub source_format_label: String,
    /// The ingested line-item count (provenance only).
    pub line_item_count: usize,
    pub event_time: EventTime,
}

/// `MarketReconciliationCompleted` v2 payload — a statement→expected
/// reconciliation run completed (P16.09). The counts are the run's own
/// outcome counts (provenance), never settlement figures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketReconciliationCompletedV2Payload {
    pub correlation_id: FlexUuid,
    pub statement_ref: String,
    pub matched_count: usize,
    pub mismatch_count: usize,
    /// The number of dispute-queue entries the run produced (each
    /// carrying BOTH values verbatim — the engine never adjusts).
    pub dispute_queue_size: usize,
    pub event_time: EventTime,
}

/// `MarketDisputeOpened` v2 payload — a line-item challenge was opened
/// against a P16.09 reconciliation-queue entry (P16.12). Disputes are
/// never self-invented: the queue-entry reference is mandatory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketDisputeOpenedV2Payload {
    pub correlation_id: FlexUuid,
    /// The dispute record's reference.
    pub dispute_ref: String,
    /// The P16.09 queue entry the dispute was raised AGAINST.
    pub queue_entry_ref: String,
    /// The challenge-kind label (the closed P16.12 vocabulary spelling).
    pub challenge_kind_label: String,
    pub event_time: EventTime,
}

/// `MarketDisputeResolved` v2 payload — a dispute reached a recorded
/// resolution (P16.12). The resolution kind label is recorded data; an
/// adjusted outcome re-enters through the ingestion pipeline as data —
/// never applied in place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketDisputeResolvedV2Payload {
    pub correlation_id: FlexUuid,
    pub dispute_ref: String,
    /// The resolution-kind label (the closed P16.12 vocabulary spelling).
    pub resolution_kind_label: String,
    /// The evidence-bundle reference (a resolution without its
    /// chain-linked evidence is unconstructable — the P16.12 rule).
    pub evidence_bundle_ref: String,
    pub event_time: EventTime,
}

/// A proposed v2 market event: the additive, separately versioned
/// vocabulary (see the module header — never an edit of the frozen 26).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "market_event_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketEventV2 {
    /// `MarketBidComposed` — a bid RECORD was composed (never submitted).
    MarketBidComposed(MarketBidComposedV2Payload),
    /// `MarketGrantIssued` — a per-market bid grant was issued.
    MarketGrantIssued(MarketGrantIssuedV2Payload),
    /// `MarketGrantRevoked` — a per-market bid grant was revoked.
    MarketGrantRevoked(MarketGrantRevokedV2Payload),
    /// `MarketPriceRecordIngested` — a price record was ingested.
    MarketPriceRecordIngested(MarketPriceRecordIngestedV2Payload),
    /// `MarketStatementIngested` — a settlement statement record was
    /// ingested.
    MarketStatementIngested(MarketStatementIngestedV2Payload),
    /// `MarketReconciliationCompleted` — a reconciliation run completed.
    MarketReconciliationCompleted(MarketReconciliationCompletedV2Payload),
    /// `MarketDisputeOpened` — a dispute was opened against a queue entry.
    MarketDisputeOpened(MarketDisputeOpenedV2Payload),
    /// `MarketDisputeResolved` — a dispute reached a recorded resolution.
    MarketDisputeResolved(MarketDisputeResolvedV2Payload),
}

impl MarketEventV2 {
    /// The v2 event's PascalCase name (the human-readable contract name;
    /// matches [`MARKET_V2_EVENT_NAMES`]).
    pub fn name(&self) -> &'static str {
        match self {
            MarketEventV2::MarketBidComposed(_) => "MarketBidComposed",
            MarketEventV2::MarketGrantIssued(_) => "MarketGrantIssued",
            MarketEventV2::MarketGrantRevoked(_) => "MarketGrantRevoked",
            MarketEventV2::MarketPriceRecordIngested(_) => "MarketPriceRecordIngested",
            MarketEventV2::MarketStatementIngested(_) => "MarketStatementIngested",
            MarketEventV2::MarketReconciliationCompleted(_) => "MarketReconciliationCompleted",
            MarketEventV2::MarketDisputeOpened(_) => "MarketDisputeOpened",
            MarketEventV2::MarketDisputeResolved(_) => "MarketDisputeResolved",
        }
    }

    /// The v2 destination subject: `flexcore.v2.market.<slug>` (the
    /// separately versioned scheme; the frozen `flexcore.v1` scheme is
    /// untouched).
    pub fn subject(&self) -> String {
        let slug = match self {
            MarketEventV2::MarketBidComposed(_) => "bid_composed",
            MarketEventV2::MarketGrantIssued(_) => "grant_issued",
            MarketEventV2::MarketGrantRevoked(_) => "grant_revoked",
            MarketEventV2::MarketPriceRecordIngested(_) => "price_record_ingested",
            MarketEventV2::MarketStatementIngested(_) => "statement_ingested",
            MarketEventV2::MarketReconciliationCompleted(_) => "reconciliation_completed",
            MarketEventV2::MarketDisputeOpened(_) => "dispute_opened",
            MarketEventV2::MarketDisputeResolved(_) => "dispute_resolved",
        };
        format!("flexcore.v2.market.{slug}")
    }

    /// One representative instance of every proposed v2 event, in
    /// [`MARKET_V2_EVENT_NAMES`] order (nil identifiers / empty-ish
    /// reference shapes — the shape, not the data, matters).
    pub fn all_variants() -> Vec<MarketEventV2> {
        let event_time =
            EventTime::from_unix_millis(1_700_000_000_000).expect("valid fixture instant");
        let correlation = FlexUuid::nil();
        vec![
            MarketEventV2::MarketBidComposed(MarketBidComposedV2Payload {
                correlation_id: correlation,
                tenant_ref: "tenant-ref".to_string(),
                market_structure_name: "PJM_SINGLE_AGGREGATION_NODE".to_string(),
                bid_record_ref: "bid://fixture/1".to_string(),
                interval_count: 1,
                contributing_asset_count: 1,
                event_time,
            }),
            MarketEventV2::MarketGrantIssued(MarketGrantIssuedV2Payload {
                correlation_id: correlation,
                tenant_ref: "tenant-ref".to_string(),
                grant_ref: "grant://fixture/1".to_string(),
                market_structure_name: "PJM_SINGLE_AGGREGATION_NODE".to_string(),
                valid_until_unix_ms: 1_700_003_600_000,
                event_time,
            }),
            MarketEventV2::MarketGrantRevoked(MarketGrantRevokedV2Payload {
                correlation_id: correlation,
                tenant_ref: "tenant-ref".to_string(),
                grant_ref: "grant://fixture/1".to_string(),
                event_time,
            }),
            MarketEventV2::MarketPriceRecordIngested(MarketPriceRecordIngestedV2Payload {
                correlation_id: correlation,
                record_ref: "price://fixture/1".to_string(),
                segment_label: "day-ahead".to_string(),
                interval_start: event_time,
                interval_end: EventTime::from_unix_millis(1_700_003_600_000)
                    .expect("valid fixture instant"),
                price_text: "42.50".to_string(),
                currency_code: "EUR".to_string(),
                area: "FIXTURE-ZONE".to_string(),
                event_time,
            }),
            MarketEventV2::MarketStatementIngested(MarketStatementIngestedV2Payload {
                correlation_id: correlation,
                statement_ref: "stmt://fixture/1".to_string(),
                source_format_label: "pjm-manual-29".to_string(),
                line_item_count: 1,
                event_time,
            }),
            MarketEventV2::MarketReconciliationCompleted(MarketReconciliationCompletedV2Payload {
                correlation_id: correlation,
                statement_ref: "stmt://fixture/1".to_string(),
                matched_count: 1,
                mismatch_count: 0,
                dispute_queue_size: 0,
                event_time,
            }),
            MarketEventV2::MarketDisputeOpened(MarketDisputeOpenedV2Payload {
                correlation_id: correlation,
                dispute_ref: "dispute://fixture/1".to_string(),
                queue_entry_ref: "stmt://fixture/1#line#mismatch".to_string(),
                challenge_kind_label: "line_amount_mismatch".to_string(),
                event_time,
            }),
            MarketEventV2::MarketDisputeResolved(MarketDisputeResolvedV2Payload {
                correlation_id: correlation,
                dispute_ref: "dispute://fixture/1".to_string(),
                resolution_kind_label: "upheld".to_string(),
                evidence_bundle_ref: "bundle://fixture/1".to_string(),
                event_time,
            }),
        ]
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// property of this module's own constants or the frozen vocabulary's
// constants, quoted verbatim. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EVENT_COUNT, FROZEN_EVENT_NAMES};

    #[test]
    fn the_frozen_26_stay_exactly_the_frozen_26() {
        // The freeze, pinned here from the frozen module's own constants:
        // byte-identical, never extended in place.
        assert_eq!(EVENT_COUNT, 26);
        assert_eq!(FROZEN_EVENT_NAMES.len(), 26);
        assert_eq!(FROZEN_EVENT_NAMES[0], "AssetRegistered");
        assert_eq!(FROZEN_EVENT_NAMES[25], "OperatorActionRecorded");
        assert!(!FROZEN_EVENT_NAMES.contains(&"MarketBidComposed"));
    }

    #[test]
    fn every_v2_name_is_disjoint_from_the_frozen_names() {
        assert_eq!(MARKET_V2_EVENT_NAMES.len(), MARKET_V2_EVENT_COUNT);
        for name in MARKET_V2_EVENT_NAMES {
            assert!(
                !FROZEN_EVENT_NAMES.contains(&name),
                "v2 name '{name}' must never collide with the frozen vocabulary"
            );
        }
    }

    #[test]
    fn v2_names_round_trip_through_the_enum() {
        let variants = MarketEventV2::all_variants();
        assert_eq!(variants.len(), MARKET_V2_EVENT_COUNT);
        for (expected, event) in MARKET_V2_EVENT_NAMES.iter().zip(variants.iter()) {
            assert_eq!(event.name(), *expected);
        }
    }

    #[test]
    fn v2_events_serde_round_trip_with_the_v2_tag() {
        for event in MarketEventV2::all_variants() {
            let text = serde_json::to_string(&event).expect("v2 event serializes");
            // The v2 tag is its own; the frozen tag name never appears.
            assert!(text.contains("market_event_type"));
            assert!(!text.contains("\"event_type\""));
            let back: MarketEventV2 =
                serde_json::from_str(&text).expect("v2 event deserializes");
            assert_eq!(back, event);
        }
    }

    #[test]
    fn v2_subjects_use_the_versioned_market_scheme() {
        for event in MarketEventV2::all_variants() {
            let subject = event.subject();
            assert!(
                subject.starts_with("flexcore.v2.market."),
                "subject '{subject}' must use the v2 scheme"
            );
        }
        assert_eq!(
            MarketEventV2::MarketBidComposed(MarketBidComposedV2Payload {
                correlation_id: FlexUuid::nil(),
                tenant_ref: "t".to_string(),
                market_structure_name: "PJM_SINGLE_AGGREGATION_NODE".to_string(),
                bid_record_ref: "bid://f".to_string(),
                interval_count: 1,
                contributing_asset_count: 1,
                event_time: EventTime::from_unix_millis(1).expect("valid"),
            })
            .subject(),
            "flexcore.v2.market.bid_composed"
        );
    }

    #[test]
    fn the_proposal_and_disjoint_statements_are_pinned() {
        assert!(MARKET_EVENTS_V2_PROPOSAL_STATEMENT.contains("P16-E-2"));
        assert!(MARKET_EVENTS_V2_PROPOSAL_STATEMENT.contains("REGISTERED OPEN"));
        assert!(MARKET_EVENTS_V2_PROPOSAL_STATEMENT.contains("never an edit of events.rs"));
        assert!(MARKET_EVENTS_V2_PROPOSAL_STATEMENT.contains("command lifecycle"));
        assert!(MARKET_EVENTS_V2_DISJOINT_STATEMENT.contains("DISJOINT"));
        assert!(MARKET_EVENTS_V2_DISJOINT_STATEMENT.contains("byte-identical"));
    }

    #[test]
    fn no_v2_event_carries_imperative_semantics() {
        // The P0.14 rule carried to v2: every name is a past-tense fact;
        // none may be reused as a command name.
        for name in MARKET_V2_EVENT_NAMES {
            assert!(name.ends_with("Composed") || name.ends_with("Issued") || name.ends_with("Revoked") || name.ends_with("Ingested") || name.ends_with("Completed") || name.ends_with("Opened") || name.ends_with("Resolved"), "v2 name '{name}' must be a recorded fact");
        }
    }
}
