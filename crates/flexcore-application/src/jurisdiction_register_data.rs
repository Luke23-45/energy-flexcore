// P16.18 — The CHECKED-IN market jurisdiction register DATA file: the
//         per-jurisdiction record rows (FERC 2222 timelines [S-26]
//         [S-27], the ERCOT ADER pilot posture [S-100][S-101], the UK
//         LFM [S-36][S-115], the EU EBGL posture [S-107][S-108]
//         [S-109]) with ZERO markets enabled.
//
// Phase 16 plan §2 row P16.18; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 46_p16_18_market_jurisdiction_register.md`.
//
// THE RULES THIS FILE HONORS (decided once):
//
// - **This is the register AS DATA** ([`crate::jurisdiction_register`]
//   holds the types; this file holds the checked-in rows). Every date
//   is a [`SourcedDate`] (the re-verify-at-use rule is mandatory on
//   the type itself); every conflict between program sources is
//   CARRIED with a conflict note, never silently resolved (the LR-075
//   pattern, generalized to the NYISO rows where the gap analysis and
//   the Phase 9 capture template disagree).
// - **ZERO markets are enabled**: this file registers jurisdiction
//   rows only — it registers NO enablement rows. The
//   `checked_in_register_has_zero_enabled` test pins this. An
//   enablement is a recorded HUMAN decision (D1-class, conventions
//   §4) taken against the register at run time, never shipped data.
// - **Dates are verified as of the research cut-off** ([`RESEARCH_
//   CUTOFF`]) and re-verified at use (plan risk R-P16-1). A later
//   human check updates the entry's verification date — never
//   silently.
//
// Control status: data for the simulation-only 0.x line. This file
// enables nothing, connects to nothing, and claims nothing.

use crate::capacity_market_records::{RESEARCH_CUTOFF, SourcedDate};
use crate::jurisdiction_register::{
    JurisdictionEntry, MarketJurisdictionRegister, MarketPosture, TimelineEntry,
};

/// A summary of what the checked-in register carries (pinned by
/// tests): the named jurisdictions, their postures, and the fact that
/// ZERO enablement rows exist.
pub const CHECKED_IN_REGISTER_SUMMARY: &str = "The checked-in register names nine \
     jurisdiction rows (CAISO, ISO-NE, NYISO, PJM, MISO, SPP, ERCOT ADER, GB LFM, EU \
     EBGL) plus the FERC 2222 regulatory-calendar row, carries sourced dates verified \
     as of 2026-11-07 with the re-verify-at-use rule on every date, carries the NYISO \
     and PJM date conflicts with BOTH sides and conflict notes, and registers ZERO \
     market enablements — no market is enabled by default.";

/// Builds one sourced date against the gap-analysis/primary-source
/// class, verified at the research cut-off, with the mandatory
/// re-verify rule.
fn sourced(
    date_text: &str,
    source_label: &str,
) -> SourcedDate {
    SourcedDate::new(
        date_text.to_string(),
        source_label.to_string(),
        RESEARCH_CUTOFF.to_string(),
        true,
    )
    .expect("checked-in dates always carry the re-verify rule")
}

/// Builds one timeline entry.
fn timeline(
    label: &str,
    date_text: &str,
    source_label: &str,
    conflict_note: Option<&str>,
) -> TimelineEntry {
    TimelineEntry::new(
        label.to_string(),
        sourced(date_text, source_label),
        conflict_note.map(|note| note.to_string()),
    )
    .expect("checked-in timeline entries are well-formed")
}

/// Builds the checked-in jurisdiction rows. The rows are DATA with
/// named sources; nothing here is a capability claim.
fn checked_in_entries() -> Vec<JurisdictionEntry> {
    vec![
        // --- FERC 2222 participation-model jurisdictions [S-26][S-27] ---
        JurisdictionEntry::new(
            "CAISO".to_string(),
            "California ISO".to_string(),
            "FERC 2222 DER participation model (the only RTO/ISO fully live as of the \
             research cut-off)"
                .to_string(),
            MarketPosture::Ferc2222,
            vec![timeline(
                "FERC 2222 DER aggregation participation fully live",
                "1 November 2024",
                "[S-26] FERC Order 2222 explainer; [S-97] CAISO Tariff section 11 \
                 (settlements as of Nov 1 2024)",
                None,
            )],
        )
        .expect("CAISO row"),
        JurisdictionEntry::new(
            "ISO_NE".to_string(),
            "ISO New England".to_string(),
            "FERC 2222 DER Aggregation Resource participation model".to_string(),
            MarketPosture::Ferc2222,
            vec![
                timeline(
                    "Energy and ancillary-services participation targeted/effective",
                    "1 November 2026",
                    "[S-103] ISO-NE Order 2222 participant readiness; \
                     docs/phases-11-20/00_GAP_ANALYSIS.md section 4",
                    None,
                ),
                timeline(
                    "Forward Capacity Auction 19 (FCA19) held",
                    "February 2026",
                    "[S-103] ISO-NE Order 2222 participant readiness (FCA schedule \
                     class; re-verified at use)",
                    None,
                ),
            ],
        )
        .expect("ISO-NE row"),
        JurisdictionEntry::new(
            "NYISO".to_string(),
            "New York ISO".to_string(),
            "FERC 2222 DER participation model (partial implementation in the interim)"
                .to_string(),
            MarketPosture::Ferc2222,
            vec![
                timeline(
                    "Full implementation (gap-analysis timeline row)",
                    "end-2026",
                    "[S-26] FERC Order 2222 explainer (the timeline row lists NYISO \
                     end-2026)",
                    Some("CONFLICT with the Phase 9 capture row below — both dates \
                          carried, neither selected; re-verified at use"),
                ),
                timeline(
                    "Full implementation (Phase 9 capture row)",
                    "30 April 2027 (FERC accepted the extension 30 July 2026)",
                    "ledger/phase-09-pilot-qualification/\
                     11_market_program_requirements.md section 7 (checked 2026-11-07)",
                    Some("CONFLICT with the gap-analysis row above — both dates \
                          carried, neither selected; re-verified at use"),
                ),
            ],
        )
        .expect("NYISO row"),
        JurisdictionEntry::new(
            "PJM".to_string(),
            "PJM Interconnection".to_string(),
            "FERC 2222 DER aggregation participation model (the LR-075 date conflict \
             applies to the capacity row)"
                .to_string(),
            MarketPosture::Ferc2222,
            vec![
                timeline(
                    "Capacity-market participation (LR-075 conflict side A)",
                    "February 2027 (proposed)",
                    "[S-26] FERC Order 2222 explainer (the gap-analysis timeline row \
                     lists PJM Feb 2027/28)",
                    Some("LR-075 date conflict — see side B below and the LR-075 \
                          conflict record (P16.16); BOTH carried, NEITHER selected; \
                          re-verified against PJM's tariff at use"),
                ),
                timeline(
                    "Energy participation expected (LR-075 conflict side B)",
                    "1 February 2028",
                    "ledger/phase-09-pilot-qualification/\
                     11_market_program_requirements.md section 7 (checked 2026-11-07; \
                     the capacity date explicitly conflicting between sources)",
                    Some("LR-075 date conflict — see side A above; BOTH carried, \
                          NEITHER selected; re-verified against PJM's tariff at use"),
                ),
            ],
        )
        .expect("PJM row"),
        JurisdictionEntry::new(
            "MISO".to_string(),
            "Midcontinent ISO".to_string(),
            "FERC 2222 DER participation model (two-phase)".to_string(),
            MarketPosture::Ferc2222,
            vec![
                timeline(
                    "Phase 1",
                    "by 1 June 2027",
                    "[S-26] FERC Order 2222 explainer",
                    None,
                ),
                timeline(
                    "Phase 2",
                    "by 1 June 2029",
                    "[S-26] FERC Order 2222 explainer",
                    None,
                ),
            ],
        )
        .expect("MISO row"),
        JurisdictionEntry::new(
            "SPP".to_string(),
            "Southwest Power Pool".to_string(),
            "FERC 2222 DER participation model (proposed)".to_string(),
            MarketPosture::Ferc2222,
            vec![timeline(
                "Proposed implementation",
                "second quarter of 2030",
                "[S-26] FERC Order 2222 explainer",
                None,
            )],
        )
        .expect("SPP row"),
        // --- The ERCOT ADER pilot posture [S-100][S-101] ---
        JurisdictionEntry::new(
            "ERCOT_ADER".to_string(),
            "ERCOT (Aggregate Distributed Energy Resource pilot)".to_string(),
            "The ADER pilot is ERCOT's DER participation path; ERCOT is NOT under FERC \
             jurisdiction and is not subject to Order 2222"
                .to_string(),
            MarketPosture::ErcotAderPilot,
            vec![
                timeline(
                    "ADER pilot posture (pilot program; participation by pilot rules)",
                    "pilot program ongoing as of the research cut-off",
                    "[S-100] ERCOT ADER pilot",
                    None,
                ),
                timeline(
                    "ADER Phase 3.3 pilot report published",
                    "22 May 2026 (document date as published)",
                    "[S-101] ERCOT ADER Pilot Project Phase 3.3 report",
                    None,
                ),
            ],
        )
        .expect("ERCOT ADER row"),
        // --- The UK Local Flexibility Market [S-36][S-37][S-115] ---
        JurisdictionEntry::new(
            "GB_LFM".to_string(),
            "Great Britain Local Flexibility Market".to_string(),
            "DSO flexibility procurement via the Piclo platform; availability and \
             utilization payment structure"
                .to_string(),
            MarketPosture::UkLfm,
            vec![
                timeline(
                    "P483 dual-participation arrangements live",
                    "27 November 2025",
                    "[S-115] Elexon P483 (via industry coverage; re-verified at use)",
                    None,
                ),
                timeline(
                    "Piclo flexibility tender platform posture",
                    "platform in operation as of the research cut-off",
                    "[S-36] Piclo; [S-37] ENA flexibility overview; [S-114] NGED \
                     flexibility procurement report 2025-26",
                    None,
                ),
            ],
        )
        .expect("GB LFM row"),
        // --- The EU balancing / EBGL posture [S-107][S-108][S-109] ---
        JurisdictionEntry::new(
            "EU_EBGL".to_string(),
            "EU imbalance settlement (EBGL)".to_string(),
            "EBGL Art. 52-55 15-minute imbalance settlement period (ISP)".to_string(),
            MarketPosture::EuBalancing,
            vec![timeline(
                "15-minute ISP transition",
                "15-minute ISP per EBGL Art. 52-55 (member-state transition dates \
                 differ; re-verified at use)",
                "[S-107] EBGL (EU) 2017/2195; [S-108] ACER Decision 18-2020; [S-109] \
                 RTE 15-minute ISP transition",
                None,
            )],
        )
        .expect("EU EBGL row"),
        // --- The FERC 2222 regulatory-calendar row [S-26][S-27] ---
        JurisdictionEntry::new(
            "FERC_2222_CALENDAR".to_string(),
            "FERC Order 2222 regulatory calendar (cross-jurisdiction)".to_string(),
            "The compliance-timeline tracker row: state/RERRA actions tracked per the \
             NARUC tracker; RTO/ISO compliance dates per the FERC explainer"
                .to_string(),
            MarketPosture::Ferc2222,
            vec![timeline(
                "Compliance-timeline tracking posture",
                "tracker in operation as of the research cut-off",
                "[S-26] FERC Order 2222 explainer; [S-27] NARUC 2222 tracker",
                None,
            )],
        )
        .expect("FERC 2222 calendar row"),
    ]
}

/// Builds the checked-in register: the jurisdiction rows above and
/// ZERO enablement rows. This is the ONLY data file the register
/// ships with, and no market is enabled by default.
pub fn checked_in_register() -> MarketJurisdictionRegister {
    let mut register = MarketJurisdictionRegister::new();
    for entry in checked_in_entries() {
        register
            .register_jurisdiction(entry)
            .expect("checked-in rows are unique and well-formed");
    }
    // NO enablement rows are registered here: zero markets enabled.
    register
        .validate()
        .expect("the checked-in register validates");
    register
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. These tests pin the DATA FILE's
// honesty properties; the fixture dates above are sourced data, and the
// synthetic-fixture dates in the register-type tests are separate.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jurisdiction_register::NONE_ENABLED_BY_DEFAULT_STATEMENT;

    #[test]
    fn the_checked_in_register_carries_zero_enablements() {
        let register = checked_in_register();
        // Zero enablement rows exist at all.
        assert!(
            register
                .enabled_jurisdictions_for_tenant("any-tenant")
                .is_empty()
        );
        register.validate().expect("validates");
        // The summary names the zero-enabled fact.
        assert!(CHECKED_IN_REGISTER_SUMMARY.contains("ZERO market enablements"));
        assert!(NONE_ENABLED_BY_DEFAULT_STATEMENT.contains("ZERO markets enabled"));
    }

    #[test]
    fn every_checked_in_date_carries_the_reverify_rule() {
        let register = checked_in_register();
        for entry in register.jurisdictions() {
            assert!(!entry.timeline().is_empty(), "{} has a timeline", entry.jurisdiction_id());
            for timeline_entry in entry.timeline() {
                assert!(
                    timeline_entry.date().reverify_at_use(),
                    "{}: the re-verify rule is mandatory",
                    entry.jurisdiction_id()
                );
                assert_eq!(
                    timeline_entry.date().verified_on(),
                    crate::capacity_market_records::RESEARCH_CUTOFF
                );
                assert!(
                    !timeline_entry.date().source_label().trim().is_empty(),
                    "{}: every date names its source",
                    entry.jurisdiction_id()
                );
            }
        }
    }

    #[test]
    fn the_conflicts_are_carried_with_notes() {
        let register = checked_in_register();
        // NYISO: two conflicting rows, both noted.
        let nyiso = register.jurisdiction("NYISO").expect("NYISO row");
        let noted: Vec<&TimelineEntry> = nyiso
            .timeline()
            .iter()
            .filter(|t| t.conflict_note().is_some())
            .collect();
        assert_eq!(noted.len(), 2, "the NYISO conflict is carried on both sides");
        // PJM: the LR-075 conflict is named on both sides.
        let pjm = register.jurisdiction("PJM").expect("PJM row");
        let lr075: Vec<&TimelineEntry> = pjm
            .timeline()
            .iter()
            .filter(|t| t.conflict_note().map(|n| n.contains("LR-075")).unwrap_or(false))
            .collect();
        assert_eq!(lr075.len(), 2, "the LR-075 conflict is carried on both sides");
    }

    #[test]
    fn the_checked_in_rows_are_nine_plus_the_calendar() {
        let register = checked_in_register();
        let ids: Vec<&str> = register
            .jurisdictions()
            .iter()
            .map(|e| e.jurisdiction_id())
            .collect();
        for expected in [
            "CAISO",
            "ISO_NE",
            "NYISO",
            "PJM",
            "MISO",
            "SPP",
            "ERCOT_ADER",
            "GB_LFM",
            "EU_EBGL",
            "FERC_2222_CALENDAR",
        ] {
            assert!(ids.contains(&expected), "missing row {expected}");
        }
        assert_eq!(ids.len(), 10);
    }

    #[test]
    fn the_ercot_posture_records_the_pilot_and_not_ferc_2222() {
        let register = checked_in_register();
        let ercot = register.jurisdiction("ERCOT_ADER").expect("ERCOT row");
        assert_eq!(ercot.posture().name(), "ERCOT_ADER_PILOT");
        assert!(
            ercot.participation_model_label().contains("NOT under FERC jurisdiction"),
            "the ADER posture records ERCOT's non-FERC status [S-100]"
        );
    }

    #[test]
    fn a_d1_enablement_records_against_the_register_but_ships_disabled() {
        // The enablement path EXISTS (for the human decision to be
        // recorded at run time) but the shipped data carries none.
        let mut register = checked_in_register();
        assert!(
            register
                .record_enablement(
                    crate::jurisdiction_register::TenantMarketEnablement::disabled(
                        "tenant-1".to_string(),
                        "CAISO".to_string(),
                        1_700_000_000_000,
                    )
                    .expect("disabled row")
                )
                .is_ok(),
            "a DISABLED row can be recorded at run time"
        );
        assert!(
            register
                .enabled_jurisdictions_for_tenant("tenant-1")
                .is_empty()
        );
        // And the shipped register (rebuilt) still carries nothing.
        let shipped = checked_in_register();
        assert_eq!(shipped.enablement("tenant-1", "CAISO"), None);
    }
}
