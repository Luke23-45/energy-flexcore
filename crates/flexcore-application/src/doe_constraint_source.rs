// FIM-F14.01 (wave 5, Agent B) — The DOE envelope feed source seam: the
// application-layer port through which dynamic operating envelopes (the
// EXISTING P14.15 domain types in `flexcore_domain::doe_envelope`, consumed
// — never restated) arrive as INGESTED DATA from a DSO/operator feed.
//
// Feature: F-14 "Dynamic operating envelopes from live DSO feeds"
// (docs/releases/final-implementation-before-publication.md §5, FIM-F14).
// Owning phase record: P14.15 (`doe_envelope.rs` — the domain consumption
// interface) and decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 63_p14_15_doe_consumption_interface.md`.
//
// HONESTY STATUS (FIM-F14.06, restated here because this is a NEW feed
// seam):
//
// - **Implemented-under-FIM-F14, WRITTEN-UNRUN.** No cargo command was
//   executed in this loop; the human build/test gate owns the first run.
// - **Live DSO feed interop is UNPROVEN.** No DOE was received from any
//   operator or DNSP; no wire protocol (CSIP-AUS, SAPN Flexible Exports,
//   Project EDGE [S-158][S-159][S-160]) is implemented here — every DOE
//   semantic below inherits the UNVERIFIED register of record 63 §4.
//   Wiring a real feed is a **HUMAN LAB GATE**: it happens as a deliberate
//   adapter composition, never as a silent default, and its conformance is
//   proven against a real counterparty, never claimed in source text.
// - **ADR-010 posture (constraint data, never commands).** An envelope
//   read from this seam is optimizer CONSTRAINT DATA only (phase-14
//   context §3 constraint 3). Nothing here issues, extends, approves, or
//   modifies a DOE; nothing here talks to a device; a curtailment decision
//   is a command-lifecycle outcome (the P4 gates), never an effect of
//   reading this port. The port has no write path at all.
//
// THE SEAM SHAPE (the established port-trait pattern — the P15.03
// `WeatherDataProvider` port mirrored):
//
// - A **sync trait** (the crate rule): no future, no async runtime; an
//   adapter that must bridge asynchrony does so on its own side.
// - The trait returns `Option<DynamicOperatingEnvelope>`: `None` means the
//   feed declares no envelope for the connection point — a NORMAL state
//   (most points have no DOE program), answered by the static limit (the
//   SAPN fallback, record 63 §2). This differs deliberately from the
//   weather port's refuse-on-unwired posture: an absent envelope is not an
//   error, it is "the static limit is the whole truth", and the
//   composition records the absence as a MISSING-quality observation
//   (`doe_constraint_wiring`). The static limit itself is MANDATORY
//   declared data on every evaluation — the composition REFUSES to
//   evaluate a point without one rather than guessing an unconstrained
//   default (record 63: fail-closed, never "unconstrained").
// - Every envelope carries its issue timestamp + expiry (the existing
//   `issued_at_unix_ms` / `expires_at_unix_ms` fields, validated by the
//   domain constructor) — the feed declares no envelope without them
//   because the domain type refuses the construction.
//
// WHAT SHIPS HERE: only the deterministic, configured in-memory source
// (for composition and tests) and the named unwired/feed-absent default.
// NO feed adapter ships: no vendor, product, protocol, or endpoint is
// named anywhere in this module, and none may be without the human lab
// gate.

use std::collections::BTreeMap;

use flexcore_domain::doe_envelope::{ConnectionPointId, DynamicOperatingEnvelope, EnvelopeScope};

// --- Recorded statements -------------------------------------------------------

/// The name the unwired default reports as its source identity (surfaced on
/// every reversion observation the composition produces from it, so a log
/// or operator view labels the unwired seam by name).
pub const DOE_FEED_SEAM_NOT_WIRED: &str = "unwired-feed-absent";

/// The identity the in-memory source reports.
pub const DOE_FEED_SOURCE_IN_MEMORY: &str = "in-memory-configured";

/// The recorded honesty statement for the feed seam (pinned by tests).
pub const DOE_CONSTRAINT_SOURCE_STATEMENT: &str = "DOE envelopes enter the \
     platform as INGESTED DATA through this feed seam: per-connection-point \
     import/export limits with issue timestamps and expiry, consumed as \
     optimizer constraint data only. No envelope is issued, extended, \
     approved, or modified here; no device is talked to; a curtailment \
     decision is a command-lifecycle outcome, never an effect of reading \
     this port (ADR-010). A feed that declares no envelope is a normal \
     state answered by the mandatory static limit — never by an \
     unconstrained default.";

/// The recorded human-lab-gate statement for live feed interop (pinned by
/// tests).
pub const DOE_FEED_LAB_GATE_STATEMENT: &str = "Live DSO feed interop is \
     UNPROVEN: no DOE was received from any operator or DNSP, no wire \
     protocol is implemented in this crate, and every DOE semantic inherits \
     the UNVERIFIED register of record 63 §4 (EDGE Workstream 3 [S-158], \
     SAPN Flexible Exports [S-159], CSIP-AUS/Amber framing [S-160]). \
     Wiring a real feed is a HUMAN LAB GATE: a deliberate adapter \
     composition proven against a real counterparty, never a silent \
     default and never a conformance claim in source text.";

// --- Errors ----------------------------------------------------------------------

/// Why a feed read failed. This is the port/adapter boundary: an adapter
/// reports an opaque code + message, never raw wire text (the P1.08 rule
/// the `WeatherPortError` posture mirrors). A feed error is never answered
/// with fabricated envelope data — the composition fails closed to the
/// static limit and records the failure as a named reversion observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoeSourceError {
    /// The feed's backing adapter is unavailable or errored (fail-closed:
    /// never synthetic envelope data, never an invented envelope).
    FeedUnavailable {
        /// Opaque adapter error code.
        code: String,
        /// Opaque adapter message (no raw vendor/wire text).
        message: String,
    },
}

impl DoeSourceError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            DoeSourceError::FeedUnavailable { .. } => "DOE_FEED_UNAVAILABLE",
        }
    }
}

impl std::fmt::Display for DoeSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DoeSourceError::FeedUnavailable { code, message } => write!(
                f,
                "DOE feed unavailable ({code}): {message} — a feed error is never \
                 answered with fabricated envelope data; the composition fails \
                 closed to the static limit and records the failure"
            ),
        }
    }
}

impl std::error::Error for DoeSourceError {}

// --- The port --------------------------------------------------------------------

/// The DOE envelope feed source port (FIM-F14.01): the read-only seam the
/// constraint composition (`doe_constraint_wiring`) consumes to obtain the
/// feed's CURRENT declared envelope for one connection point.
///
/// Control status: an INPUT seam for optimizer constraint data (ADR-010).
/// The port defines the interface ONLY; the implementing adapter is the
/// composition root's choice once the human lab gate is taken. No adapter
/// ships in this crate — the shipped implementations are the deterministic
/// [`InMemoryDoeEnvelopeSource`] (configured declared data, for composition
/// and tests) and the named [`UnwiredDoeEnvelopeSource`] feed-absent
/// default.
pub trait DoeEnvelopeSource {
    /// The feed's current declared envelope for one connection point.
    ///
    /// - `Ok(None)` — the feed declares no envelope for the point right
    ///   now: the static limit governs (the composition records a
    ///   MISSING-quality reversion observation naming the source).
    /// - `Ok(Some(envelope))` — the declared envelope; whether it is in
    ///   force at an instant is decided ONLY by the domain consumption
    ///   evaluation (`evaluate_envelope`), never by the source.
    /// - `Err(DoeSourceError::FeedUnavailable)` — the feed errored: the
    ///   composition fails closed to the static limit and records a
    ///   MISSING-quality observation carrying the opaque failure code.
    ///
    /// The identity returned by [`Self::source_identity`] travels on every
    /// observation produced from this source — the seam is never silent.
    fn current_envelope(
        &self,
        connection_point: &ConnectionPointId,
    ) -> Result<Option<DynamicOperatingEnvelope>, DoeSourceError>;

    /// The declared identity of this source (recorded on every reversion
    /// observation; the unwired default names itself).
    fn source_identity(&self) -> &str;
}

// --- The in-memory configured source ----------------------------------------------

/// A deterministic, configured [`DoeEnvelopeSource`]: envelopes are
/// declared into it as DATA (the domain constructor validated them at
/// construction — this type re-validates nothing and invents nothing) and
/// are indexed per covered connection point in a `BTreeMap` (deterministic
/// iteration order).
///
/// `declare` implements the feed's CURRENT-envelope semantics: the latest
/// declaration for a connection point replaces the previous one, and the
/// replaced envelope's id is returned so the caller can record the
/// supersession. A GROUP-scope envelope declares into every member point
/// (the deterministic `connection_points()` listing, sorted and
/// deduplicated — record 63 §1). No issuance path exists: declaring here is
/// storing already-declared data, never creating or approving a DOE.
#[derive(Debug, Clone, Default)]
pub struct InMemoryDoeEnvelopeSource {
    envelopes: BTreeMap<String, DynamicOperatingEnvelope>,
}

impl InMemoryDoeEnvelopeSource {
    /// Constructs an empty source (no envelope declared for any point: the
    /// static limit is the whole truth until something is declared).
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares one envelope as DATA, indexed by every connection point its
    /// scope covers. Returns the ids of the envelopes replaced per point
    /// (deduplicated, sorted — for the caller's supersession record). The
    /// latest declaration per point wins (the feed's current-envelope
    /// semantics).
    pub fn declare(&mut self, envelope: DynamicOperatingEnvelope) -> Vec<String> {
        let points = match &envelope.scope {
            EnvelopeScope::Single(id) => vec![id.as_str().to_string()],
            EnvelopeScope::Group { members, .. } => {
                // The same deterministic listing the domain exposes
                // (sorted, deduplicated) — mirrored per-point indexing,
                // not a restated scope rule.
                let mut out: Vec<String> =
                    members.iter().map(|p| p.as_str().to_string()).collect();
                out.sort();
                out.dedup();
                out
            }
        };
        let mut replaced: Vec<String> = Vec::new();
        for point in points {
            if let Some(previous) = self.envelopes.insert(point, envelope.clone()) {
                if !replaced.contains(&previous.envelope_id) {
                    replaced.push(previous.envelope_id);
                }
            }
        }
        replaced.sort();
        replaced
    }

    /// Whether anything is declared (an empty source answers `Ok(None)`
    /// for every point, exactly like the unwired default — the difference
    /// is the recorded source identity only).
    pub fn is_empty(&self) -> bool {
        self.envelopes.is_empty()
    }
}

impl DoeEnvelopeSource for InMemoryDoeEnvelopeSource {
    fn current_envelope(
        &self,
        connection_point: &ConnectionPointId,
    ) -> Result<Option<DynamicOperatingEnvelope>, DoeSourceError> {
        Ok(self.envelopes.get(connection_point.as_str()).cloned())
    }

    fn source_identity(&self) -> &str {
        DOE_FEED_SOURCE_IN_MEMORY
    }
}

// --- The unwired / feed-absent default ---------------------------------------------

/// The shipped feed-absent default [`DoeEnvelopeSource`]: answers
/// `Ok(None)` for EVERY connection point — no envelope is declared because
/// no feed is wired. The composition fails closed to the mandatory static
/// limit and records a MISSING-quality reversion observation carrying this
/// source's named identity ([`DOE_FEED_SEAM_NOT_WIRED`]), so a composition
/// that runs without a real feed is visible in the operator view instead
/// of silently assuming "the static limit is the whole truth" without a
/// record.
///
/// This is the fail-closed posture the task records for the feed-absent
/// case: the answer is never fabricated envelope data and never an
/// unconstrained default — it is the declared static limit, loudly. A real
/// feed arrives only as a deliberate adapter implementation (the HUMAN LAB
/// GATE; [`DOE_FEED_LAB_GATE_STATEMENT`]), never as a silent default.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnwiredDoeEnvelopeSource;

impl DoeEnvelopeSource for UnwiredDoeEnvelopeSource {
    fn current_envelope(
        &self,
        _connection_point: &ConnectionPointId,
    ) -> Result<Option<DynamicOperatingEnvelope>, DoeSourceError> {
        Ok(None)
    }

    fn source_identity(&self) -> &str {
        DOE_FEED_SEAM_NOT_WIRED
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the rules above over CONSTRUCTED inputs.
// Deterministic and socket-free: no clock is read, no I/O, no threads.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::capability::Provenance;
    use flexcore_domain::doe_envelope::{ImportExportLimits, UpdateCadence};

    const POINT_A: &str = "cp-fim14-0001";
    const POINT_B: &str = "cp-fim14-0002";
    const POINT_C: &str = "cp-fim14-0003";

    fn point(name: &str) -> ConnectionPointId {
        ConnectionPointId::new(name.to_string()).expect("valid id")
    }

    /// An envelope over POINT_A and POINT_B (group scope), limits
    /// import 2.0 / export 1.0 MW, issued 1_000, expires 600_000, cadence
    /// 60 s x 2.
    fn group_envelope(id: &str) -> DynamicOperatingEnvelope {
        DynamicOperatingEnvelope::new(
            id.to_string(),
            EnvelopeScope::Group {
                name: "feeder-fim14".to_string(),
                members: vec![point(POINT_B), point(POINT_A)],
            },
            ImportExportLimits::new(2.0, 1.0).expect("valid limits"),
            1_000,
            600_000,
            UpdateCadence {
                expected_interval_seconds: 60,
                max_intervals_before_stale: 2,
            },
            1_000,
            "dnsp-operator (declared data, synthetic)".to_string(),
            Provenance::Configured,
            "FIM-F14 test envelope (synthetic; never a real operator issuance)".to_string(),
        )
        .expect("valid envelope")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(DOE_CONSTRAINT_SOURCE_STATEMENT.contains("INGESTED DATA"));
        assert!(DOE_CONSTRAINT_SOURCE_STATEMENT.contains("optimizer constraint data only"));
        assert!(DOE_CONSTRAINT_SOURCE_STATEMENT.contains("command-lifecycle outcome"));
        assert!(DOE_CONSTRAINT_SOURCE_STATEMENT.contains("mandatory static limit"));
        assert!(DOE_FEED_LAB_GATE_STATEMENT.contains("UNPROVEN"));
        assert!(DOE_FEED_LAB_GATE_STATEMENT.contains("HUMAN LAB GATE"));
        assert!(DOE_FEED_LAB_GATE_STATEMENT.contains("UNVERIFIED"));
        assert!(DOE_FEED_LAB_GATE_STATEMENT.contains("63 §4"));
        assert_eq!(DOE_FEED_SEAM_NOT_WIRED, "unwired-feed-absent");
        assert_eq!(DOE_FEED_SOURCE_IN_MEMORY, "in-memory-configured");
    }

    #[test]
    fn the_in_memory_source_serves_the_declared_envelope_per_covered_point() {
        let mut source = InMemoryDoeEnvelopeSource::new();
        assert!(source.is_empty());
        // Undeclared points answer Ok(None) — the static limit governs.
        let undeclared = source
            .current_envelope(&point(POINT_C))
            .expect("a read never errors on the in-memory source");
        assert!(undeclared.is_none());

        // A group-scope envelope declares into BOTH member points.
        let replaced = source.declare(group_envelope("doe-fim14-1"));
        assert!(replaced.is_empty(), "first declaration replaces nothing");
        assert!(!source.is_empty());
        for name in [POINT_A, POINT_B] {
            let envelope = source
                .current_envelope(&point(name))
                .expect("a read never errors on the in-memory source");
            let envelope = envelope.expect("the group scope covers this point");
            assert_eq!(envelope.envelope_id, "doe-fim14-1");
            // Issue timestamp + expiry travel with every envelope (the
            // existing domain fields, validated at construction).
            assert_eq!(envelope.issued_at_unix_ms, 1_000);
            assert_eq!(envelope.expires_at_unix_ms, 600_000);
        }
        // The non-member point stays undeclared.
        assert!(source
            .current_envelope(&point(POINT_C))
            .expect("a read never errors")
            .is_none());
    }

    #[test]
    fn a_newer_declaration_replaces_the_previous_one_and_reports_the_supersession() {
        let mut source = InMemoryDoeEnvelopeSource::new();
        source.declare(group_envelope("doe-fim14-1"));
        let replaced = source.declare(group_envelope("doe-fim14-2"));
        assert_eq!(replaced, vec!["doe-fim14-1".to_string()]);
        for name in [POINT_A, POINT_B] {
            let envelope = source
                .current_envelope(&point(name))
                .expect("a read never errors")
                .expect("declared");
            assert_eq!(envelope.envelope_id, "doe-fim14-2");
        }
    }

    #[test]
    fn the_unwired_default_declares_nothing_and_names_itself() {
        let source = UnwiredDoeEnvelopeSource;
        for name in [POINT_A, POINT_B, POINT_C] {
            let envelope = source
                .current_envelope(&point(name))
                .expect("the unwired default never errors: absence is Ok(None)");
            assert!(envelope.is_none(), "no envelope exists without a feed");
        }
        // The seam is never silent: the identity names the unwired state.
        assert_eq!(source.source_identity(), DOE_FEED_SEAM_NOT_WIRED);
    }

    #[test]
    fn the_source_identities_distinguish_the_seams() {
        // A reversion observation must be attributable to its source, so
        // the two shipped implementations never share an identity.
        assert_ne!(
            UnwiredDoeEnvelopeSource.source_identity(),
            InMemoryDoeEnvelopeSource::new().source_identity()
        );
    }

    #[test]
    fn the_source_error_renders_its_fail_closed_posture() {
        let err = DoeSourceError::FeedUnavailable {
            code: "E-FIXTURE".to_string(),
            message: "fixture".to_string(),
        };
        assert_eq!(err.name(), "DOE_FEED_UNAVAILABLE");
        let rendered = err.to_string();
        assert!(rendered.contains("fails closed to the static limit"));
        assert!(rendered.contains("never"));
        assert!(!rendered.is_empty());
    }
}
