// P16.06 — The bid composition service: asset envelopes → aggregation-
//         level bids per market structure (PJM single aggregation node
//         [S-90]; CAISO PDR two-ID load-shift [S-96]; ISO-NE DAM Asset
//         tab [S-103]).
//
// Phase 16 plan §2 row P16.06; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 06_p16_06_bid_composition_service.md`.
//
// THE BOUNDARIES, DECIDED ONCE:
//
// - **The envelope remains the physical truth layer.** Every asset
//   quantity entering composition carries its envelope-version
//   reference; a composition without envelope provenance is refused
//   (`MissingEnvelopeProvenance`). The composed record carries the
//   envelope references forward — the bid is DERIVED FROM the envelope,
//   never independent of it, and no adjustment factor is applied
//   silently (there is none to apply).
// - **Operator-approved mode + an explicit per-market grant are
//   REQUIRED before any bid is composed** (context §3.2; conventions
//   §6.4; [S-120]). Composition refuses any other control-authority
//   mode, and refuses a grant that is not ACTIVE, is expired (per the
//   injected clock), is scoped to another tenant, or does not name the
//   market structure being composed. The mode is consumed BY NAME (the
//   kernel's `ControlAuthorityPosture` name — one home, never
//   restated); the grant is a RECORD, and the permission surface
//   (market.* actions) is P16.19's, not this module's.
// - **No bid is submitted — none can be.** The output is a
//   `ComposedBidRecord`: an internal record of what WOULD be bid.
//   Submission requires a jurisdiction adapter (ADR-011), a named
//   market counterparty (D1, OPEN), and an operator-approved deployment
//   that this program does not have. No transmission path exists here.
// - **Jurisdiction-specific structures, never a universal bid**
//   (ADR-011). The closed structure set below is the three the plan
//   names; a fourth enters as a NEW variant in a jurisdiction record
//   pack. The CAISO two-ID semantics are recorded pack data [S-96] —
//   the identifier ROLES are structural, the market-facing semantics
//   stay with the record pack (secondary-source specifics marked
//   UNVERIFIED in the decision record).
// - **No new solver classes** (conventions §6.5): composition is
//   deterministic summation of envelope-derived quantities per
//   interval — LP/QP results consumed, no optimization performed, no
//   MISOCP/MINLP creep.
// - **Determinism**: BTreeMap ordering by asset id, explicit interval
//   alignment checks, injected clock for the composed-at timestamp.
//
// Control status: a record composer for the simulation-only 0.x line.
// No I/O, no transmission, no market connection.

use flexcore_ports::clock::Clock;
use std::collections::BTreeMap;

/// The bid-composition record schema version (a change is a NEW
/// version, never a silent edit).
pub const BID_COMPOSITION_SCHEMA_VERSION: u32 = 1;

/// The no-submission statement (pinned by tests).
pub const NO_BID_SUBMITTED_STATEMENT: &str = "No bid is submitted — none can be: the \
     output is a ComposedBidRecord, an internal record of what WOULD be bid. Submission \
     requires a jurisdiction adapter (ADR-011), a named market counterparty (D1, OPEN), \
     and an operator-approved deployment that this program does not have.";

/// The envelope-truth statement (pinned by tests).
pub const ENVELOPE_IS_TRUTH_LAYER_STATEMENT: &str = "The envelope remains the physical \
     truth layer: every asset quantity entering composition carries its envelope-version \
     reference, the composed record carries the references forward, and no adjustment \
     factor is applied silently — the bid is derived from the envelope, never independent \
     of it.";

/// The grant statement (pinned by tests).
pub const OPERATOR_GRANT_REQUIRED_STATEMENT: &str = "Operator-approved mode plus an \
     explicit per-market grant are REQUIRED before any bid is composed (context §3.2; \
     program conventions §6.4; [S-120]): any other control-authority mode, a non-ACTIVE \
     grant, an expired grant, a wrong tenant, or a wrong market structure is a named \
     refusal.";

/// The kernel's `ControlAuthorityPosture::OperatorApproved` name (the
/// one home of the control-authority vocabulary is the kernel; this
/// module consumes the NAME, never restates the enum).
pub const REQUIRED_CONTROL_MODE_NAME: &str = "operator-approved";

// --- Errors -------------------------------------------------------------------

/// Why a bid composition was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BidCompositionError {
    /// A required text field was empty (`field` names it).
    EmptyField {
        field: &'static str,
    },
    /// A value was outside its closed vocabulary — not an empty field, an
    /// out-of-vocabulary one (named separately: FRV1-R7-007).
    InvalidEnumValue {
        field: &'static str,
        value: String,
    },
    /// A timestamp field carried a negative value — not an empty field, a
    /// negative one (named separately: FRV1-R7-007).
    NegativeTimestamp {
        field: &'static str,
    },
    /// A validity window was inverted (valid_until not strictly after
    /// granted_at, end-exclusive semantics) — not an empty field
    /// (FRV1-R7-007).
    InvertedValidityWindow {
        granted_at_unix_ms: i64,
        valid_until_unix_ms: i64,
    },
    /// The control-authority mode is not operator-approved (composition
    /// requires the exact kernel posture name).
    ControlModeInsufficient {
        mode_name: String,
    },
    /// The per-market grant was not ACTIVE.
    GrantNotActive {
        grant_id: String,
        grant_state: String,
    },
    /// The per-market grant was expired (per the injected clock).
    GrantExpired {
        grant_id: String,
        now_unix_ms: i64,
        valid_until_unix_ms: i64,
    },
    /// The per-market grant is scoped to another tenant.
    GrantTenantMismatch {
        grant_id: String,
    },
    /// The per-market grant names a different market structure than the
    /// composition targets.
    GrantMarketMismatch {
        grant_id: String,
    },
    /// An asset quantity lacked its envelope-version reference (the
    /// envelope is the physical truth layer — no provenance, no bid).
    MissingEnvelopeProvenance {
        asset_id: String,
    },
    /// An asset quantity was negative or non-finite (an offer is a
    /// finite non-negative magnitude; the sign convention is recorded
    /// pack data and no sign is invented here).
    InvalidQuantity {
        asset_id: String,
        interval_index: usize,
    },
    /// The asset inputs do not share the event interval count.
    IntervalCountMismatch {
        asset_id: String,
        expected: usize,
        found: usize,
    },
    /// No asset inputs were supplied (a bid of nothing is not a bid).
    NoAssetInputs,
    /// A market identifier violated its shape (non-empty, bounded).
    InvalidMarketIdentifier {
        field: &'static str,
    },
}

impl std::fmt::Display for BidCompositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BidCompositionError::EmptyField { field } => {
                write!(f, "the bid-composition field '{field}' must be non-empty")
            }
            BidCompositionError::InvalidEnumValue { field, value } => {
                write!(f, "the bid-composition field '{field}' carries the out-of-vocabulary value '{value}'")
            }
            BidCompositionError::NegativeTimestamp { field } => {
                write!(f, "the bid-composition field '{field}' must be non-negative")
            }
            BidCompositionError::InvertedValidityWindow {
                granted_at_unix_ms,
                valid_until_unix_ms,
            } => write!(
                f,
                "the grant's validity window is inverted: valid_until {valid_until_unix_ms} \
                 must be strictly after granted_at {granted_at_unix_ms} (end-exclusive, §10.3)"
            ),
            BidCompositionError::ControlModeInsufficient { mode_name } => write!(
                f,
                "the control-authority mode '{mode_name}' is not '{REQUIRED_CONTROL_MODE_NAME}'; \
                 no bid is composed outside operator-approved mode"
            ),
            BidCompositionError::GrantNotActive {
                grant_id,
                grant_state,
            } => write!(
                f,
                "the per-market grant {grant_id} is {grant_state}, not ACTIVE — no bid is \
                 composed without an active explicit grant"
            ),
            BidCompositionError::GrantExpired {
                grant_id,
                now_unix_ms,
                valid_until_unix_ms,
            } => write!(
                f,
                "the per-market grant {grant_id} expired at {valid_until_unix_ms} (now \
                 {now_unix_ms}) — no bid is composed under an expired grant"
            ),
            BidCompositionError::GrantTenantMismatch { grant_id } => write!(
                f,
                "the per-market grant {grant_id} is scoped to another tenant (tenant \
                 scoping, P0.16 rule 5)"
            ),
            BidCompositionError::GrantMarketMismatch { grant_id } => write!(
                f,
                "the per-market grant {grant_id} names a different market structure than the \
                 composition targets"
            ),
            BidCompositionError::MissingEnvelopeProvenance { asset_id } => write!(
                f,
                "the asset {asset_id} supplied no envelope-version reference — the envelope \
                 is the physical truth layer and a bid without provenance is refused"
            ),
            BidCompositionError::InvalidQuantity {
                asset_id,
                interval_index,
            } => write!(
                f,
                "the asset {asset_id} carries an invalid quantity at interval {interval_index} \
                 (an offer is a finite non-negative magnitude)"
            ),
            BidCompositionError::IntervalCountMismatch {
                asset_id,
                expected,
                found,
            } => write!(
                f,
                "the asset {asset_id} carries {found} intervals; the bid window declares \
                 {expected}"
            ),
            BidCompositionError::NoAssetInputs => {
                write!(f, "no asset inputs were supplied — a bid of nothing is not a bid")
            }
            BidCompositionError::InvalidMarketIdentifier { field } => write!(
                f,
                "the market identifier '{field}' violates its shape (non-empty, bounded)"
            ),
        }
    }
}

impl std::error::Error for BidCompositionError {}

// --- Market structures (jurisdiction record-pack data) ------------------------------

/// The market-structure declaration: jurisdiction record-pack DATA, one
/// of the three structures the plan names (ADR-011 — a fourth enters as
/// a NEW variant in a record pack, never by widening these).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketStructureDeclaration {
    /// PJM: all asset contributions aggregate to ONE aggregation node
    /// [S-90].
    PjmSingleAggregationNode {
        aggregation_node_ref: String,
    },
    /// CAISO PDR: two-ID load-shift bidding [S-96]. The two identifier
    /// ROLES are structural; the market-facing semantics stay with the
    /// record pack (secondary-source specifics UNVERIFIED — the
    /// decision record carries the marking).
    CaisoPdrTwoIdLoadShift {
        pdr_resource_id: String,
        load_shift_id: String,
    },
    /// ISO-NE DAM: per-asset entries under the Asset tab [S-103].
    IsoNeDamAssetTab {
        asset_tab_registration_ref: String,
    },
}

impl MarketStructureDeclaration {
    /// The stable structure name (the grant names THIS name).
    pub fn name(&self) -> &'static str {
        match self {
            MarketStructureDeclaration::PjmSingleAggregationNode { .. } => "PJM_SINGLE_AGGREGATION_NODE",
            MarketStructureDeclaration::CaisoPdrTwoIdLoadShift { .. } => "CAISO_PDR_TWO_ID_LOAD_SHIFT",
            MarketStructureDeclaration::IsoNeDamAssetTab { .. } => "ISO_NE_DAM_ASSET_TAB",
        }
    }

    fn validate_identifiers(&self) -> Result<(), BidCompositionError> {
        match self {
            MarketStructureDeclaration::PjmSingleAggregationNode {
                aggregation_node_ref,
            } => validate_identifier("aggregation_node_ref", aggregation_node_ref),
            MarketStructureDeclaration::CaisoPdrTwoIdLoadShift {
                pdr_resource_id,
                load_shift_id,
            } => {
                validate_identifier("pdr_resource_id", pdr_resource_id)?;
                validate_identifier("load_shift_id", load_shift_id)
            }
            MarketStructureDeclaration::IsoNeDamAssetTab {
                asset_tab_registration_ref,
            } => validate_identifier("asset_tab_registration_ref", asset_tab_registration_ref),
        }
    }
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), BidCompositionError> {
    if value.trim().is_empty() {
        return Err(BidCompositionError::InvalidMarketIdentifier { field });
    }
    if value.len() > 256 {
        return Err(BidCompositionError::InvalidMarketIdentifier { field });
    }
    Ok(())
}

// --- The per-market grant (a RECORD; the permission surface is P16.19's) -------------

/// The closed grant-state vocabulary.
pub const MARKET_GRANT_STATES: [&str; 3] = ["ACTIVE", "REVOKED", "EXPIRED"];

/// One explicit per-market bid grant: the RECORD an operator (or the
/// deployment's composition root) issues to authorize composing bids for
/// ONE market structure under operator-approved mode. The grant is
/// recorded data; the market.* permission actions that would back it in
/// the authz registry are P16.19's (proposed, never registered here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketBidGrant {
    grant_id: String,
    tenant_id: String,
    /// The market-structure name the grant covers (a
    /// `MarketStructureDeclaration::name()` spelling).
    market_structure_name: String,
    state: String,
    granted_by: String,
    granted_at_unix_ms: i64,
    /// The grant validity end, END-EXCLUSIVE (unix millis).
    valid_until_unix_ms: i64,
    /// The P11 audit-chain LINK of the audit record appended when the
    /// grant was issued (financial-adjacent record — the P16.01
    /// treatment).
    audit_record_id: String,
}

impl MarketBidGrant {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        grant_id: String,
        tenant_id: String,
        market_structure_name: String,
        state: String,
        granted_by: String,
        granted_at_unix_ms: i64,
        valid_until_unix_ms: i64,
        audit_record_id: String,
    ) -> Result<Self, BidCompositionError> {
        for (field, value) in [
            ("grant_id", grant_id.as_str()),
            ("tenant_id", tenant_id.as_str()),
            ("market_structure_name", market_structure_name.as_str()),
            ("granted_by", granted_by.as_str()),
            ("audit_record_id", audit_record_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(BidCompositionError::EmptyField { field });
            }
        }
        if state.trim().is_empty() {
            return Err(BidCompositionError::EmptyField { field: "state" });
        }
        if !MARKET_GRANT_STATES.contains(&state.as_str()) {
            return Err(BidCompositionError::InvalidEnumValue {
                field: "state",
                value: state.clone(),
            });
        }
        if granted_at_unix_ms < 0 {
            return Err(BidCompositionError::NegativeTimestamp {
                field: "granted_at_unix_ms",
            });
        }
        if valid_until_unix_ms <= granted_at_unix_ms {
            return Err(BidCompositionError::InvertedValidityWindow {
                granted_at_unix_ms,
                valid_until_unix_ms,
            });
        }
        Ok(Self {
            grant_id,
            tenant_id,
            market_structure_name,
            state,
            granted_by,
            granted_at_unix_ms,
            valid_until_unix_ms,
            audit_record_id,
        })
    }

    pub fn grant_id(&self) -> &str {
        &self.grant_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn market_structure_name(&self) -> &str {
        &self.market_structure_name
    }

    pub fn state(&self) -> &str {
        &self.state
    }

    pub fn valid_until_unix_ms(&self) -> i64 {
        self.valid_until_unix_ms
    }
}

// --- Inputs -----------------------------------------------------------------------

/// One asset's bid input: quantities DERIVED FROM the asset's flexibility
/// envelope (the physical truth layer) by the caller, with the
/// envelope-version reference carried FORCIBLY — no provenance, no bid.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetBidInput {
    pub asset_id: String,
    /// The envelope-version reference the quantities were derived from.
    pub envelope_ref: String,
    /// The per-interval offer magnitudes in MW (finite, non-negative;
    /// one value per interval of the bid window).
    pub interval_quantities_mw: Vec<f64>,
}

// --- The composed record -----------------------------------------------------------

/// One composed bid interval (start END-INCLUSIVE convention noted: the
/// window is [start, end) end-exclusive, §10.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BidInterval {
    pub interval_start_millis: i64,
    pub interval_end_millis: i64,
    /// The aggregated (or per-asset, for ISO-NE) quantity in MW.
    pub quantity_mw: f64,
}

/// The composed bid record — an internal RECORD of what WOULD be bid.
/// No bid is submitted — none can be (`NO_BID_SUBMITTED_STATEMENT`).
#[derive(Debug, Clone, PartialEq)]
pub enum ComposedBidRecord {
    /// PJM: ONE aggregated bid at the single aggregation node [S-90].
    PjmAggregationBid {
        aggregation_node_ref: String,
        intervals: Vec<BidInterval>,
        /// The contributing asset ids (ascending, deduplicated — the
        /// determinism contract).
        contributing_assets: Vec<String>,
        /// The envelope-version references (ascending, deduplicated).
        envelope_refs: Vec<String>,
    },
    /// CAISO PDR: the aggregated load-shift bid under the two recorded
    /// IDs [S-96].
    CaisoPdrLoadShiftBid {
        pdr_resource_id: String,
        load_shift_id: String,
        intervals: Vec<BidInterval>,
        contributing_assets: Vec<String>,
        envelope_refs: Vec<String>,
    },
    /// ISO-NE DAM: per-asset entries under the Asset tab [S-103] — the
    /// per-asset quantities stay PER ASSET (no cross-asset aggregation).
    IsoNeDamAssetBid {
        asset_tab_registration_ref: String,
        /// The per-asset entries (ascending asset_id — deterministic).
        per_asset: Vec<IsoNeAssetTabEntry>,
    },
}

/// One ISO-NE Asset-tab entry (per-asset quantities and provenance).
#[derive(Debug, Clone, PartialEq)]
pub struct IsoNeAssetTabEntry {
    pub asset_id: String,
    pub envelope_ref: String,
    pub intervals: Vec<BidInterval>,
}

// --- The composer -------------------------------------------------------------------

/// Validates the grant + mode preconditions (shared by all structures).
fn validate_grant_and_mode(
    control_mode_name: &str,
    grant: &MarketBidGrant,
    tenant_id: &str,
    structure: &MarketStructureDeclaration,
    now_unix_ms: i64,
) -> Result<(), BidCompositionError> {
    if control_mode_name != REQUIRED_CONTROL_MODE_NAME {
        return Err(BidCompositionError::ControlModeInsufficient {
            mode_name: control_mode_name.to_string(),
        });
    }
    if grant.tenant_id() != tenant_id {
        return Err(BidCompositionError::GrantTenantMismatch {
            grant_id: grant.grant_id().to_string(),
        });
    }
    if grant.market_structure_name() != structure.name() {
        return Err(BidCompositionError::GrantMarketMismatch {
            grant_id: grant.grant_id().to_string(),
        });
    }
    if grant.state() != "ACTIVE" {
        return Err(BidCompositionError::GrantNotActive {
            grant_id: grant.grant_id().to_string(),
            grant_state: grant.state().to_string(),
        });
    }
    if now_unix_ms >= grant.valid_until_unix_ms {
        return Err(BidCompositionError::GrantExpired {
            grant_id: grant.grant_id().to_string(),
            now_unix_ms,
            valid_until_unix_ms: grant.valid_until_unix_ms,
        });
    }
    Ok(())
}

/// Validates the asset inputs (provenance, quantities, interval
/// alignment) and returns them keyed deterministically by asset id.
fn validate_asset_inputs<'a>(
    inputs: &'a [AssetBidInput],
    interval_count: usize,
) -> Result<BTreeMap<&'a str, &'a AssetBidInput>, BidCompositionError> {
    if inputs.is_empty() {
        return Err(BidCompositionError::NoAssetInputs);
    }
    let mut keyed = BTreeMap::new();
    for input in inputs {
        if input.asset_id.trim().is_empty() {
            return Err(BidCompositionError::EmptyField { field: "asset_id" });
        }
        if input.envelope_ref.trim().is_empty() {
            return Err(BidCompositionError::MissingEnvelopeProvenance {
                asset_id: input.asset_id.clone(),
            });
        }
        if input.interval_quantities_mw.len() != interval_count {
            return Err(BidCompositionError::IntervalCountMismatch {
                asset_id: input.asset_id.clone(),
                expected: interval_count,
                found: input.interval_quantities_mw.len(),
            });
        }
        for (index, quantity) in input.interval_quantities_mw.iter().enumerate() {
            if !quantity.is_finite() || *quantity < 0.0 {
                return Err(BidCompositionError::InvalidQuantity {
                    asset_id: input.asset_id.clone(),
                    interval_index: index,
                });
            }
        }
        if keyed.insert(input.asset_id.as_str(), input).is_some() {
            return Err(BidCompositionError::EmptyField { field: "asset_id" });
        }
    }
    Ok(keyed)
}

/// Composes the aggregation-level bid per the market structure. Time
/// enters ONLY through the injected clock (grant-expiry and composed-at
/// checks). Deterministic: BTreeMap asset ordering, ascending outputs.
///
/// `interval_bounds` is the bid window's per-interval [start, end)
/// bounds (end-exclusive, §10.3) — one pair per interval.
pub fn compose_bid(
    structure: &MarketStructureDeclaration,
    grant: &MarketBidGrant,
    control_mode_name: &str,
    tenant_id: &str,
    interval_bounds: &[(i64, i64)],
    inputs: &[AssetBidInput],
    clock: &dyn Clock,
) -> Result<ComposedBidRecord, BidCompositionError> {
    structure.validate_identifiers()?;
    let event_time = clock.now_event_time();
    let event_millis = event_time.unix_millis();
    if event_millis < 0 || event_millis > i64::MAX as i128 {
        return Err(BidCompositionError::EmptyField {
            field: "clock_now_unix_ms",
        });
    }
    let now_unix_ms = event_millis as i64;
    validate_grant_and_mode(control_mode_name, grant, tenant_id, structure, now_unix_ms)?;
    if interval_bounds.is_empty() {
        return Err(BidCompositionError::EmptyField {
            field: "interval_bounds",
        });
    }
    for (start, end) in interval_bounds {
        if *start < 0 || *end <= *start {
            return Err(BidCompositionError::EmptyField {
                field: "interval_bounds",
            });
        }
    }
    let keyed = validate_asset_inputs(inputs, interval_bounds.len())?;

    let interval_summaries: Vec<BidInterval> = interval_bounds
        .iter()
        .enumerate()
        .map(|(index, (start, end))| {
            let quantity: f64 = keyed.values().map(|input| input.interval_quantities_mw[index]).sum();
            BidInterval {
                interval_start_millis: *start,
                interval_end_millis: *end,
                quantity_mw: quantity,
            }
        })
        .collect();
    let contributing_assets: Vec<String> = keyed.keys().map(|k| k.to_string()).collect();
    let mut envelope_set = std::collections::BTreeSet::new();
    for input in keyed.values() {
        envelope_set.insert(input.envelope_ref.clone());
    }
    let envelope_refs: Vec<String> = envelope_set.into_iter().collect();

    match structure {
        MarketStructureDeclaration::PjmSingleAggregationNode {
            aggregation_node_ref,
        } => Ok(ComposedBidRecord::PjmAggregationBid {
            aggregation_node_ref: aggregation_node_ref.clone(),
            intervals: interval_summaries,
            contributing_assets,
            envelope_refs,
        }),
        MarketStructureDeclaration::CaisoPdrTwoIdLoadShift {
            pdr_resource_id,
            load_shift_id,
        } => Ok(ComposedBidRecord::CaisoPdrLoadShiftBid {
            pdr_resource_id: pdr_resource_id.clone(),
            load_shift_id: load_shift_id.clone(),
            intervals: interval_summaries,
            contributing_assets,
            envelope_refs,
        }),
        MarketStructureDeclaration::IsoNeDamAssetTab {
            asset_tab_registration_ref,
        } => {
            // ISO-NE: per-asset entries stay PER ASSET (no cross-asset
            // aggregation) [S-103]; the aggregated summaries computed
            // above are NOT used.
            let _ = interval_summaries;
            let per_asset = keyed
                .values()
                .map(|input| IsoNeAssetTabEntry {
                    asset_id: input.asset_id.clone(),
                    envelope_ref: input.envelope_ref.clone(),
                    intervals: input
                        .interval_quantities_mw
                        .iter()
                        .zip(interval_bounds.iter())
                        .map(|(quantity, (start, end))| BidInterval {
                            interval_start_millis: *start,
                            interval_end_millis: *end,
                            quantity_mw: *quantity,
                        })
                        .collect(),
                })
                .collect();
            Ok(ComposedBidRecord::IsoNeDamAssetBid {
                asset_tab_registration_ref: asset_tab_registration_ref.clone(),
                per_asset,
            })
        }
    }
}
