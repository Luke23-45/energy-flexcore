// P1.10 — RetentionPolicy: per-data-class retention configuration that
//         defaults to "unset" everywhere. No universal retention period.
//
// Part of the flexcore-storage P1.10 deliverable. Normative source: P0.16 §4
// (`ledger/phase-00-product-freeze/16_tenancy_storage_principles.md`, report
// §16.3): "Define retention per data class … Retention must be configurable
// by customer contract, regional market rules, and applicable
// privacy/security requirements. **Do not choose a universal retention
// period in code** (§16.3). No retention value, default period, or
// per-class duration is set anywhere in this document — that is the rule,
// not an omission."
//
// This module is therefore a **configuration type, not a policy engine**:
//
// - Every data class (the seven §16.3 classes, in report order) starts as
//   [`RetentionSetting::Unset`]; there is no default duration anywhere in
//   this file, not even a "reasonable" one.
// - Values arrive from **deployment configuration** (per customer contract,
//   regional market rules, privacy/security requirements) — the operator
//   and the deployment own them, never this crate.
// - Deserialization uses `#[serde(default)]`, so a partial configuration
//   document fills missing classes with `Unset`. A missing value can never
//   silently become a built-in period.
// - The command/audit-evidence class is deliberately included with an unset
//   default too: even audit evidence has no universal retention period in
//   code (its actual retention is a deployment/legal decision; the
//   append-only rule of flexcore-audit governs writes, not deletion).
//
// Enforcement of the *setting* itself (partition drops, deletion jobs) is
// infrastructure work owned by the deployment/adapters, never by this type —
// this type only carries the configuration with an unset default.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The retention setting for one data class.
///
/// `Unset` is the default and means "no decision has been supplied by the
/// deployment configuration" — consumers MUST treat it as "no retention
/// action authorized by this configuration", never as "keep forever" and
/// never as "apply some built-in default period" (P0.16 §4: no universal
/// period in code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetentionSetting {
    /// No retention period supplied by deployment configuration.
    Unset,
    /// A retention period supplied by deployment configuration. The value
    /// carries no provenance or legal basis here — the deployment
    /// configuration that produced it is the record of that basis.
    Configured {
        /// The configured retention period. Positivity is a deployment
        /// concern; this type records the configured value as given.
        period: Duration,
    },
}

/// Per-data-class retention configuration (the seven §16.3 classes, in
/// report order).
///
/// Defaults to all-`Unset` ([`RetentionPolicy::default`],
/// [`RetentionPolicy::unset`]); deserialization fills missing classes with
/// `Unset` (`#[serde(default)]`). Fields are public because this is a plain
/// configuration record — validation of a *deployment's* choices belongs to
/// the deployment tooling that loads it, not to this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionPolicy {
    /// Class 1: raw high-frequency telemetry.
    pub raw_high_frequency_telemetry: RetentionSetting,
    /// Class 2: normalized telemetry.
    pub normalized_telemetry: RetentionSetting,
    /// Class 3: aggregated intervals.
    pub aggregated_intervals: RetentionSetting,
    /// Class 4: command/audit evidence.
    pub command_audit_evidence: RetentionSetting,
    /// Class 5: market records.
    pub market_records: RetentionSetting,
    /// Class 6: simulation artifacts.
    pub simulation_artifacts: RetentionSetting,
    /// Class 7: model/forecast outputs.
    pub model_forecast_outputs: RetentionSetting,
}

impl RetentionPolicy {
    /// A policy with every class unset — the default and the only state
    /// this crate can construct without deployment input.
    pub fn unset() -> Self {
        Self::default()
    }

    /// `true` if no class carries a configured value. A deployment loader
    /// can use this to detect "retention was never configured" and surface
    /// that to the operator instead of assuming anything.
    pub fn is_fully_unset(&self) -> bool {
        [
            self.raw_high_frequency_telemetry,
            self.normalized_telemetry,
            self.aggregated_intervals,
            self.command_audit_evidence,
            self.market_records,
            self.simulation_artifacts,
            self.model_forecast_outputs,
        ]
        .iter()
        .all(|s| matches!(s, RetentionSetting::Unset))
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            raw_high_frequency_telemetry: RetentionSetting::Unset,
            normalized_telemetry: RetentionSetting::Unset,
            aggregated_intervals: RetentionSetting::Unset,
            command_audit_evidence: RetentionSetting::Unset,
            market_records: RetentionSetting::Unset,
            simulation_artifacts: RetentionSetting::Unset,
            model_forecast_outputs: RetentionSetting::Unset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_fully_unset() {
        // P1.10 acceptance: RetentionPolicy defaults are all unset — no
        // universal retention period exists in code (P0.16 §4).
        let policy = RetentionPolicy::default();
        assert!(policy.is_fully_unset());
        assert_eq!(policy, RetentionPolicy::unset());
        assert_eq!(policy.raw_high_frequency_telemetry, RetentionSetting::Unset);
        assert_eq!(policy.normalized_telemetry, RetentionSetting::Unset);
        assert_eq!(policy.aggregated_intervals, RetentionSetting::Unset);
        assert_eq!(policy.command_audit_evidence, RetentionSetting::Unset);
        assert_eq!(policy.market_records, RetentionSetting::Unset);
        assert_eq!(policy.simulation_artifacts, RetentionSetting::Unset);
        assert_eq!(policy.model_forecast_outputs, RetentionSetting::Unset);
    }

    #[test]
    fn configuring_one_class_leaves_the_others_unset() {
        let mut policy = RetentionPolicy::unset();
        policy.raw_high_frequency_telemetry = RetentionSetting::Configured {
            period: Duration::from_secs(90 * 24 * 60 * 60),
        };
        assert!(!policy.is_fully_unset());
        // Even the configured class is only configured because the test —
        // standing in for deployment configuration — supplied a value.
        assert!(matches!(
            policy.raw_high_frequency_telemetry,
            RetentionSetting::Configured { .. }
        ));
        // Every other class stays unset; no class inherits the configured
        // value as a fallback.
        assert_eq!(policy.normalized_telemetry, RetentionSetting::Unset);
        assert_eq!(policy.aggregated_intervals, RetentionSetting::Unset);
        assert_eq!(policy.command_audit_evidence, RetentionSetting::Unset);
        assert_eq!(policy.market_records, RetentionSetting::Unset);
        assert_eq!(policy.simulation_artifacts, RetentionSetting::Unset);
        assert_eq!(policy.model_forecast_outputs, RetentionSetting::Unset);
    }

    #[test]
    fn partial_configuration_deserializes_with_unset_gaps() {
        // An empty configuration document yields the all-unset policy: a
        // missing class never silently becomes a built-in period.
        let empty: RetentionPolicy = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(empty, RetentionPolicy::unset());

        // A partial document configures exactly the classes it names; every
        // unnamed class stays unset. (Variant names are SCREAMING_SNAKE_CASE
        // per the RetentionSetting serde rule; serde renders a Duration as
        // {"secs", "nanos"}.)
        let partial: RetentionPolicy = serde_json::from_value(serde_json::json!({
            "raw_high_frequency_telemetry": {
                "CONFIGURED": { "period": { "secs": 7776000, "nanos": 0 } }
            }
        }))
        .unwrap();
        assert!(matches!(
            partial.raw_high_frequency_telemetry,
            RetentionSetting::Configured { .. }
        ));
        assert_eq!(partial.command_audit_evidence, RetentionSetting::Unset);
        assert_eq!(partial.market_records, RetentionSetting::Unset);
    }

    #[test]
    fn configured_values_round_trip_and_unset_stays_unset() {
        let mut policy = RetentionPolicy::unset();
        policy.market_records = RetentionSetting::Configured {
            period: Duration::from_secs(3_600),
        };
        let json = serde_json::to_value(&policy).unwrap();
        let back: RetentionPolicy = serde_json::from_value(json).unwrap();
        assert_eq!(back, policy);
        assert_eq!(
            back.market_records,
            RetentionSetting::Configured {
                period: Duration::from_secs(3_600)
            }
        );
        assert_eq!(back.command_audit_evidence, RetentionSetting::Unset);
    }

    #[test]
    fn a_fully_unset_policy_survives_a_round_trip() {
        let policy = RetentionPolicy::unset();
        let back: RetentionPolicy =
            serde_json::from_value(serde_json::to_value(&policy).unwrap()).unwrap();
        assert!(back.is_fully_unset());
    }

    #[test]
    fn no_class_defaults_to_a_configured_period() {
        // Direct check of the P0.16 §4 rule across all seven classes: the
        // Default impl contains no duration anywhere.
        let policy = RetentionPolicy::default();
        let classes = [
            &policy.raw_high_frequency_telemetry,
            &policy.normalized_telemetry,
            &policy.aggregated_intervals,
            &policy.command_audit_evidence,
            &policy.market_records,
            &policy.simulation_artifacts,
            &policy.model_forecast_outputs,
        ];
        for class in classes {
            assert!(
                matches!(*class, RetentionSetting::Unset),
                "a data class must not carry a built-in retention period"
            );
        }
    }
}
