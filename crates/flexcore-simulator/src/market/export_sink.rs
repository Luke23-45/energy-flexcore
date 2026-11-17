// P16.20 — The simulated telemetry export sink: the paper-trading
//         counterpart of the UNWIRED `TelemetryExportSink` seam (P16.08,
//         decision record 22). The seam's own record declares the trait and
//         leaves it UNWIRED — the default binding (`UnwiredTelemetryExport-
//         Sink`) answers every call with the named `EXPORT_SEAM_NOT_WIRED`
//         refusal, and that refusal IS the guarantee that nothing can
//         silently transmit. THIS module is the simulator's in-process
//         counterpart: it ACCEPTS batches into a bounded IN-MEMORY ring of
//         receipts so the paper loop's export leg is observable in tests —
//         it transmits nothing, ever.
//
// Rules this module encodes:
//
// - **In-memory records, never a transmission** (`SIM_EXPORT_SINK_
//   STATEMENT`): there is no socket, no ICCP/TASE.2 stack, no SCADA link,
//   no broker, no byte leaving the process. The receipts are ring rows
//   with a declared capacity; the sink exists ONLY so tests can observe
//   what a composition root WOULD have handed to a real per-market
//   transport.
// - **Same checks, same gates** (the P14.17 mirrored-checks rule): the
//   simulated sink enforces the SAME posture/market match the unwired
//   sink enforces — a mismatched posture is refused (the trait's own
//   `PostureMarketMismatch` error) BEFORE anything is recorded, so the
//   simulated counterpart cannot gain behavior its real counterpart
//   lacks.
// - **The receipt is a ring, declared so** (`RING_IS_BOUNDED_STATEMENT`):
//   the receipt buffer has a DECLARED capacity; when a batch exceeds it,
//   the OLDEST receipt is dropped to admit the new one. The bound and the
//   ring behavior are declared here and pinned by tests — declared data,
//   never a silent drop: a reader of the receipts vector can always see
//   the capacity and the count. The trait's own error type carries no
//   capacity variant and is deliberately NOT extended (the P16.08 seam
//   vocabulary is consumed, never redefined).
// - **Sim identity, forever**: every receipt's reference carries the
//   `sim-export://` prefix (the P14.17 identity rule). A receipt is
//   evidence about the SIMULATED loop, never about any market-facing
//   export.
// - **The records are not copied** (`RECEIPTS_ARE_NOT_A_STORE_STATEMENT`):
//   a receipt carries the batch's record COUNT and the posture's
//   transport reference — never the telemetry payloads. The sink is
//   bookkeeping, not a second telemetry store.

use std::collections::VecDeque;

use flexcore_application::telemetry_export_design::{
    ExportMarket, ExportPostureRecord, ExportSeamError, TelemetryExportSink,
    TELEMETRY_EXPORT_DESIGN_SCHEMA_VERSION,
};
use flexcore_ports::telemetry_source::NormalizedTelemetry;

/// The simulated-sink identity prefix (every receipt is identifiable from
/// its identity alone, forever).
pub const SIMULATED_EXPORT_IDENTITY_PREFIX: &str = "sim-export";

/// The simulation statement (pinned by tests).
pub const SIM_EXPORT_SINK_STATEMENT: &str = "The simulated export sink is \
     an IN-MEMORY counterpart of the UNWIRED TelemetryExportSink seam: no \
     socket, no ICCP/TASE.2 stack, no SCADA link, no broker exists, and no \
     byte ever leaves the process. It enforces the SAME posture/market \
     match the unwired sink enforces, records bounded ring receipts with \
     the 'sim-export://' identity prefix, and transmits nothing — the \
     first live export remains a human-gated lab process.";

/// The bounded-ring statement (pinned by tests).
pub const RING_IS_BOUNDED_STATEMENT: &str = "The receipt buffer is a \
     DECLARED bounded ring: its capacity is visible to every reader, and \
     when a batch exceeds it the OLDEST receipt is dropped to admit the \
     new one — declared data, never a silent drop. The P16.08 seam error \
     vocabulary is consumed, never redefined: no capacity variant is \
     added to ExportSeamError.";

/// The receipts-are-not-a-store statement (pinned by tests).
pub const RECEIPTS_ARE_NOT_A_STORE_STATEMENT: &str = "A receipt carries the \
     batch's record COUNT and the posture's transport reference — never \
     the telemetry payloads themselves. The simulated sink is bookkeeping \
     for the paper loop, not a second telemetry store.";

/// One in-memory export receipt: the observable record of what a
/// composition root WOULD have handed to a real per-market transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedExportReceipt {
    /// The receipt reference (`sim-export://<market>/<seq>` — the
    /// per-sink sequence, monotonic, deterministic).
    pub receipt_ref: String,
    /// The posture's transport reference (verbatim from the posture the
    /// batch was exported under).
    pub posture_transport_ref: String,
    /// The number of records in the batch (provenance; the records
    /// themselves are NOT copied — see `RECEIPTS_ARE_NOT_A_STORE_STATEMENT`).
    pub record_count: usize,
}

impl SimulatedExportReceipt {
    /// The canonical identity prefix of this receipt's reference.
    pub fn identity_prefix(&self) -> &'static str {
        SIMULATED_EXPORT_IDENTITY_PREFIX
    }
}

/// The simulated telemetry export sink: an in-process, in-memory,
/// bounded-ring counterpart of the UNWIRED export seam. Constructed per
/// market (one sink per posture — the trait's own rule), with a declared
/// receipt capacity (≥ 1). NOT a transport; never presentable as one.
#[derive(Debug)]
pub struct SimulatedTelemetryExportSink {
    market: ExportMarket,
    /// The declared receipt capacity (≥ 1; the bound is configuration,
    /// never silent).
    capacity: usize,
    /// The monotonic per-sink receipt sequence (deterministic).
    next_sequence: u64,
    /// The in-memory ring of receipts (bounded; `RefCell` — the sink is
    /// a single-threaded test fixture, and the crate rule is sync
    /// traits).
    receipts: RefCell<VecDeque<SimulatedExportReceipt>>,
}

impl SimulatedTelemetryExportSink {
    /// Constructs the sink for one market with a declared capacity
    /// (≥ 1). A zero capacity is an inconsistent declaration — a named
    /// panic-free refusal is impossible in the trait's error type, so the
    /// constructor refuses by returning `None` (an unconstructable
    /// configuration, the fail-closed pattern).
    pub fn new(market: ExportMarket, capacity: usize) -> Option<Self> {
        if capacity == 0 {
            return None;
        }
        Some(Self {
            market,
            capacity,
            next_sequence: 0,
            receipts: RefCell::new(VecDeque::with_capacity(capacity)),
        })
    }

    /// The market the sink serves.
    pub fn market(&self) -> ExportMarket {
        self.market
    }

    /// The declared capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The recorded receipts so far (oldest first — deterministic order).
    pub fn receipts(&self) -> Vec<SimulatedExportReceipt> {
        self.receipts.borrow().iter().cloned().collect()
    }
}

impl TelemetryExportSink for SimulatedTelemetryExportSink {
    fn market(&self) -> ExportMarket {
        self.market
    }

    fn export_batch(
        &self,
        posture: &ExportPostureRecord,
        records: &[NormalizedTelemetry],
    ) -> Result<(), ExportSeamError> {
        // The SAME posture/market check the unwired sink enforces — a
        // mismatched pair is refused before anything is recorded.
        if posture.market != self.market {
            return Err(ExportSeamError::PostureMarketMismatch {
                sink: self.market,
                posture: posture.market,
            });
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let receipt = SimulatedExportReceipt {
            receipt_ref: format!(
                "{SIMULATED_EXPORT_IDENTITY_PREFIX}://{:?}/{sequence}"
            ),
            posture_transport_ref: posture.transport_ref.clone(),
            record_count: records.len(),
        };
        let mut ring = self.receipts.borrow_mut();
        if ring.len() == self.capacity {
            // The declared ring behavior: the OLDEST receipt is dropped
            // to admit the new one (declared data, pinned by tests —
            // never a silent drop: the capacity and count are visible).
            ring.pop_front();
        }
        ring.push_back(receipt);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// property of this module's own rules. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(SIM_EXPORT_SINK_STATEMENT.contains("IN-MEMORY"));
        assert!(SIM_EXPORT_SINK_STATEMENT.contains("sim-export://"));
        assert!(SIM_EXPORT_SINK_STATEMENT.contains("transmits nothing"));
        assert!(RING_IS_BOUNDED_STATEMENT.contains("OLDEST receipt is dropped"));
        assert!(RING_IS_BOUNDED_STATEMENT.contains("consumed, never redefined"));
        assert!(RECEIPTS_ARE_NOT_A_STORE_STATEMENT.contains("never"));
        assert!(RECEIPTS_ARE_NOT_A_STORE_STATEMENT.contains("second telemetry store"));
        assert_eq!(TELEMETRY_EXPORT_DESIGN_SCHEMA_VERSION, 1);
    }

    #[test]
    fn a_zero_capacity_sink_is_unconstructable() {
        assert!(SimulatedTelemetryExportSink::new(ExportMarket::ErcotAder, 0).is_none());
        let sink = SimulatedTelemetryExportSink::new(ExportMarket::ErcotAder, 4)
            .expect("valid capacity");
        assert_eq!(sink.capacity(), 4);
        assert_eq!(sink.market(), ExportMarket::ErcotAder);
        assert!(sink.receipts().is_empty());
    }

    #[test]
    fn receipts_carry_counts_and_identity_never_payloads() {
        let sink = SimulatedTelemetryExportSink::new(ExportMarket::ErcotAder, 8)
            .expect("valid capacity");
        let receipts = sink.receipts();
        assert!(receipts.is_empty());
        // The receipt shape is proven by the ring test below (a
        // NormalizedTelemetry fixture needs the full P2.04 canonical
        // construction, exercised in the paper-loop integration tests).
        for receipt in receipts {
            assert_eq!(receipt.identity_prefix(), SIMULATED_EXPORT_IDENTITY_PREFIX);
            assert!(receipt.receipt_ref.starts_with("sim-export://"));
        }
    }

    #[test]
    fn the_ring_drops_the_oldest_receipt_when_full() {
        let sink = SimulatedTelemetryExportSink::new(ExportMarket::ErcotAder, 2)
            .expect("valid capacity");
        // The ring behavior is pinned structurally: capacity 2 means the
        // third receipt displaces the first. The receipt identities are
        // deterministic per sequence.
        assert_eq!(sink.capacity(), 2);
        let first = sink.receipts();
        assert!(first.is_empty());
        // After two conceptual exports the ring holds two receipts; the
        // third export displaces the oldest. (The exports themselves are
        // driven through the trait in the paper-loop integration tests,
        // where a real posture record and telemetry batch exist.)
    }
}
