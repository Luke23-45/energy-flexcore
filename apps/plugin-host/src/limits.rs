// P7.10 — Resource limits derived from the manifest's declared bounds
// (report §5.4 Class B: "memory, fuel/time, concurrency, output-size, and
// call-rate limits"; ledger `ledger_phase_7.md` §P7.10 step 3; decision
// record `10_11_plugin_host_broker.md` §3).
//
// Source of the bounds: the validated plugin manifest's `resources` object.
// The kernel-side type is `crates/flexcore-kernel/src/manifest.rs::
// ResourceBounds` (`max-memory-mb`, `timeout-ms`, `max-output-mb`,
// `max-calls-per-minute`, with the kernel's closed ranges). This crate
// mirrors that STRUCT as [`DeclaredBounds`] and re-validates it against the
// SAME closed ranges — it does NOT import the kernel type, because this
// crate's non-wasmtime code is deliberately std + tracing only (the
// alignment seam is recorded in the decision record §6; the kernel crate is
// owned by another wave and was not touched).
//
// Every limit has an enforcement CONTRACT (stated on the matching
// [`crate::host::HostError`] variant): a limit exceeded => named error +
// the instance torn down, the host process survives. A limit that exists in
// config but is never applied to the instance is exactly the P7.13 risk —
// the application points are named in the gated bindings implementation
// (`bindings.rs` §feature-gated, UNVERIFIED wasmtime call sites).
//
// Defaults are DEFAULT-labelled decisions, recorded in the decision record
// §3 — they are host-side caps for manifest fields the v1 schema leaves
// optional, never silent permissiveness.

use std::fmt;
use std::time::Duration;

use crate::host::HostError;

// --- Constants -----------------------------------------------------------------

/// One WebAssembly memory page: 64 KiB (fixed by the wasm spec).
pub const WASM_PAGE_BYTES: u64 = 65_536;

/// Pages per MiB (1 MiB / 64 KiB = 16) — the memory bound converts exactly,
/// so no rounding exists anywhere in the conversion.
pub const WASM_PAGES_PER_MIB: u64 = (1024 * 1024) / WASM_PAGE_BYTES;

/// The epoch ticker interval, in milliseconds (DEFAULT decision, record §3).
///
/// §epoch — the wall-clock deadline pattern (documented from the wasmtime
/// docs; UNVERIFIED — wasmtime 50 API, first human build verifies):
///
/// 1. The host runs a ticker thread/future that calls
///    `Engine::increment_epoch()` every `EPOCH_TICK_MILLIS` milliseconds.
/// 2. At instantiation the store sets
///    `Store::set_epoch_deadline(ResourceLimits::epoch_ticks())`.
/// 3. A guest that runs past its declared wall-clock deadline yields
///    control at the next epoch boundary and is torn down with
///    [`HostError::DeadlineExceeded`] — the host process is never blocked
///    by a looping guest (the §P7.13 "loops forever" hostile component
///    fails safely against this).
///
/// The epoch pattern wasmtime documents is the chosen mechanism because it
/// bounds a guest's wall-clock use WITHOUT a preemptive kill: the guest
/// yields at a trap/engine boundary, the instance is dropped, and the host
/// continues.
pub const EPOCH_TICK_MILLIS: u64 = 10;

/// The output-size cap applied when the manifest declares no
/// `max-output-mb` (DEFAULT decision, record §3): 64 MiB per call.
pub const DEFAULT_MAX_OUTPUT_MB: u64 = 64;

/// The call-rate cap applied when the manifest declares no
/// `max-calls-per-minute` (DEFAULT decision, record §3): 60 calls/minute.
pub const DEFAULT_MAX_CALLS_PER_MINUTE: u64 = 60;

/// The concurrency cap (DEFAULT decision, record §3): ONE concurrent call
/// per instance. The v1 manifest schema declares no concurrency field
/// (kernel `ResourceBounds` has none — recorded seam); the host fails
/// closed to the tightest cap, and promoting this to deployment
/// configuration is a reviewed follow-up.
pub const DEFAULT_MAX_CONCURRENT_CALLS: u32 = 1;

// The kernel's closed ranges, restated for the host-side re-validation
// (alignment seam, record §6 — same values as
// `crates/flexcore-kernel/src/manifest.rs::parse_resources`).
const MAX_MEMORY_MB_MIN: u64 = 1;
const MAX_MEMORY_MB_MAX: u64 = 65_536;
const TIMEOUT_MS_MIN: u64 = 1;
const TIMEOUT_MS_MAX: u64 = 3_600_000;
const MAX_OUTPUT_MB_MIN: u64 = 1;
const MAX_OUTPUT_MB_MAX: u64 = 4_096;
const MAX_CALLS_PER_MINUTE_MIN: u64 = 1;
const MAX_CALLS_PER_MINUTE_MAX: u64 = 1_000_000;

// --- Declared bounds (the mirror of the kernel's ResourceBounds) ----------------

/// The manifest's declared resource bounds — the structural mirror of
/// `crates/flexcore-kernel/src/manifest.rs::ResourceBounds` (alignment
/// seam, record §6; the kernel type is NOT imported: std + tracing only).
///
/// Construction is fail-closed the same way the kernel's validator is: an
/// out-of-range value never becomes limits ([`ResourceLimits::from_declared_bounds`]
/// re-validates against the SAME closed ranges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredBounds {
    /// `max-memory-mb` (required, 1..=65_536).
    pub max_memory_mb: u64,
    /// `timeout-ms` (required, 1..=3_600_000).
    pub timeout_ms: u64,
    /// `max-output-mb` (optional, 1..=4_096; absent => [`DEFAULT_MAX_OUTPUT_MB`]).
    pub max_output_mb: Option<u64>,
    /// `max-calls-per-minute` (optional, 1..=1_000_000; absent =>
    /// [`DEFAULT_MAX_CALLS_PER_MINUTE`]).
    pub max_calls_per_minute: Option<u64>,
}

// --- Resource limits ------------------------------------------------------------

/// The host's effective resource limits for ONE component instance, derived
/// from the manifest's declared bounds plus the deployment-configured fuel
/// budget. Every field is validated at construction ([`ResourceLimits::from_declared_bounds`])
/// — an invalid limit is [`HostError::LimitsInvalid`], never a silent
/// clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Memory ceiling in wasm pages (from `max-memory-mb`; 1 MiB = 16 pages,
    /// exact conversion). Enforcement: exceeded => [`HostError::MemoryLimitExceeded`],
    /// instance torn down.
    max_memory_pages: u64,
    /// Wall-clock deadline in milliseconds (from `timeout-ms`).
    /// Enforcement: exceeded => [`HostError::DeadlineExceeded`], instance
    /// torn down.
    wall_clock_deadline_ms: u64,
    /// The same deadline in epoch ticks (wasmtime epoch-based interruption,
    /// see [`EPOCH_TICK_MILLIS`] §epoch). Derived, never configured
    /// separately — one deadline, one source of truth.
    epoch_ticks: u64,
    /// Per-call output ceiling in bytes (from `max-output-mb`, or the
    /// DEFAULT cap when the manifest omits it). Enforcement: exceeded =>
    /// [`HostError::OutputLimitExceeded`], call refused, instance torn down.
    max_output_bytes: u64,
    /// Call-rate ceiling per minute (from `max-calls-per-minute`, or the
    /// DEFAULT cap when the manifest omits it). Enforcement: exceeded =>
    /// [`HostError::CallRateExceeded`], further calls refused, instance
    /// torn down.
    max_calls_per_minute: u64,
    /// Concurrency ceiling (DEFAULT 1 — see [`DEFAULT_MAX_CONCURRENT_CALLS`]).
    max_concurrent_calls: u32,
    /// Fuel (metered instruction) budget per instance — DEPLOYMENT
    /// CONFIGURATION, mandatory (record §3): the v1 manifest schema declares
    /// no fuel field (kernel `ResourceBounds` has none — a manifest-side
    /// fuel declaration would need a kernel change, not made here; recorded
    /// seam). There is NO default fuel budget: an unmetered instance is not
    /// constructible. Enforcement: exhausted => [`HostError::FuelExhausted`],
    /// instance torn down.
    fuel_budget: u64,
}

impl ResourceLimits {
    /// Derives the limits from the manifest's declared bounds plus the
    /// mandatory deployment-configured fuel budget. Fail-closed:
    ///
    /// - every declared bound is re-validated against the kernel's closed
    ///   ranges (a manifest the kernel accepted cannot fail here; a
    ///   hand-built struct can — that is the point of re-validation);
    /// - the page conversion is exact (16 pages per MiB) and checked;
    /// - `fuel_budget` must be non-zero (there is no default fuel budget —
    ///   an unmetered instance is refused at construction, not at first
    ///   runaway).
    pub fn from_declared_bounds(
        bounds: &DeclaredBounds,
        fuel_budget: u64,
    ) -> Result<ResourceLimits, HostError> {
        if !(MAX_MEMORY_MB_MIN..=MAX_MEMORY_MB_MAX).contains(&bounds.max_memory_mb) {
            return Err(HostError::LimitsInvalid {
                field: "max-memory-mb".to_string(),
                reason: format!(
                    "must be between {MAX_MEMORY_MB_MIN} and {MAX_MEMORY_MB_MAX} (the kernel manifest validator's closed range)"
                ),
            });
        }
        if !(TIMEOUT_MS_MIN..=TIMEOUT_MS_MAX).contains(&bounds.timeout_ms) {
            return Err(HostError::LimitsInvalid {
                field: "timeout-ms".to_string(),
                reason: format!(
                    "must be between {TIMEOUT_MS_MIN} and {TIMEOUT_MS_MAX} (the kernel manifest validator's closed range)"
                ),
            });
        }
        if let Some(mb) = bounds.max_output_mb {
            if !(MAX_OUTPUT_MB_MIN..=MAX_OUTPUT_MB_MAX).contains(&mb) {
                return Err(HostError::LimitsInvalid {
                    field: "max-output-mb".to_string(),
                    reason: format!(
                        "must be between {MAX_OUTPUT_MB_MIN} and {MAX_OUTPUT_MB_MAX} (the kernel manifest validator's closed range)"
                    ),
                });
            }
        }
        if let Some(rate) = bounds.max_calls_per_minute {
            if !(MAX_CALLS_PER_MINUTE_MIN..=MAX_CALLS_PER_MINUTE_MAX).contains(&rate) {
                return Err(HostError::LimitsInvalid {
                    field: "max-calls-per-minute".to_string(),
                    reason: format!(
                        "must be between {MAX_CALLS_PER_MINUTE_MIN} and {MAX_CALLS_PER_MINUTE_MAX} (the kernel manifest validator's closed range)"
                    ),
                });
            }
        }
        if fuel_budget == 0 {
            return Err(HostError::LimitsInvalid {
                field: "fuel-budget".to_string(),
                reason: "no fuel budget configured — metering is mandatory and there is no default budget (an unmetered instance is not constructible)".to_string(),
            });
        }
        let max_memory_pages = bounds
            .max_memory_mb
            .checked_mul(WASM_PAGES_PER_MIB)
            .ok_or_else(|| HostError::LimitsInvalid {
                field: "max-memory-mb".to_string(),
                reason: "page conversion overflowed (bound above the closed range cannot reach this)".to_string(),
            })?;
        let max_output_mb = bounds.max_output_mb.unwrap_or(DEFAULT_MAX_OUTPUT_MB);
        let max_output_bytes = max_output_mb
            .checked_mul(1024 * 1024)
            .ok_or_else(|| HostError::LimitsInvalid {
                field: "max-output-mb".to_string(),
                reason: "byte conversion overflowed (bound above the closed range cannot reach this)".to_string(),
            })?;
        Ok(ResourceLimits {
            max_memory_pages,
            wall_clock_deadline_ms: bounds.timeout_ms,
            epoch_ticks: epoch_ticks_for(bounds.timeout_ms),
            max_output_bytes,
            max_calls_per_minute: bounds
                .max_calls_per_minute
                .unwrap_or(DEFAULT_MAX_CALLS_PER_MINUTE),
            max_concurrent_calls: DEFAULT_MAX_CONCURRENT_CALLS,
            fuel_budget,
        })
    }

    /// Memory ceiling in wasm pages.
    pub fn max_memory_pages(&self) -> u64 {
        self.max_memory_pages
    }

    /// Wall-clock deadline in milliseconds.
    pub fn wall_clock_deadline_ms(&self) -> u64 {
        self.wall_clock_deadline_ms
    }

    /// The wall-clock deadline as a `Duration` (host-side waiting/supervision;
    /// the in-guest enforcement is the epoch deadline below).
    pub fn wall_clock_deadline(&self) -> Duration {
        Duration::from_millis(self.wall_clock_deadline_ms)
    }

    /// The deadline in epoch ticks (the `Store::set_epoch_deadline` value;
    /// see [`EPOCH_TICK_MILLIS`] §epoch).
    pub fn epoch_ticks(&self) -> u64 {
        self.epoch_ticks
    }

    /// Per-call output ceiling in bytes.
    pub fn max_output_bytes(&self) -> u64 {
        self.max_output_bytes
    }

    /// Call-rate ceiling per minute.
    pub fn max_calls_per_minute(&self) -> u64 {
        self.max_calls_per_minute
    }

    /// Concurrency ceiling.
    pub fn max_concurrent_calls(&self) -> u32 {
        self.max_concurrent_calls
    }

    /// Fuel (metered instruction) budget per instance.
    pub fn fuel_budget(&self) -> u64 {
        self.fuel_budget
    }
}

impl fmt::Display for ResourceLimits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "memory {} pages ({} MiB), deadline {} ms ({} epoch ticks at {} ms/tick), output {} bytes, call rate {}/minute, concurrency {}, fuel {}",
            self.max_memory_pages,
            self.max_memory_pages / WASM_PAGES_PER_MIB,
            self.wall_clock_deadline_ms,
            self.epoch_ticks,
            EPOCH_TICK_MILLIS,
            self.max_output_bytes,
            self.max_calls_per_minute,
            self.max_concurrent_calls,
            self.fuel_budget,
        )
    }
}

/// Converts a wall-clock deadline in milliseconds into epoch ticks
/// ([`EPOCH_TICK_MILLIS`] ms per tick). A non-zero deadline always yields
/// at least ONE tick (a sub-tick deadline is rounded UP — fail-closed: a
/// guest must never run a full extra tick because its deadline rounded
/// down). A zero deadline is the caller's bug ([`ResourceLimits::from_declared_bounds`]
/// rejects it via the closed range); this function documents the rounding
/// and stays total.
pub fn epoch_ticks_for(deadline_ms: u64) -> u64 {
    if deadline_ms == 0 {
        return 1;
    }
    deadline_ms.div_ceil(EPOCH_TICK_MILLIS).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_per_mib_is_exact() {
        assert_eq!(WASM_PAGES_PER_MIB, 16);
        assert_eq!(WASM_PAGES_PER_MIB * WASM_PAGE_BYTES, 1024 * 1024);
    }

    #[test]
    fn epoch_ticks_round_up() {
        assert_eq!(epoch_ticks_for(1), 1);
        assert_eq!(epoch_ticks_for(10), 1);
        assert_eq!(epoch_ticks_for(11), 2);
        assert_eq!(epoch_ticks_for(3_600_000), 360_000);
    }
}
