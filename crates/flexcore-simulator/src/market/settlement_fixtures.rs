// P16.20 — The simulated settlement statements: the paper-trading
//         counterparts of the P16.09 fixture grammars (decision record 23:
//         `23_p16_09_settlement_ingestion.md`). The P16.09 module declares
//         DELIBERATELY NARROW v1 fixture grammars and REFUSES everything
//         outside them; THIS module generates synthetic statements IN
//         exactly those grammars, so the statement → parse → reconcile leg
//         of the paper loop (P16.20) is testable with zero market
//         connectivity and zero real statements (none exists here — none
//         ever will in this program).
//
// Rules this module encodes:
//
// - **SYNTHETIC, and named so** (`SIMULATED_STATEMENTS_ARE_SYNTHETIC_
//   STATEMENT`): every statement text and node tree here is a hand-shaped
//   deterministic fixture; every amount is a synthetic fixture value with
//   NO market meaning; the identity strings carry the `sim://statement`
//   prefix. No real statement has ever been received (none exists in this
//   program) and no format-conformance claim is made in either direction
//   (the grammars are declared v1 subsets, risk R-P16-2).
// - **No money arithmetic in floats** (plan §4; hard ban): every amount
//   and quantity is generated in INTEGER HUNDREDTHS (cents) or TENTHS and
//   rendered as wire-faithful decimal text; the exact-decimal parse stays
//   at the P16.09 record boundary. There is no float anywhere in this
//   module and no RNG: the same inputs ⇒ byte-identical statement text.
// - **The grammars are consumed, never redefined**: the generated shapes
//   are exactly the v1 subsets the P16.09 module declared (PJM M29 CSV
//   `statement_ref,charge_type` header + 4-cell rows; PJM M29 XML
//   `statement[@ref][@currency]` / `bli[@ref][@charge_type]` nodes; CAISO
//   SQMD 5-cell rows with EXACTLY 300,000 ms intervals; EBGL ISP 3-cell
//   rows with EXACTLY 900,000 ms ISPs). This module adds no column, no
//   quoted cell, no second grammar.
// - **PJM line identity is exact** (the P16.09 exact-line-identity
//   discipline): the CSV generator derives `line_ref` from
//   `statement_ref#row_index` exactly as the extractor does, so a
//   simulated statement reconciles cleanly against expected lines keyed
//   the same way; a deliberately-keyed mismatch is the negative case.

use flexcore_application::settlement_models::StatementXmlNode;

/// The simulated-statement identity prefix (every generated statement is
/// identifiable from its identity alone, forever — the P14.17 identity
/// rule carried to the settlement tier).
pub const SIMULATED_STATEMENT_IDENTITY_PREFIX: &str = "sim://statement";

/// The synthetic-statement statement (pinned by tests).
pub const SIMULATED_STATEMENTS_ARE_SYNTHETIC_STATEMENT: &str = "Every \
     statement this module generates is SYNTHETIC: no real settlement \
     statement has ever been received (none exists in this program), \
     every amount is a deterministic fixture value with NO market \
     meaning, every identity carries the 'sim://statement' prefix, and no \
     format-conformance claim is made in either direction (the P16.09 \
     grammars are declared v1 subsets; risk R-P16-2).";

/// Renders integer hundredths (cents) as wire-faithful decimal text
/// (scale 2). `123456 → "1234.56"`, `-250 → "-2.50"`, `0 → "0.00"`.
/// Exact: the integer IS the value; no float ever exists.
pub fn render_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let magnitude = cents.abs();
    format!("{sign}{}.{:02}", magnitude / 100, magnitude % 100)
}

/// Renders integer tenths as wire-faithful decimal text (scale 1).
/// `4250 → "425.0"`, `-125 → "-12.5"`. Exact; no float.
pub fn render_tenths(tenths: i64) -> String {
    let sign = if tenths < 0 { "-" } else { "" };
    let magnitude = tenths.abs();
    format!("{sign}{}.{:01}", magnitude / 10, magnitude % 10)
}

/// Generates a PJM Manual 29-shaped SYNTHETIC CSV statement in the
/// declared v1 fixture grammar: the header row
/// `statement_ref,charge_type` followed by exactly 4-cell rows
/// `statement_ref,charge_type,amount,currency`. The charge types are the
/// FIXTURE AUTHOR's labels (verbatim data, never a source vocabulary
/// claim); the amounts are integer cents rendered as decimal text. The
/// `line_ref` each row will extract under is `{statement_ref}#{row_index}`
/// (the extractor's own derivation, 1-based row index after the header —
/// the extractor enumerates data rows from 0, so the FIRST data row is
/// `#0`). No quoted cell, no escaped comma: a caller asking for either is
/// outside the v1 grammar and must not (the generator provides no way to
/// build one).
pub fn simulated_pjm_csv_statement(
    statement_ref: &str,
    currency: &str,
    lines: &[(/* charge_type */ &str, /* cents */ i64)],
) -> String {
    let mut text = String::from("statement_ref,charge_type\n");
    for (charge_type, cents) in lines {
        // The extractor derives each row's line_ref from the 0-based data
        // row index (`{statement_ref}#{row_index}`), so the FIRST data row
        // extracts as `{statement_ref}#0`. No quoted cell, no escaped
        // comma: those are outside the v1 grammar and the generator
        // provides no way to build one.
        text.push_str(&format!(
            "{statement_ref},{charge_type},{},{currency}\n",
            render_cents(*cents)
        ));
    }
    text
}

/// Builds the SYNTHETIC PJM Manual 29-shaped XML node tree in the
/// declared v1 fixture grammar (`<statement ref currency>` /
/// `<bli ref charge_type>amount</bli>`), for the seam-based extractor
/// (`extract_pjm_m29_bli_xml`). No XML parser exists or is introduced:
/// this IS the filled tree, exactly as a composition-root parser would
/// hand it over.
pub fn simulated_pjm_xml_statement_tree(
    statement_ref: &str,
    currency: &str,
    lines: &[(/* line_ref */ &str, /* charge_type */ &str, /* cents */ i64)],
) -> StatementXmlNode {
    let children = lines
        .iter()
        .map(|(line_ref, charge_type, cents)| StatementXmlNode {
            name: "bli".to_string(),
            attrs: vec![
                ("ref".to_string(), (*line_ref).to_string()),
                ("charge_type".to_string(), (*charge_type).to_string()),
            ],
            text: Some(render_cents(*cents)),
            children: Vec::new(),
        })
        .collect();
    StatementXmlNode {
        name: "statement".to_string(),
        attrs: vec![
            ("ref".to_string(), statement_ref.to_string()),
            ("currency".to_string(), currency.to_string()),
        ],
        text: None,
        children,
    }
}

/// Generates CAISO SQMD-shaped SYNTHETIC rows in the declared v1 fixture
/// grammar (`interval_start_millis,interval_end_millis,quantity,basis,
/// quality`), one per 5-minute interval starting at `start_millis`. The
/// quantities are integer tenths rendered as decimal text; the basis and
/// quality labels are the FIXTURE AUTHOR's (verbatim data). The interval
/// length is EXACTLY the extractor's declared 300,000 ms — re-binning is
/// refused downstream, so the generator never produces any other length.
pub fn simulated_caiso_sqmd_rows(
    start_millis: i64,
    row_count: usize,
    quantity_tenths: i64,
    basis: &str,
    quality: &str,
) -> Vec<String> {
    (0..row_count)
        .map(|index| {
            let start = start_millis + (index as i64) * 300_000;
            let end = start + 300_000;
            format!(
                "{start},{end},{},{basis},{quality}",
                render_tenths(quantity_tenths + index as i64)
            )
        })
        .collect()
}

/// Generates EBGL ISP-shaped SYNTHETIC rows in the declared v1 fixture
/// grammar (`isp_start_millis,isp_end_millis,imbalance_mwh`), one per
/// 15-minute ISP starting at `start_millis`. The imbalance energies are
/// integer tenths rendered as decimal text (the sign pattern follows the
/// caller's per-row values; a negative value extracts as DEFICIT under
/// the declared sign convention). The ISP length is EXACTLY the
/// extractor's declared 900,000 ms — a 60-minute pre-transition ISP is
/// refused downstream, so the generator never produces one.
pub fn simulated_ebgl_isp_rows(
    start_millis: i64,
    row_count: usize,
    imbalance_tenths: i64,
) -> Vec<String> {
    (0..row_count)
        .map(|index| {
            let start = start_millis + (index as i64) * 900_000;
            let end = start + 900_000;
            // A deterministic alternating sign pattern (surplus, deficit,
            // surplus, …): exact integer arithmetic, no RNG.
            let tenths = if index % 2 == 0 {
                imbalance_tenths + index as i64
            } else {
                -(imbalance_tenths + index as i64)
            };
            format!("{start},{end},{}", render_tenths(tenths))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-derived from the declared grammars; the round-trips go through the
// P16.09 extractors, which are the single homes of the grammar rules.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_application::settlement_models::{
        extract_caiso_sqmd_row, extract_ebgl_isp_row, extract_pjm_m29_bli_csv,
        extract_pjm_m29_bli_xml,
    };

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(SIMULATED_STATEMENTS_ARE_SYNTHETIC_STATEMENT.contains("SYNTHETIC"));
        assert!(SIMULATED_STATEMENTS_ARE_SYNTHETIC_STATEMENT.contains("sim://statement"));
        assert!(SIMULATED_STATEMENTS_ARE_SYNTHETIC_STATEMENT.contains("no real settlement"));
    }

    #[test]
    fn render_cents_is_exact_decimal_text() {
        assert_eq!(render_cents(123456), "1234.56");
        assert_eq!(render_cents(-250), "-2.50");
        assert_eq!(render_cents(0), "0.00");
        assert_eq!(render_cents(5), "0.05");
    }

    #[test]
    fn the_csv_statement_round_trips_through_the_m29_grammar() {
        let statement_ref = "sim://statement/m29/csv/1";
        let text = simulated_pjm_csv_statement(
            statement_ref,
            "EUR",
            &[("ENERGY_CHARGE", 123456), ("CAPACITY_CHARGE", -250)],
        );
        let items = extract_pjm_m29_bli_csv(&text).expect("valid statement");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].line_ref, format!("{statement_ref}#0"));
        assert_eq!(items[0].charge_type, "ENERGY_CHARGE");
        assert_eq!(items[0].amount.canonical_string(), "1234.56 EUR");
        assert_eq!(items[1].line_ref, format!("{statement_ref}#1"));
        assert_eq!(items[1].amount.canonical_string(), "-2.50 EUR");
        // Determinism: the same inputs ⇒ byte-identical text.
        let again = simulated_pjm_csv_statement(
            statement_ref,
            "EUR",
            &[("ENERGY_CHARGE", 123456), ("CAPACITY_CHARGE", -250)],
        );
        assert_eq!(again, text);
    }

    #[test]
    fn the_xml_statement_tree_round_trips_through_the_m29_grammar() {
        let tree = simulated_pjm_xml_statement_tree(
            "sim://statement/m29/xml/1",
            "USD",
            &[("BLI-1", "ANCILLARY", 500), ("BLI-2", "ENERGY", -75)],
        );
        let items = extract_pjm_m29_bli_xml(&tree).expect("valid statement");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].line_ref, "BLI-1");
        assert_eq!(items[0].statement_ref, "sim://statement/m29/xml/1");
        assert_eq!(items[0].amount.canonical_string(), "5.00 USD");
        assert_eq!(items[1].amount.canonical_string(), "-0.75 USD");
    }

    #[test]
    fn the_sqmd_rows_round_trip_through_the_caiso_grammar() {
        // Quantities are INTEGER TENTHS (the generator's documented unit):
        // 4250 tenths round-trips as 425.0, and each row adds one tenth.
        let rows = simulated_caiso_sqmd_rows(1_700_000_000_000, 3, 4250, "kWh", "ESTIMATED");
        assert_eq!(rows.len(), 3);
        let first = extract_caiso_sqmd_row(&rows[0]).expect("valid row");
        assert_eq!(first.quantity_basis, "kWh");
        assert_eq!(first.quality_flag, "ESTIMATED");
        assert_eq!(first.meter_quantity.canonical_string(), "425.0");
        let third = extract_caiso_sqmd_row(&rows[2]).expect("valid row");
        assert_eq!(third.meter_quantity.canonical_string(), "425.2");
        // The interval length is EXACTLY the declared 5 minutes.
        assert_eq!(
            third.interval_end.unix_millis() - third.interval_start.unix_millis(),
            300_000
        );
    }

    #[test]
    fn the_isp_rows_round_trip_through_the_ebgl_grammar() {
        // Imbalances are INTEGER TENTHS: 9000 tenths round-trips as 900.0,
        // and the alternating sign pattern adds one tenth per row.
        let rows = simulated_ebgl_isp_rows(1_700_000_000_000, 4, 9000);
        assert_eq!(rows.len(), 4);
        let first = extract_ebgl_isp_row(&rows[0]).expect("valid row");
        assert_eq!(first.imbalance_energy_mwh.canonical_string(), "900.0");
        let second = extract_ebgl_isp_row(&rows[1]).expect("valid row");
        assert_eq!(second.imbalance_energy_mwh.canonical_string(), "-900.1");
        // The ISP length is EXACTLY the declared 15 minutes.
        assert_eq!(
            second.isp_end.unix_millis() - second.isp_start.unix_millis(),
            900_000
        );
    }

    #[test]
    fn the_identity_prefix_is_sim_only() {
        assert_eq!(SIMULATED_STATEMENT_IDENTITY_PREFIX, "sim://statement");
        let text = simulated_pjm_csv_statement(
            "sim://statement/m29/csv/2",
            "EUR",
            &[("X", 1)],
        );
        assert!(text.starts_with("statement_ref,charge_type\n"));
    }
}
