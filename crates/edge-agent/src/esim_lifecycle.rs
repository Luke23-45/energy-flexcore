// P18.11 — eSIM / CELLULAR LIFECYCLE TRACKING — DATA MODEL ONLY (phase-18
//          plan P18.11; source [S-167] IDEMIA eSIM IoT Manager overview).
//
// SCOPE, stated twice (here and in the decision record): this module is a
// DATA MODEL keyed to the migration-0023 fleet-state journal
// ([`crate::fleet_registry`], kind `ESIM_PROFILE_STATE`). It TRACKS
// profile state as recorded data; it does NOT manage profiles, talk to
// any carrier system, or implement any eSIM/SGP standard. CARRIER
// SPECIFICS ARE OUT OF SCOPE: no SM-DP+ address, no activation code, no
// confirmation code, no carrier-specific policy field exists in this
// model — only carrier-AGNOSTIC observations (state, carrier LABEL,
// identifier REFERENCES, recorded instants).
//
// PROVENANCE (honest): the profile-state vocabulary below is inspired by
// the eSIM lifecycle concepts named in the [S-167] commercial overview
// (download / enable / disable / delete). The exact state names, the
// transitions a real eUICC enforces, and the SGP.22/SGP.32 semantics are
// UNVERIFIED — this program holds no standard text. The vocabulary is
// therefore CLOSED but DELIBERATELY COARSE, and a deployment tracking a
// real eSIM fleet must verify the mapping against the standard before
// relying on it (a named human-gate check in the decision record).
//
// WHAT THE MODEL IS FOR: a fleet operator sees, per device, whether its
// cellular link is expected to work (profile present + enabled) — an
// input to the last-seen health interpretation (a device whose only
// uplink is cellular and whose profile is DISABLED is expected to be
// NOT_SEEN, not "mysteriously offline"). The platform NEVER changes a
// profile state; it records what was OBSERVED (the journal is
// append-only).
//
// HONESTY BOUNDS: no SIM exists, no profile was ever downloaded, enabled,
// or deleted; no carrier relationship exists (phase-18 constraint 6).
// `secret.read` stays in the never-granted set — identifier VALUES on
// this model are recorded references, and nothing here reads or stores
// credentials of any kind.

use crate::fleet_registry::FleetRegistryError;

// --- The closed profile-state vocabulary (the journal state_value domain) ---

/// The closed, deliberately coarse eSIM profile-state vocabulary (the
/// `state_value` domain of the `ESIM_PROFILE_STATE` journal rows).
/// Unknown real-world states map to [`EsimProfileState::Unknown`] by the
/// reporter — never invented values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EsimProfileState {
    /// No profile is (known to be) present on the device.
    NoProfile,
    /// A profile is downloaded and enabled: the cellular link is expected
    /// to work.
    Enabled,
    /// A profile is downloaded but disabled: the cellular link is NOT
    /// expected to work.
    Disabled,
    /// A profile is present but in a state the reporter could not
    /// characterize. Never healthy-by-default.
    Unknown,
    /// The profile was deleted; the device has no usable cellular
    /// identity until a new profile arrives.
    Deleted,
}

impl EsimProfileState {
    /// The exact `state_value` string for the ESIM_PROFILE_STATE journal
    /// rows (the migration-0023 domain).
    pub fn sql_value(self) -> &'static str {
        match self {
            EsimProfileState::NoProfile => "NO_PROFILE",
            EsimProfileState::Enabled => "ENABLED",
            EsimProfileState::Disabled => "DISABLED",
            EsimProfileState::Unknown => "UNKNOWN",
            EsimProfileState::Deleted => "DELETED",
        }
    }

    /// Parses the journal's `state_value` for an eSIM row. Unknown
    /// strings are the named [`FleetRegistryError::UnknownStateValue`]
    /// refusal — the vocabulary is closed, never guessed open.
    pub fn from_sql_value(value: &str) -> Result<Self, FleetRegistryError> {
        match value {
            "NO_PROFILE" => Ok(EsimProfileState::NoProfile),
            "ENABLED" => Ok(EsimProfileState::Enabled),
            "DISABLED" => Ok(EsimProfileState::Disabled),
            "UNKNOWN" => Ok(EsimProfileState::Unknown),
            "DELETED" => Ok(EsimProfileState::Deleted),
            other => Err(FleetRegistryError::UnknownStateValue {
                kind: "ESIM_PROFILE_STATE",
                encountered: other.to_string(),
            }),
        }
    }
}

// --- The carrier-agnostic observation record -------------------------------------------

/// One carrier-AGNOSTIC cellular-lifecycle observation for one device
/// (the shape a reporter records into the journal). Every identifier is a
/// REFERENCE label (an opaque correlation label the deployment defines) —
/// no ICCID/IMSI/eID VALUE validation is performed and none is trusted
/// (this module cannot verify identifier formats without a standard text;
/// the exact identifier semantics are UNVERIFIED [S-167]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsimObservation {
    /// The device identity (matches the registry/journal identity scope).
    pub device_id: String,
    pub profile_state: EsimProfileState,
    /// The carrier LABEL (a free-text label, e.g. "Carrier A" — never a
    /// carrier identifier the platform validates or acts on).
    pub carrier_label: String,
    /// The profile REFERENCE label (opaque, deployment-defined; empty =
    /// unrecorded, which is legal only for [`EsimProfileState::NoProfile`]).
    pub profile_reference: String,
    /// When the observation was made (unix millis — recorded data).
    pub observed_at_unix_millis: i64,
}

impl EsimObservation {
    /// The `state_value` encoding for the journal row: the profile state,
    /// with the carrier label and profile reference riding the journal
    /// `detail` column (see [`EsimObservation::detail_text`]) — the
    /// state_value stays exactly the closed vocabulary so the journal
    /// kind's value domain stays parseable.
    pub fn sql_value(&self) -> &'static str {
        self.profile_state.sql_value()
    }

    /// The `detail` encoding: `carrier:<label>|profile:<reference-or-unrecorded>`.
    /// Labels with control characters are refused at validation time.
    pub fn detail_text(&self) -> String {
        let profile_part = if self.profile_reference.trim().is_empty() {
            "unrecorded".to_string()
        } else {
            self.profile_reference.clone()
        };
        format!("carrier:{}|profile:{}", self.carrier_label, profile_part)
    }

    /// Validates the observation (fail-closed):
    /// - device id non-empty, positive instant;
    /// - a profile REFERENCE must be recorded whenever a profile exists
    ///   (an ENABLED/DISABLED/UNKNOWN state with no reference is refused —
    ///   an unidentifiable profile is untrackable);
    /// - the carrier label and reference are single-line printable text
    ///   (they ride journal detail text; control characters are refused).
    pub fn validate(&self) -> Result<(), FleetRegistryError> {
        if self.device_id.trim().is_empty() {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "the eSIM observation names no device".to_string(),
            });
        }
        if self.observed_at_unix_millis <= 0 {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "the eSIM observation instant must be a positive unix-millis value"
                    .to_string(),
            });
        }
        if !self.carrier_label.is_single_line_printable() {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "the carrier label must be single-line printable text".to_string(),
            });
        }
        if !self.profile_reference.is_single_line_printable() {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "the profile reference must be single-line printable text".to_string(),
            });
        }
        if self.profile_reference.trim().is_empty()
            && matches!(
                self.profile_state,
                EsimProfileState::Enabled | EsimProfileState::Disabled | EsimProfileState::Unknown
            )
        {
            return Err(FleetRegistryError::InvalidRecord {
                reason: format!(
                    "a profile in state {} must carry a profile reference (an unidentifiable \
                     profile is untrackable)",
                    self.profile_state.sql_value()
                ),
            });
        }
        Ok(())
    }
}

/// Single-line printable check shared by the observation text fields.
trait SingleLinePrintable {
    fn is_single_line_printable(&self) -> bool;
}

impl SingleLinePrintable for String {
    fn is_single_line_printable(&self) -> bool {
        !self.contains(|c: char| c.is_control())
    }
}

// --- The expected-reachability interplay (the reason the model exists) -----

/// The closed expected-reachability vocabulary: what the LAST-SEEN health
/// interpretation should expect given the latest profile state. This is a
/// READING AID for the fleet view (P18.06 consumes it as visualization
/// data), never a guarantee — a device can be unreachable for reasons a
/// data model cannot see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedCellularReachability {
    /// The cellular link is expected to work (profile ENABLED).
    ExpectedReachable,
    /// The cellular link is NOT expected to work (DISABLED / DELETED /
    /// NO_PROFILE / UNKNOWN / no observation).
    ExpectedUnreachable,
}

impl ExpectedCellularReachability {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            ExpectedCellularReachability::ExpectedReachable => "CELLULAR_EXPECTED_REACHABLE",
            ExpectedCellularReachability::ExpectedUnreachable => {
                "CELLULAR_EXPECTED_UNREACHABLE"
            }
        }
    }

    /// Derives the expectation from the latest profile state. `None`
    /// (no observation ever) maps to
    /// [`ExpectedCellularReachability::ExpectedUnreachable`] — absence of
    /// evidence is never treated as reachability (the unknown-is-not-
    /// healthy discipline).
    pub fn from_state(state: Option<EsimProfileState>) -> Self {
        match state {
            Some(EsimProfileState::Enabled) => {
                ExpectedCellularReachability::ExpectedReachable
            }
            _ => ExpectedCellularReachability::ExpectedUnreachable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(state: EsimProfileState) -> EsimObservation {
        EsimObservation {
            device_id: "edge-01".to_string(),
            profile_state: state,
            carrier_label: "Carrier A".to_string(),
            profile_reference: "profile-ref-01".to_string(),
            observed_at_unix_millis: 1_700_000_000_000,
        }
    }

    #[test]
    fn the_profile_vocabulary_is_the_journal_value_domain() {
        for state in [
            EsimProfileState::NoProfile,
            EsimProfileState::Enabled,
            EsimProfileState::Disabled,
            EsimProfileState::Unknown,
            EsimProfileState::Deleted,
        ] {
            assert_eq!(
                EsimProfileState::from_sql_value(state.sql_value()).expect("round trip"),
                state
            );
        }
        assert_eq!(
            EsimProfileState::from_sql_value("enabled").unwrap_err().name(),
            "FLEET_STATE_VALUE_UNKNOWN",
            "the vocabulary is exact-case closed"
        );
    }

    #[test]
    fn the_observation_encodes_state_in_the_value_and_labels_in_the_detail() {
        let enabled = observation(EsimProfileState::Enabled);
        assert_eq!(enabled.sql_value(), "ENABLED");
        assert_eq!(
            enabled.detail_text(),
            "carrier:Carrier A|profile:profile-ref-01"
        );
        let mut no_profile = observation(EsimProfileState::NoProfile);
        no_profile.profile_reference = String::new();
        assert!(no_profile.validate().is_ok(), "no profile legally has no reference");
        assert_eq!(
            no_profile.detail_text(),
            "carrier:Carrier A|profile:unrecorded"
        );
    }

    #[test]
    fn validation_refuses_untrackable_and_unprintable_observations() {
        let mut anonymous = observation(EsimProfileState::Enabled);
        anonymous.profile_reference = String::new();
        assert_eq!(
            anonymous.validate().unwrap_err().name(),
            "FLEET_STATE_RECORD_INVALID",
            "an enabled profile with no reference is untrackable — refused"
        );
        let mut control_char = observation(EsimProfileState::Disabled);
        control_char.carrier_label = "Carrier\nB".to_string();
        assert!(control_char.validate().is_err());
        let mut empty_device = observation(EsimProfileState::Enabled);
        empty_device.device_id = "  ".to_string();
        assert!(empty_device.validate().is_err());
    }

    #[test]
    fn the_reachability_expectation_treats_absence_as_unreachable() {
        assert_eq!(
            ExpectedCellularReachability::from_state(Some(EsimProfileState::Enabled)),
            ExpectedCellularReachability::ExpectedReachable
        );
        for state in [
            EsimProfileState::NoProfile,
            EsimProfileState::Disabled,
            EsimProfileState::Deleted,
            EsimProfileState::Unknown,
        ] {
            assert_eq!(
                ExpectedCellularReachability::from_state(Some(state)),
                ExpectedCellularReachability::ExpectedUnreachable
            );
        }
        assert_eq!(
            ExpectedCellularReachability::from_state(None),
            ExpectedCellularReachability::ExpectedUnreachable,
            "no observation is never reachability"
        );
    }
}
