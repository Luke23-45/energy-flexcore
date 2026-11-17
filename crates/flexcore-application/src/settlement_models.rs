// P16.09 — Settlement ingestion: jurisdiction statement MODEL TYPES and
//         parser SPECIFICATIONS with synthetic-fixture grammars. No real
//         statement is parsed — none exists here.
//
// Phase 16 plan §2 row P16.09; sources: [S-93] (PJM Manual 29 settlement
// formats — CSV/XML statement shapes and the MSRS/BLI statement class),
// [S-97] (CAISO Tariff §11 settlements as of Nov 1 2024 — the 5-minute
// Settlement Quality Meter Data class), [S-107] (EBGL (EU) 2017/2195 —
// the imbalance-settlement regulation, Art. 52–55 15-minute ISP),
// [S-108] (ACER Decision 18-2020 ISHM — the imbalance-settlement
// methodology class), [S-109] (RTE 15-minute ISP transition — the
// migration context).
//
// WHAT THIS IS (decided once):
//
// - **Jurisdiction-specific RECORDS, never a universal statement engine**
//   (ADR-011): the three modeled formats are separate record packs with
//   named sources. There is no "generic settlement statement" type that
//   claims to work for any market; a fourth jurisdiction is a NEW pack.
// - **Parser SPECS, not parsers of real statements.** Each extraction
//   function implements a DECLARED fixture grammar — a documented,
//   deliberately-narrow subset of the format (the v1 subset is recorded
//   per grammar below). No real statement has ever been received (no
//   market is connected; conventions §7.3); every fixture in the tests
//   is SYNTHETIC and hand-built. A format-conformance claim is NOT made
//   — the risk register (R-P16-2) records that real statements encode
//   tariff detail no fixture grammar captures.
// - **Wire-level vocabulary is carried verbatim with UNVERIFIED labels**:
//   charge-type codes, quality flags, and column names are recorded as
//   the source's vocabulary (UNVERIFIED until checked against the
//   primary manual at implementation-for-use time). Nothing is invented.
// - **Money and metered quantities are exact decimals** ([`crate::fixed_decimal`]):
//   no settlement number is ever a float, and none is claimed — every
//   value in the tests is a synthetic fixture value with no market
//   meaning.
// - **Persistence is a SEAM consumed by citation**: the market-records
//   store is Agent A's P16.01 deliverable (the phase-16 wave-1 split;
//   migration 0022). This module declares NO store trait of its own —
//   the reconciliation layer ([`crate::settlement_reconciliation`])
//   references the statement record types; the composition root binds
//   them to A's store.
//
// Control status: pure model types and pure extraction functions for the
// simulation-only 0.x line. No statement is received, parsed, or
// settled; no settlement number is claimed anywhere.

use flexcore_contracts::time_model::EventTime;

use crate::fixed_decimal::{FixedDecimal, FixedDecimalError, MoneyAmount};

/// The settlement-ingestion model schema version (a change is a NEW
/// version, never a silent edit).
pub const SETTLEMENT_MODEL_SCHEMA_VERSION: u32 = 1;

/// The no-real-statement statement (pinned by tests).
pub const NO_REAL_STATEMENT_PARSED_STATEMENT: &str = "No real settlement statement has \
     ever been received or parsed: no market is connected and no counterparty exists in \
     this program (conventions 7.3; ADR-011). Every fixture in the parser tests is \
     SYNTHETIC and hand-built; no settlement number carries market meaning; no \
     format-conformance claim is made (risk R-P16-2: real statements encode tariff \
     detail no fixture grammar captures).";

/// The jurisdiction-specificity statement (pinned by tests).
pub const JURISDICTION_SPECIFIC_STATEMENT: &str = "The settlement model is \
     jurisdiction-specific RECORDS (ADR-011): PJM Manual 29 CSV/XML + MSRS/BLI [S-93], \
     CAISO 5-minute SQMD [S-97], and EBGL Art. 52-55 15-minute ISP [S-107][S-108][S-109] \
     are separate packs with named sources. There is no universal statement type and no \
     generic tariff adapter.";

/// The closed jurisdiction-format vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettlementFormat {
    /// PJM Manual 29 statement formats: CSV and XML statement files with
    /// MSRS Billing Line Items [S-93].
    PjmManual29,
    /// CAISO settlement quality meter data at 5-minute resolution
    /// (Tariff §11 as of Nov 1 2024) [S-97].
    CaisoSqmd,
    /// EBGL imbalance statements on the 15-minute imbalance settlement
    /// period (Art. 52–55) [S-107][S-108][S-109].
    EbglIsp,
}

impl SettlementFormat {
    /// The canonical label spelling (records and audit details).
    pub const fn as_str(self) -> &'static str {
        match self {
            SettlementFormat::PjmManual29 => "pjm-manual-29",
            SettlementFormat::CaisoSqmd => "caiso-sqmd",
            SettlementFormat::EbglIsp => "ebgl-isp",
        }
    }
}

/// Why a statement extraction was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementParseError {
    /// The statement text was empty or whitespace.
    EmptyInput,
    /// A required header/structure element was absent or unrecognized
    /// (the fixture grammar is the declared v1 subset; anything else is
    /// a named refusal, never a guess).
    StructureUnrecognized { detail: String },
    /// A decimal field failed to parse (the fixed-decimal error rides).
    DecimalInvalid { field: &'static str, reason: String },
    /// An interval was not the format's declared length (CAISO SQMD is a
    /// 5-minute record; the EBGL ISP is 15 minutes) or was inverted.
    IntervalInvalid { expected_millis: i64 },
    /// A currency code was empty or malformed (the money-record rule).
    CurrencyInvalid { reason: String },
    /// A required reference/identifier field was empty.
    EmptyReference { field: &'static str },
    /// The node seam carried no text where the grammar requires text.
    MissingNodeText { node: &'static str },
}

impl SettlementParseError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            SettlementParseError::EmptyInput => "SETTLEMENT_PARSE_EMPTY_INPUT",
            SettlementParseError::StructureUnrecognized { .. } => {
                "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED"
            }
            SettlementParseError::DecimalInvalid { .. } => "SETTLEMENT_PARSE_DECIMAL_INVALID",
            SettlementParseError::IntervalInvalid { .. } => "SETTLEMENT_PARSE_INTERVAL_INVALID",
            SettlementParseError::CurrencyInvalid { .. } => "SETTLEMENT_PARSE_CURRENCY_INVALID",
            SettlementParseError::EmptyReference { .. } => "SETTLEMENT_PARSE_EMPTY_REFERENCE",
            SettlementParseError::MissingNodeText { .. } => "SETTLEMENT_PARSE_MISSING_NODE_TEXT",
        }
    }
}

impl std::fmt::Display for SettlementParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettlementParseError::EmptyInput => write!(f, "the statement text is empty"),
            SettlementParseError::StructureUnrecognized { detail } => write!(
                f,
                "the statement structure is outside the declared v1 fixture grammar: \
                 {detail} — refused, never guessed"
            ),
            SettlementParseError::DecimalInvalid { field, reason } => {
                write!(f, "the '{field}' decimal failed to parse: {reason}")
            }
            SettlementParseError::IntervalInvalid { expected_millis } => write!(
                f,
                "the interval is not the format's declared {expected_millis} ms length \
                 (or is inverted); the record is refused, never re-binned"
            ),
            SettlementParseError::CurrencyInvalid { reason } => {
                write!(f, "the currency code is invalid: {reason}")
            }
            SettlementParseError::EmptyReference { field } => {
                write!(f, "the '{field}' reference must be a non-empty string")
            }
            SettlementParseError::MissingNodeText { node } => {
                write!(f, "the XML seam node '{node}' carries no text where the \
                           grammar requires text")
            }
        }
    }
}

impl std::error::Error for SettlementParseError {}

// --- 1. PJM Manual 29 / MSRS Billing Line Items [S-93] -------------------------

/// One MSRS Billing Line Item as a MODEL TYPE (the BLI class per [S-93]).
/// The charge-type vocabulary is the SOURCE's, carried verbatim
/// (UNVERIFIED until checked against Manual 29 at use) — never invented,
/// never normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PjmBliLineItem {
    /// The billing line item's identity as published (verbatim).
    pub line_ref: String,
    /// The charge type as published (verbatim source vocabulary;
    /// UNVERIFIED per-source until the manual check).
    pub charge_type: String,
    /// The statement's billing amount for the line (exact decimal with
    /// declared currency — no floats).
    pub amount: MoneyAmount,
    /// The parent statement reference (verbatim).
    pub statement_ref: String,
}

/// The PJM Manual 29 CSV fixture grammar (the DECLARED v1 subset — see
/// the module header for the no-conformance statement):
///
/// - Line 1: a header row whose FIRST cell is exactly `statement_ref`
///   and whose SECOND cell is exactly `charge_type` (the declared column
///   grammar; real M29 column sets are UNVERIFIED and richer — a
///   mismatch is [`SettlementParseError::StructureUnrecognized`]).
/// - Lines 2..: `statement_ref,charge_type,amount,currency` — exactly
///   four cells, comma-separated.
/// - v1 LIMITATION (recorded): no quoted-cell grammar, no escaped
///   commas, no embedded newlines. A cell containing a comma is
///   structurally impossible here; a real statement needing quotes is
///   OUTSIDE the v1 subset and is refused, never approximated.
///
/// Empty statements (header only) are refused — an empty statement is a
/// named refusal, never a clean zero bill.
pub fn extract_pjm_m29_bli_csv(
    text: &str,
) -> Result<Vec<PjmBliLineItem>, SettlementParseError> {
    if text.trim().is_empty() {
        return Err(SettlementParseError::EmptyInput);
    }
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| SettlementParseError::StructureUnrecognized {
            detail: "the statement carries no header row".to_string(),
        })?;
    let header_cells: Vec<&str> = header.split(',').collect();
    if header_cells.first() != Some(&"statement_ref")
        || header_cells.get(1) != Some(&"charge_type")
    {
        return Err(SettlementParseError::StructureUnrecognized {
            detail: format!("the header row '{header}' does not start with the declared \
                             statement_ref,charge_type grammar"),
        });
    }
    let mut items = Vec::new();
    for (row_index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split(',').collect();
        if cells.len() != 4 {
            return Err(SettlementParseError::StructureUnrecognized {
                detail: format!("row {row_index} carries {} cells; the declared grammar \
                                 is exactly 4", cells.len()),
            });
        }
        let statement_ref = cells[0].trim();
        let charge_type = cells[1].trim();
        let amount_text = cells[2].trim();
        let currency = cells[3].trim();
        if statement_ref.is_empty() {
            return Err(SettlementParseError::EmptyReference {
                field: "statement_ref",
            });
        }
        if charge_type.is_empty() {
            return Err(SettlementParseError::EmptyReference { field: "charge_type" });
        }
        let amount = MoneyAmount::parse(amount_text, currency.to_string()).map_err(|e| {
            SettlementParseError::DecimalInvalid {
                field: "amount",
                reason: e.to_string(),
            }
        })?;
        items.push(PjmBliLineItem {
            line_ref: format!("{statement_ref}#{row_index}"),
            charge_type: charge_type.to_string(),
            amount,
            statement_ref: statement_ref.to_string(),
        });
    }
    if items.is_empty() {
        return Err(SettlementParseError::StructureUnrecognized {
            detail: "the statement carries a header but no billing line items".to_string(),
        });
    }
    Ok(items)
}

/// The internal XML node SEAM (the protocol-entsoe `xml_seam` precedent):
/// a minimal node type filled by the composition root's parser. This
/// module performs NO XML parsing — no parser library exists in this
/// crate; the extraction below is pure logic over the filled tree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatementXmlNode {
    /// The element name (verbatim).
    pub name: String,
    /// The attributes, in document order.
    pub attrs: Vec<(String, String)>,
    /// The element's text content, if any.
    pub text: Option<String>,
    /// The child elements, in document order.
    pub children: Vec<StatementXmlNode>,
}

impl StatementXmlNode {
    /// The first child with the given element name (exact match).
    pub fn child(&self, name: &str) -> Option<&StatementXmlNode> {
        self.children.iter().find(|c| c.name == name)
    }

    /// All children with the given element name (exact match), in
    /// document order.
    pub fn children_named(&self, name: &str) -> Vec<&StatementXmlNode> {
        self.children.iter().filter(|c| c.name == name).collect()
    }

    /// The attribute value for the given name (exact match).
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// The PJM Manual 29 XML fixture grammar (the DECLARED v1 subset):
///
/// ```text
/// <statement ref="...">
///   <bli ref="..." charge_type="...">amount</bli>
/// </statement>
/// ```
///
/// - The root element must be named `statement` and carry a non-empty
///   `ref` attribute (real M29 XML element sets are UNVERIFIED and
///   richer — a mismatch is [`SettlementParseError::StructureUnrecognized`]).
/// - Each `bli` child carries `ref` + `charge_type` attributes and its
///   text content is the amount as published; the currency is declared
///   per statement by the `currency` attribute on the root (the v1
///   grammar; a per-line currency is outside the subset and refused).
/// - A `bli` with no text content is a named refusal.
pub fn extract_pjm_m29_bli_xml(
    root: &StatementXmlNode,
) -> Result<Vec<PjmBliLineItem>, SettlementParseError> {
    if root.name != "statement" {
        return Err(SettlementParseError::StructureUnrecognized {
            detail: format!("the root element is '{}'; the declared grammar is 'statement'",
                            root.name),
        });
    }
    let statement_ref = root
        .attr("ref")
        .ok_or(SettlementParseError::StructureUnrecognized {
            detail: "the statement element carries no 'ref' attribute".to_string(),
        })?
        .to_string();
    if statement_ref.trim().is_empty() {
        return Err(SettlementParseError::EmptyReference {
            field: "statement_ref",
        });
    }
    let currency = root
        .attr("currency")
        .ok_or(SettlementParseError::StructureUnrecognized {
            detail: "the statement element carries no 'currency' attribute".to_string(),
        })?
        .to_string();
    let mut items = Vec::new();
    for node in root.children_named("bli") {
        let line_ref = node.attr("ref").unwrap_or("").to_string();
        if line_ref.trim().is_empty() {
            return Err(SettlementParseError::EmptyReference { field: "line_ref" });
        }
        let charge_type = node.attr("charge_type").unwrap_or("").to_string();
        if charge_type.trim().is_empty() {
            return Err(SettlementParseError::EmptyReference {
                field: "charge_type",
            });
        }
        let text = node
            .text
            .as_deref()
            .ok_or(SettlementParseError::MissingNodeText { node: "bli" })?;
        let amount = MoneyAmount::parse(text, currency.clone()).map_err(|e| {
            SettlementParseError::DecimalInvalid {
                field: "amount",
                reason: e.to_string(),
            }
        })?;
        items.push(PjmBliLineItem {
            line_ref,
            charge_type,
            amount,
            statement_ref: statement_ref.clone(),
        });
    }
    if items.is_empty() {
        return Err(SettlementParseError::StructureUnrecognized {
            detail: "the statement carries no billing line items".to_string(),
        });
    }
    Ok(items)
}

// --- 2. CAISO 5-minute SQMD [S-97] -----------------------------------------------

/// The CAISO SQMD interval length: 5 minutes in milliseconds ([S-97];
/// re-verify at use against Tariff §11).
pub const CAISO_SQMD_INTERVAL_MILLIS: i64 = 300_000;

/// One CAISO 5-minute Settlement Quality Meter Data interval as a MODEL
/// TYPE ([S-97]). The quality flag is the SOURCE's, carried verbatim
/// (UNVERIFIED per-source until the tariff check); the metered quantity
/// is an exact decimal in the declared unit-as-published basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaisoSqmdIntervalRecord {
    /// Start of the 5-minute settlement interval (UTC, start-inclusive).
    pub interval_start: EventTime,
    /// End of the interval (UTC, end-exclusive — must be exactly 5
    /// minutes later; re-binning is refused, never performed).
    pub interval_end: EventTime,
    /// The metered quantity as published (exact decimal, source basis).
    pub meter_quantity: FixedDecimal,
    /// The quantity basis as published (e.g. the metered-energy unit
    /// label; verbatim source vocabulary, UNVERIFIED per-source).
    pub quantity_basis: String,
    /// The quality flag as published (verbatim).
    pub quality_flag: String,
}

/// Extracts one CAISO SQMD interval record from the declared v1 fixture
/// grammar: `interval_start_millis,interval_end_millis,quantity,basis,quality`
/// (exactly five cells; epoch-millis integers for the interval bounds so
/// the fixture grammar stays timezone-free). The interval length is
/// validated to be exactly [`CAISO_SQMD_INTERVAL_MILLIS`].
pub fn extract_caiso_sqmd_row(text: &str) -> Result<CaisoSqmdIntervalRecord, SettlementParseError> {
    if text.trim().is_empty() {
        return Err(SettlementParseError::EmptyInput);
    }
    let cells: Vec<&str> = text.trim().split(',').collect();
    if cells.len() != 5 {
        return Err(SettlementParseError::StructureUnrecognized {
            detail: format!("the SQMD row carries {} cells; the declared grammar is \
                             exactly 5", cells.len()),
        });
    }
    let start: i64 = cells[0]
        .trim()
        .parse()
        .map_err(|_| SettlementParseError::StructureUnrecognized {
            detail: "interval_start_millis is not an integer".to_string(),
        })?;
    let end: i64 = cells[1]
        .trim()
        .parse()
        .map_err(|_| SettlementParseError::StructureUnrecognized {
            detail: "interval_end_millis is not an integer".to_string(),
        })?;
    if end - start != CAISO_SQMD_INTERVAL_MILLIS {
        return Err(SettlementParseError::IntervalInvalid {
            expected_millis: CAISO_SQMD_INTERVAL_MILLIS,
        });
    }
    let quantity = FixedDecimal::parse(cells[2].trim()).map_err(|e| {
        SettlementParseError::DecimalInvalid {
            field: "meter_quantity",
            reason: e.to_string(),
        }
    })?;
    let basis = cells[3].trim();
    if basis.is_empty() {
        return Err(SettlementParseError::EmptyReference {
            field: "quantity_basis",
        });
    }
    let quality = cells[4].trim();
    if quality.is_empty() {
        return Err(SettlementParseError::EmptyReference { field: "quality_flag" });
    }
    Ok(CaisoSqmdIntervalRecord {
        interval_start: EventTime::from_unix_millis(start)
            .map_err(|_| SettlementParseError::StructureUnrecognized {
                detail: "interval_start is not representable".to_string(),
            })?,
        interval_end: EventTime::from_unix_millis(end)
            .map_err(|_| SettlementParseError::StructureUnrecognized {
                detail: "interval_end is not representable".to_string(),
            })?,
        meter_quantity: quantity,
        quantity_basis: basis.to_string(),
        quality_flag: quality.to_string(),
    })
}

// --- 3. EBGL 15-minute ISP imbalance [S-107][S-108][S-109] -------------------------

/// The EBGL imbalance settlement period: 15 minutes in milliseconds
/// ([S-107] Art. 52–55; the 15-minute transition context is [S-109];
/// re-verify at use).
pub const EBGL_ISP_MILLIS: i64 = 900_000;

/// The declared imbalance-direction sign convention (the reactive-sign /
/// declared-convention pattern): the direction is DERIVED from the sign
/// of the imbalance energy under a DECLARED convention, never assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImbalanceDirection {
    /// Metered energy exceeded the scheduled energy under the declared
    /// convention (positive side of the declared sign).
    Surplus,
    /// Metered energy fell below the scheduled energy (negative side).
    Deficit,
}

impl ImbalanceDirection {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            ImbalanceDirection::Surplus => "surplus",
            ImbalanceDirection::Deficit => "deficit",
        }
    }
}

/// One EBGL 15-minute ISP imbalance record as a MODEL TYPE ([S-107]).
/// The imbalance energy is an exact decimal in MWh (the canonical energy
/// base); the direction is derived from the declared sign convention
/// (positive = surplus). Zero imbalance is carried (a zero-imbalance ISP
/// is real data — it is BOTH-signed-neutral and derives to `Surplus`
/// with a zero magnitude; the magnitude is part of the record, never
/// discarded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EbglIspImbalanceRecord {
    /// Start of the 15-minute ISP (UTC, start-inclusive).
    pub isp_start: EventTime,
    /// End of the ISP (UTC, end-exclusive — exactly 15 minutes later).
    pub isp_end: EventTime,
    /// The imbalance energy for the ISP, in MWh, as published (exact
    /// decimal; sign carried verbatim).
    pub imbalance_energy_mwh: FixedDecimal,
    /// The derived direction under the declared sign convention.
    pub direction: ImbalanceDirection,
}

/// Extracts one EBGL ISP record from the declared v1 fixture grammar:
/// `isp_start_millis,isp_end_millis,imbalance_mwh` (exactly three cells).
/// The ISP length is validated to be exactly [`EBGL_ISP_MILLIS`].
pub fn extract_ebgl_isp_row(text: &str) -> Result<EbglIspImbalanceRecord, SettlementParseError> {
    if text.trim().is_empty() {
        return Err(SettlementParseError::EmptyInput);
    }
    let cells: Vec<&str> = text.trim().split(',').collect();
    if cells.len() != 3 {
        return Err(SettlementParseError::StructureUnrecognized {
            detail: format!(
                "the ISP row carries {} cells; the declared grammar is exactly 3",
                cells.len()
            ),
        });
    }
    let start: i64 = cells[0]
        .trim()
        .parse()
        .map_err(|_| SettlementParseError::StructureUnrecognized {
            detail: "isp_start_millis is not an integer".to_string(),
        })?;
    let end: i64 = cells[1]
        .trim()
        .parse()
        .map_err(|_| SettlementParseError::StructureUnrecognized {
            detail: "isp_end_millis is not an integer".to_string(),
        })?;
    if end - start != EBGL_ISP_MILLIS {
        return Err(SettlementParseError::IntervalInvalid {
            expected_millis: EBGL_ISP_MILLIS,
        });
    }
    let energy = FixedDecimal::parse(cells[2].trim()).map_err(|e| {
        SettlementParseError::DecimalInvalid {
            field: "imbalance_energy_mwh",
            reason: e.to_string(),
        }
    })?;
    let direction = if energy.mantissa() < 0 {
        ImbalanceDirection::Deficit
    } else {
        ImbalanceDirection::Surplus
    };
    Ok(EbglIspImbalanceRecord {
        isp_start: EventTime::from_unix_millis(start)
            .map_err(|_| SettlementParseError::StructureUnrecognized {
                detail: "isp_start is not representable".to_string(),
            })?,
        isp_end: EventTime::from_unix_millis(end)
            .map_err(|_| SettlementParseError::StructureUnrecognized {
                detail: "isp_end is not representable".to_string(),
            })?,
        imbalance_energy_mwh: energy,
        direction,
    })
}

// Re-export so the reconciliation layer's use of the decimal error type
// stays on one home.
pub use FixedDecimalError as SettlementDecimalError;

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every fixture below is SYNTHETIC
// and hand-built; every expected value is hand-derived from the grammar.
// No real statement exists anywhere in this program. Nothing is
// fabricated as market data.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(NO_REAL_STATEMENT_PARSED_STATEMENT.contains("No real settlement statement"));
        assert!(NO_REAL_STATEMENT_PARSED_STATEMENT.contains("SYNTHETIC"));
        assert!(JURISDICTION_SPECIFIC_STATEMENT.contains("ADR-011"));
        assert!(JURISDICTION_SPECIFIC_STATEMENT.contains("[S-93]"));
        assert!(JURISDICTION_SPECIFIC_STATEMENT.contains("[S-107]"));
    }

    #[test]
    fn format_labels_are_stable() {
        assert_eq!(SettlementFormat::PjmManual29.as_str(), "pjm-manual-29");
        assert_eq!(SettlementFormat::CaisoSqmd.as_str(), "caiso-sqmd");
        assert_eq!(SettlementFormat::EbglIsp.as_str(), "ebgl-isp");
    }

    // --- PJM M29 CSV (SYNTHETIC fixture) ---------------------------------

    #[test]
    fn pjm_csv_fixture_extracts_bli_line_items() {
        // SYNTHETIC PJM-M29-SHAPED fixture (hand-built; the charge_type
        // vocabulary is fixture-local, never a real M29 code).
        let fixture = "\
statement_ref,charge_type,amount,currency
SYNTH-STMT-1,SYN_CHARGE_A,125.75,USD
SYNTH-STMT-1,SYN_CHARGE_B,-40.50,USD
";
        let items = extract_pjm_m29_bli_csv(fixture).expect("valid fixture");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].line_ref, "SYNTH-STMT-1#0");
        assert_eq!(items[0].charge_type, "SYN_CHARGE_A");
        assert_eq!(items[0].amount.canonical_string(), "125.75 USD");
        assert_eq!(items[1].amount.canonical_string(), "-40.50 USD");
    }

    #[test]
    fn pjm_csv_refuses_malformed_structures() {
        // Wrong header grammar -> named refusal, never guessed columns.
        let err = extract_pjm_m29_bli_csv("a,b,c\n1,2,3,4")
            .expect_err("wrong header refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
        // Wrong cell count.
        let err = extract_pjm_m29_bli_csv(
            "statement_ref,charge_type,amount,currency\nS,C,1.0",
        )
        .expect_err("3-cell row refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
        // Header-only statement (no line items) is refused.
        let err = extract_pjm_m29_bli_csv("statement_ref,charge_type,amount,currency")
            .expect_err("header-only refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
        // Empty input.
        let err = extract_pjm_m29_bli_csv("   ").expect_err("empty refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_EMPTY_INPUT");
        // Bad decimal.
        let err = extract_pjm_m29_bli_csv(
            "statement_ref,charge_type,amount,currency\nS,C,1x0,USD",
        )
        .expect_err("bad decimal refused");
        assert!(matches!(err, SettlementParseError::DecimalInvalid { .. }));
        // Bad currency (the money-record rule rides).
        let err = extract_pjm_m29_bli_csv(
            "statement_ref,charge_type,amount,currency\nS,C,1.0, ",
        )
        .expect_err("empty currency refused");
        assert!(matches!(err, SettlementParseError::DecimalInvalid { .. }));
    }

    // --- PJM M29 XML (SYNTHETIC fixture, node seam) ----------------------

    fn fixture_xml_root() -> StatementXmlNode {
        StatementXmlNode {
            name: "statement".to_string(),
            attrs: vec![
                ("ref".to_string(), "SYNTH-STMT-XML-1".to_string()),
                ("currency".to_string(), "USD".to_string()),
            ],
            text: None,
            children: vec![
                StatementXmlNode {
                    name: "bli".to_string(),
                    attrs: vec![
                        ("ref".to_string(), "BLI-1".to_string()),
                        ("charge_type".to_string(), "SYN_CHARGE_A".to_string()),
                    ],
                    text: Some("99.95".to_string()),
                    children: Vec::new(),
                },
                StatementXmlNode {
                    name: "bli".to_string(),
                    attrs: vec![
                        ("ref".to_string(), "BLI-2".to_string()),
                        ("charge_type".to_string(), "SYN_CHARGE_B".to_string()),
                    ],
                    text: Some("-12.05".to_string()),
                    children: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn pjm_xml_fixture_extracts_bli_line_items() {
        let items = extract_pjm_m29_bli_xml(&fixture_xml_root()).expect("valid fixture");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].statement_ref, "SYNTH-STMT-XML-1");
        assert_eq!(items[0].line_ref, "BLI-1");
        assert_eq!(items[0].amount.canonical_string(), "99.95 USD");
        assert_eq!(items[1].amount.canonical_string(), "-12.05 USD");
    }

    #[test]
    fn pjm_xml_refuses_malformed_structures() {
        let mut root = fixture_xml_root();
        root.name = "not-a-statement".to_string();
        let err = extract_pjm_m29_bli_xml(&root).expect_err("wrong root refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");

        let mut root = fixture_xml_root();
        root.attrs.clear();
        let err = extract_pjm_m29_bli_xml(&root)
            .expect_err("missing ref attribute refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");

        let mut root = fixture_xml_root();
        root.children[0].text = None;
        let err = extract_pjm_m29_bli_xml(&root)
            .expect_err("missing amount text refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_MISSING_NODE_TEXT");

        let mut root = fixture_xml_root();
        root.children.clear();
        let err = extract_pjm_m29_bli_xml(&root)
            .expect_err("statement without line items refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
    }

    // --- CAISO SQMD (SYNTHETIC fixture) -----------------------------------

    #[test]
    fn caiso_sqmd_fixture_extracts_five_minute_records() {
        // SYNTHETIC CAISO-SQMD-SHAPED fixture (hand-built; basis and
        // quality vocabulary are fixture-local, never real CAISO codes).
        let row = "0,300000,0.125,kWh-upr,SYN-QF";
        let record = extract_caiso_sqmd_row(row).expect("valid fixture");
        assert_eq!(record.interval_start.unix_millis(), 0);
        assert_eq!(
            record.interval_end.unix_millis() - record.interval_start.unix_millis(),
            CAISO_SQMD_INTERVAL_MILLIS
        );
        assert_eq!(record.meter_quantity.canonical_string(), "0.125");
        assert_eq!(record.quantity_basis, "kWh-upr");
        assert_eq!(record.quality_flag, "SYN-QF");
    }

    #[test]
    fn caiso_sqmd_refuses_wrong_interval_length_and_cells() {
        // A 4-minute interval is refused, never re-binned to 5 minutes.
        let err = extract_caiso_sqmd_row("0,240000,0.125,kWh-upr,SYN-QF")
            .expect_err("wrong interval refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_INTERVAL_INVALID");
        // An inverted interval is refused by the same rule.
        let err = extract_caiso_sqmd_row("300000,0,0.125,kWh-upr,SYN-QF")
            .expect_err("inverted interval refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_INTERVAL_INVALID");
        // Wrong cell count.
        let err = extract_caiso_sqmd_row("0,300000,0.125,kWh-upr")
            .expect_err("4-cell row refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
        // Empty quality flag.
        let err = extract_caiso_sqmd_row("0,300000,0.125,kWh-upr, ")
            .expect_err("empty quality refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_EMPTY_REFERENCE");
    }

    // --- EBGL ISP (SYNTHETIC fixture) --------------------------------------

    #[test]
    fn ebgl_isp_fixture_extracts_fifteen_minute_records_with_direction() {
        // SYNTHETIC EBGL-ISP-SHAPED fixture (hand-built).
        let row = "0,900000,0.250";
        let record = extract_ebgl_isp_row(row).expect("valid fixture");
        assert_eq!(record.direction, ImbalanceDirection::Surplus);
        assert_eq!(record.imbalance_energy_mwh.canonical_string(), "0.250");
        // Negative imbalance derives to Deficit (declared convention).
        let record = extract_ebgl_isp_row("900000,1800000,-0.250").expect("valid fixture");
        assert_eq!(record.direction, ImbalanceDirection::Deficit);
        // Zero imbalance is carried (never discarded as trivial).
        let record = extract_ebgl_isp_row("1800000,2700000,0").expect("valid fixture");
        assert!(record.imbalance_energy_mwh.is_zero());
        assert_eq!(record.direction, ImbalanceDirection::Surplus);
    }

    #[test]
    fn ebgl_isp_refuses_wrong_period_length_and_cells() {
        // A 60-minute ISP (the pre-transition period) is refused, never
        // re-binned to 15 minutes [S-109].
        let err = extract_ebgl_isp_row("0,3600000,0.250")
            .expect_err("60-minute ISP refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_INTERVAL_INVALID");
        let err = extract_ebgl_isp_row("0,900000,0.250,extra")
            .expect_err("4-cell row refused");
        assert_eq!(err.name(), "SETTLEMENT_PARSE_STRUCTURE_UNRECOGNIZED");
        let err = extract_ebgl_isp_row("0,900000,x")
            .expect_err("bad decimal refused");
        assert!(matches!(err, SettlementParseError::DecimalInvalid { .. }));
    }

    #[test]
    fn every_parse_error_renders_non_empty_prose() {
        let errors = [
            SettlementParseError::EmptyInput,
            SettlementParseError::StructureUnrecognized {
                detail: "d".to_string(),
            },
            SettlementParseError::DecimalInvalid {
                field: "amount",
                reason: "r".to_string(),
            },
            SettlementParseError::IntervalInvalid {
                expected_millis: 300_000,
            },
            SettlementParseError::CurrencyInvalid {
                reason: "r".to_string(),
            },
            SettlementParseError::EmptyReference { field: "f" },
            SettlementParseError::MissingNodeText { node: "bli" },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
