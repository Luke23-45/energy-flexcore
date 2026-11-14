// P6.12 — The protocol-agnostic `EmsAdapter` port (report §9.1 interface
// list, §6.4; ledger P6.12 step 1).
//
// The port is deliberately protocol-shaped, not vendor-shaped: it speaks in
// points, native readings, and declared capabilities, and the anti-corruption
// layer ([`crate::adapters::acl`]) does the EMS-native → canonical mapping.
// A concrete adapter (P6.10/P6.13 for `openems-rest`) implements THIS trait;
// no protocol type ever leaks past it (§6.4).
//
// The contract, precisely:
//
// - [`EmsAdapter::discover`] returns the channel/point inventory the EMS
//   currently exposes (the OpenEMS EdgeConfig analogue, kept generic).
// - [`EmsAdapter::read`] reads ONE point and returns one
//   [`EmsNativeReading`] — a NATIVE shape (native unit, native quality
//   code, native timestamp). It is the ACL's input, never a canonical
//   record; mapping is explicit and separate (§6.4: translation happens at
//   the boundary, in one reviewable place).
// - [`EmsAdapter::capabilities`] declares what the adapter applies: the
//   declared reactive-sign convention (P0.07 §3 — required, no default),
//   the declared quality-code mapping onto the six canonical states
//   (P0.08 §3 provider rule, reusing the P2.11
//   `QualityMappingDeclaration` shape), the declared cadence, and the
//   declared clock quality (P0.09 §4 — recorded, never assumed).
// - [`EmsAdapter::health`] is the honest self-reported status (the P2.11
//   provider-health shape).
//
// **What the port deliberately does NOT define** (P6.12 step 1): dispatch,
// command forwarding, or any write path. FlexCore is supervisory; v0.4.0 is
// read-only/test-lab by default (§2.1, §21.4); the gated command receiver
// is P6.14–P6.16 behind the written enablement gate. There is no `write`,
// no `send`, no setpoint method here — absence is structural.
//
// Sync trait (house rule, `flexcore-ports` README): a concrete adapter owns
// its own I/O shape (blocking HTTP client, async WebSocket bridge, …) and
// bridges to this sync surface; the edge runner's ports stay runtime-free.

use std::time::Duration;

use flexcore_contracts::time_model::ClockQuality;
use flexcore_ports::telemetry_source::QualityMappingDeclaration;

use crate::adapters::acl::AclError;

// --- Points -----------------------------------------------------------------

/// One addressable point (channel) on the site EMS: a component id and a
/// channel id within it. The shape is generic; the concrete adapter maps its
/// protocol's own addressing onto it (for OpenEMS: component id + channel
/// id per the EdgeConfig — UNVERIFIED until P6.10, recorded as ASSUMPTION).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PointKey {
    /// The EMS-side component the channel belongs to (e.g. an ESS device id).
    pub component: String,
    /// The EMS-side channel id within the component (e.g. an active-power
    /// channel).
    pub channel: String,
}

impl PointKey {
    pub fn new(component: impl Into<String>, channel: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            channel: channel.into(),
        }
    }
}

impl std::fmt::Display for PointKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.component, self.channel)
    }
}

// --- Native reading (the ACL's input) ---------------------------------------

/// One native reading from the EMS: still in PROTOCOL terms (native unit,
/// native quality code, native timestamp). The ACL turns this into a
/// canonical [`crate::ports::EdgeTelemetrySample`] — or a named error; it
/// never passes through unmapped.
#[derive(Debug, Clone, PartialEq)]
pub struct EmsNativeReading {
    /// The point that was read (must match the mapping the ACL applies).
    pub point: PointKey,
    /// The native unit the value arrived in (parsed through
    /// [`crate::adapters::acl::NativeUnit::parse`] — an unknown unit is a
    /// named error before any conversion is attempted).
    pub native_unit: crate::adapters::acl::NativeUnit,
    /// The raw native value (finite; NaN/inf is a named ACL error).
    pub raw_value: f64,
    /// The verbatim native quality code, if the source reported one. The
    /// ACL maps it through the adapter's declared
    /// [`AdapterCapabilities::quality_mapping`]; an absent or unknown code
    /// is a named error — NEVER a default quality.
    pub native_quality_code: Option<String>,
    /// The native event timestamp (when the EMS measured it; P0.09
    /// event_time). The ACL requires UTC-representable values and refuses
    /// local-time timestamps rather than converting them.
    pub event_time: crate::adapters::acl::NativeTimestamp,
    /// Source-assigned ordering identifier (P0.09 §1).
    pub source_sequence: u64,
}

// --- Capabilities -----------------------------------------------------------

/// What an adapter declares about the source it fronts. There are no
/// defaults here: every field is a REQUIRED declaration (P0.07 §3, P0.08
/// §3, P0.09 §4 — the adapter records the mapping it applies; the platform
/// never infers it).
#[derive(Debug, Clone)]
pub struct AdapterCapabilities {
    /// The registered adapter id (e.g. `openems-rest`).
    pub adapter_id: String,
    /// The declared reactive-power sign convention of THIS source (P0.07
    /// §3: no universal constant; each device profile declares exactly
    /// one). Reactive readings are carried with this convention attached —
    /// never inverted silently, never interpreted from sign alone.
    pub reactive_sign_convention: flexcore_contracts::units::ReactiveSignConvention,
    /// The declared default emission cadence (supervisory minutes-scale;
    /// §3.5).
    pub cadence: Duration,
    /// The declared quality-code mapping: every native quality code the
    /// adapter can report → the one canonical state (of six) it maps to
    /// (P0.08 §3, the P2.11 provider-rule shape reused).
    pub quality_mapping: QualityMappingDeclaration,
    /// The declared clock-quality behaviour recorded on emissions (P0.09
    /// §4; §17.4 — `UNKNOWN` where the source does not report
    /// synchronisation; never silently assumed good).
    pub clock_quality: ClockQuality,
}

// --- Health -----------------------------------------------------------------

/// The adapter's honest self-reported health (the P2.11 provider-health
/// shape, edge-local). It is a status report, not a platform judgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterHealth {
    /// The adapter is reading normally.
    Healthy,
    /// The adapter is up but impaired; the reason is human-safe (never raw
    /// backend text — the P1.08 rule).
    Degraded { reason: String },
    /// The adapter cannot reach its EMS.
    Unreachable { reason: String },
}

// --- Discovery --------------------------------------------------------------

/// One entry of the channel/point inventory [`EmsAdapter::discover`]
/// returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDescriptor {
    /// The addressable point.
    pub point: PointKey,
    /// The native unit the point reports (parsed; an unknown unit fails
    /// discovery for that point — discovery returns a Result, and a point
    /// whose unit cannot be named is reported, not guessed).
    pub native_unit: crate::adapters::acl::NativeUnit,
    /// Human-safe description (never raw backend text; P1.08).
    pub description: String,
}

// --- Errors -----------------------------------------------------------------

/// Errors an adapter (or the ACL it calls) reports. Every failure is named;
/// nothing implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    /// The adapter has no usable connection to its EMS yet (it must NOT
    /// fabricate readings — the fail-closed rule).
    NotConnected { reason: String },
    /// Discovery failed; the inventory is unknown, not empty.
    DiscoveryFailed { reason: String },
    /// Reading the point failed; no reading exists for this attempt.
    ReadFailed { point: PointKey, reason: String },
    /// The anti-corruption layer rejected the mapping (unknown unit,
    /// undeclared quality, non-UTC timestamp, unmapped point, …). The
    /// reading is rejected WHOLE — never partially mapped.
    Acl(AclError),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterError::NotConnected { reason } => write!(f, "adapter not connected: {reason}"),
            AdapterError::DiscoveryFailed { reason } => write!(f, "discovery failed: {reason}"),
            AdapterError::ReadFailed { point, reason } => {
                write!(f, "read of point '{point}' failed: {reason}")
            }
            AdapterError::Acl(error) => write!(f, "anti-corruption layer rejected the reading: {error}"),
        }
    }
}

impl std::error::Error for AdapterError {}

impl From<AclError> for AdapterError {
    fn from(error: AclError) -> Self {
        AdapterError::Acl(error)
    }
}

// --- The port ---------------------------------------------------------------

/// The protocol-agnostic EMS adapter port (§9.1; §6.4). One adapter per D5
/// selection in this phase; `openems-rest` is the RECOMMENDED default while
/// D5 is OPEN (see the decision doc).
///
/// The read path is `discover` → (site mapping tables, P6.16's config
/// point) → `read` → ACL → canonical [`crate::ports::EdgeTelemetrySample`].
/// The adapter itself may call the ACL inside `read` — the trait permits
/// either, but the mapping tables stay explicit and reviewable either way.
pub trait EmsAdapter: Send {
    /// The registered adapter id (the registry's allowlist key).
    fn id(&self) -> &str;

    /// The declared capabilities (required declarations, no defaults).
    fn capabilities(&self) -> &AdapterCapabilities;

    /// The channel/point inventory the EMS currently exposes.
    fn discover(&mut self) -> Result<Vec<ChannelDescriptor>, AdapterError>;

    /// Reads one point and returns the NATIVE reading (the ACL maps it).
    /// A read of an unknown point is a named error, never an empty result.
    fn read(&mut self, point: &PointKey) -> Result<EmsNativeReading, AdapterError>;

    /// The honest self-reported health.
    fn health(&self) -> AdapterHealth;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::acl::NativeUnit;

    #[test]
    fn point_key_display_and_equality() {
        let a = PointKey::new("ess0", "ActivePower");
        let b = PointKey::new("ess0".to_string(), "ActivePower".to_string());
        assert_eq!(a, b);
        assert_eq!(a.to_string(), "ess0/ActivePower");
    }

    #[test]
    fn adapter_error_display_carries_named_reasons() {
        let error = AdapterError::ReadFailed {
            point: PointKey::new("ess0", "ActivePower"),
            reason: "timeout".to_string(),
        };
        let rendered = error.to_string();
        assert!(rendered.contains("ess0/ActivePower"));
        assert!(rendered.contains("timeout"));

        let error = AdapterError::Acl(AclError::QualityNotDeclared);
        assert!(error.to_string().contains("quality"));
    }

    #[test]
    fn channel_descriptor_carries_parsed_unit() {
        let descriptor = ChannelDescriptor {
            point: PointKey::new("meter0", "EnergyOut"),
            native_unit: NativeUnit::Kwh,
            description: "export energy register".to_string(),
        };
        assert_eq!(descriptor.native_unit, NativeUnit::Kwh);
    }
}
