// P14.07 — Gateway polling budgets (fail-closed, registered before any
// read).
//
// A Modbus deployment reads through gateways (TCP to the gateway, RTU
// behind it). Fleet-scale reading without budgets is exactly how a
// well-meaning adapter melts a serial bus or a gateway's connection limit.
// The budget is a REVIEWED deployment declaration, validated at
// construction and asserted before every poll plan; exceeding it is a
// named refusal, never a best-effort squeeze.
//
// Budget knobs (and their UNVERIFIED anchors):
// - max_registers_per_read: the Modbus application-protocol read limit is
//   125 registers per read request (secondary-source reading of the
//   Modbus spec — UNVERIFIED; checked against the protocol text at
//   implementation).
// - devices_per_gateway: deployment-specific (bus size, gateway capacity).
// - min_poll_interval_millis: deployment-specific (RTU turnaround, device
//   load); there is NO default interval — an undeclared interval is a
//   construction error.

use serde::{Deserialize, Serialize};

/// The Modbus application-protocol read-request register limit
/// (secondary-source reading — UNVERIFIED, checked against the protocol
/// text at implementation).
pub const MAX_REGISTERS_PER_READ: u16 = 125;

/// Errors returned by budget construction and poll-plan checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetError {
    /// A budget field is empty, zero, or otherwise undeclared. There are
    /// no defaults: an undeclared budget is not a reviewed budget.
    Undeclared(&'static str),
    /// max_registers_per_read exceeds the protocol read limit.
    ExceedsProtocolReadLimit { declared: u16, limit: u16 },
    /// A requested read exceeds the budget.
    ReadTooLarge { requested: u16, budget: u16 },
    /// A poll plan would exceed the gateway's device budget.
    DeviceBudgetExceeded { requested: usize, budget: usize },
    /// A poll plan's interval is below the declared minimum.
    IntervalTooFast { requested_millis: i64, minimum_millis: i64 },
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetError::Undeclared(field) => {
                write!(f, "polling budget field '{field}' is undeclared; there are no defaults")
            }
            BudgetError::ExceedsProtocolReadLimit { declared, limit } => {
                write!(f, "declared registers-per-read {declared} exceeds the Modbus read limit {limit} (UNVERIFIED limit; re-check at implementation)")
            }
            BudgetError::ReadTooLarge { requested, budget } => {
                write!(f, "requested read of {requested} registers exceeds the budget of {budget}")
            }
            BudgetError::DeviceBudgetExceeded { requested, budget } => {
                write!(f, "poll plan covers {requested} devices; the gateway budget is {budget}")
            }
            BudgetError::IntervalTooFast { requested_millis, minimum_millis } => {
                write!(
                    f, "requested poll interval {requested_millis} ms is below the declared minimum of {minimum_millis} ms"
                )
            }
        }
    }
}

impl std::error::Error for BudgetError {}

/// The declared polling budget for one gateway. Constructed only through
/// [`GatewayPollingBudget::new`] — no `Default`, no partially-filled
/// budget (mirrors the scope-declaration discipline: declared, never
/// inferred).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayPollingBudget {
    /// Maximum registers per single read request (≤ the protocol limit).
    pub max_registers_per_read: u16,
    /// Maximum devices polled per gateway per scan.
    pub devices_per_gateway: u16,
    /// Minimum poll interval in milliseconds (declared, never defaulted).
    pub min_poll_interval_millis: i64,
}

impl GatewayPollingBudget {
    /// Validates and constructs the budget.
    pub fn new(
        max_registers_per_read: u16,
        devices_per_gateway: u16,
        min_poll_interval_millis: i64,
    ) -> Result<Self, BudgetError> {
        if max_registers_per_read == 0 {
            return Err(BudgetError::Undeclared("max_registers_per_read"));
        }
        if max_registers_per_read > MAX_REGISTERS_PER_READ {
            return Err(BudgetError::ExceedsProtocolReadLimit {
                declared: max_registers_per_read,
                limit: MAX_REGISTERS_PER_READ,
            });
        }
        if devices_per_gateway == 0 {
            return Err(BudgetError::Undeclared("devices_per_gateway"));
        }
        if min_poll_interval_millis <= 0 {
            return Err(BudgetError::Undeclared("min_poll_interval_millis"));
        }
        Ok(Self {
            max_registers_per_read,
            devices_per_gateway,
            min_poll_interval_millis,
        })
    }

    /// Asserts a planned read fits the budget.
    pub fn assert_read_fits(&self, registers: u16) -> Result<(), BudgetError> {
        if registers > self.max_registers_per_read {
            return Err(BudgetError::ReadTooLarge {
                requested: registers,
                budget: self.max_registers_per_read,
            });
        }
        Ok(())
    }

    /// Asserts a planned scan (device count + interval) fits the budget.
    pub fn assert_scan_fits(
        &self,
        devices: usize,
        interval_millis: i64,
    ) -> Result<(), BudgetError> {
        if devices > usize::from(self.devices_per_gateway) {
            return Err(BudgetError::DeviceBudgetExceeded {
                requested: devices,
                budget: usize::from(self.devices_per_gateway),
            });
        }
        if interval_millis < self.min_poll_interval_millis {
            return Err(BudgetError::IntervalTooFast {
                requested_millis: interval_millis,
                minimum_millis: self.min_poll_interval_millis,
            });
        }
        Ok(())
    }

    /// The number of reads a scan of the given register block needs, given
    /// the budget's per-read limit (a whole-block scan is split into
    /// budget-sized reads — the planning arithmetic the poller uses).
    pub fn reads_needed_for(&self, block_length_registers: u16) -> u16 {
        block_length_registers.div_ceil(self.max_registers_per_read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undeclared_budgets_are_construction_errors() {
        assert_eq!(
            GatewayPollingBudget::new(0, 8, 1_000),
            Err(BudgetError::Undeclared("max_registers_per_read"))
        );
        assert_eq!(
            GatewayPollingBudget::new(100, 0, 1_000),
            Err(BudgetError::Undeclared("devices_per_gateway"))
        );
        assert_eq!(
            GatewayPollingBudget::new(100, 8, 0),
            Err(BudgetError::Undeclared("min_poll_interval_millis"))
        );
    }

    #[test]
    fn protocol_read_limit_is_enforced_at_construction() {
        assert_eq!(
            GatewayPollingBudget::new(126, 8, 1_000),
            Err(BudgetError::ExceedsProtocolReadLimit {
                declared: 126,
                limit: 125
            })
        );
        GatewayPollingBudget::new(125, 8, 1_000).expect("exactly the limit is fine");
    }

    #[test]
    fn read_and_scan_checks_refuse_exceedances() {
        let budget = GatewayPollingBudget::new(100, 4, 2_000).expect("valid");
        assert_eq!(
            budget.assert_read_fits(101),
            Err(BudgetError::ReadTooLarge { requested: 101, budget: 100 })
        );
        budget.assert_read_fits(100).expect("fits");
        assert_eq!(
            budget.assert_scan_fits(5, 2_000),
            Err(BudgetError::DeviceBudgetExceeded { requested: 5, budget: 4 })
        );
        assert_eq!(
            budget.assert_scan_fits(4, 1_999),
            Err(BudgetError::IntervalTooFast {
                requested_millis: 1_999,
                minimum_millis: 2_000
            })
        );
        budget.assert_scan_fits(4, 2_000).expect("fits");
    }

    #[test]
    fn read_count_arithmetic_splits_blocks_into_budget_reads() {
        let budget = GatewayPollingBudget::new(100, 4, 2_000).expect("valid");
        assert_eq!(budget.reads_needed_for(100), 1);
        assert_eq!(budget.reads_needed_for(101), 2);
        assert_eq!(budget.reads_needed_for(250), 3);
    }
}
