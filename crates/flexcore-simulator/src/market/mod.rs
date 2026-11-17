// P16.20 — The paper-trading market simulator (decision record
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 62_p16_20_market_simulator.md`).
//
// PURPOSE, stated plainly (the P14.17 simulated-counterpart boundary,
// carried to the market tier): the full market loop — SIGNAL → BID →
// DISPATCH → EVIDENCE → STATEMENT → RECONCILIATION → PAYMENT-STATE
// reference — is testable with ZERO market connectivity. Every module
// here lands ON the named unwired seam its Phase 16 counterpart designed
// — and on NOTHING else:
//
// | Simulated counterpart | Module | Seam it exercises (owning record) |
// |---|---|---|
// | Simulated ISO price feed | [`price_feed`] | `flexcore_application::market_price_records::IsoPricePublication` (P16.07, record 21) |
// | Simulated settlement statements | [`settlement_fixtures`] | the P16.09 fixture grammars `extract_pjm_m29_bli_csv` / `extract_pjm_m29_bli_xml` / `extract_caiso_sqmd_row` / `extract_ebgl_isp_row` (record 23) |
// | Simulated telemetry export sink | [`export_sink`] | `flexcore_application::telemetry_export_design::TelemetryExportSink` (P16.08, record 22) |
// | The paper-trading loop itself | [`paper_loop`] | the wave-1 application engines: `compose_bid` (P16.06), `reconcile_bli_statement` (P16.09), the price-record layer (P16.07) |
//
// HARD BOUNDARY (the same wording discipline as the P14.17 fixtures):
//
// - **Code, never execution.** NOTHING here has ever run: no feed was
//   contacted, no price was published to anyone, no statement was
//   received, no bid was submitted (none can be), no telemetry was
//   exported, no payment moved. Every test that composes these modules
//   is WRITTEN-UNRUN and its execution is a human gate.
// - **Records, never a connection** (plan §8 criterion 2). There is no
//   socket, no HTTP, no FIX/ICCP/SCADA stack anywhere in this module
//   tree: the simulated feed is an in-process implementation of the
//   UNWIRED price-publication seam, the simulated sink holds an
//   in-memory `Vec`, and the settlement fixtures are in-memory strings
//   and node trees. The simulator produces records only.
// - **Never a real market, never presentable as one.** Every identity
//   string here carries a `sim://market` / `sim-export://` prefix (the
//   P14.17 identity rule): a record produced FROM the simulator is
//   identifiable from its identity alone, forever. No settlement number
//   carries market meaning; every amount is a deterministic synthetic
//   fixture value.
// - **No money arithmetic in floats** (plan §4; hard ban). Every price,
//   amount, quantity, and imbalance value in this tree is generated in
//   INTEGER arithmetic (tenths / hundredths) and rendered as
//   wire-faithful decimal text; the decimal parsing itself stays in
//   `flexcore_application::fixed_decimal` at the record boundary. There
//   is no float→money conversion path, and no RNG: every value is a
//   pure function of its integer inputs (the determinism rule — same
//   config + same clock ⇒ byte-identical records).
// - **Dispatch stays paper** (the P14.16 command-relay-only rule,
//   consumed by citation). The loop's dispatch leg produces a
//   `PaperDispatchInstruction` — a RECORD of what WOULD be relayed. A
//   real instruction enters ONLY through the 14-state command lifecycle
//   with all three gates distinct; the simulator never issues, never
//   authorizes, never touches the lifecycle.
// - **Composition-class placement** (the P14.17 placement decision,
//   mirrored): `flexcore-simulator` is a COMPOSITION crate in
//   `tools/check_dependency_rule.py`, so the composition→application
//   edge this tree adds (`flexcore-application` in the crate manifest)
//   is legal without any checker change. Nothing may depend ON this
//   crate except apps/composition roots.

pub mod export_sink;
pub mod paper_loop;
pub mod price_feed;
pub mod settlement_fixtures;
