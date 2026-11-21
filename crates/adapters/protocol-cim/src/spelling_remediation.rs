// FIM wave 8B (Agent B) — the CGMES verified-spelling REMEDIATION register
// for the declared attribute-key vocabulary (record
// 44a_p14_12_fim_class_spelling_verification.md §3/§4; its 2026-11-21
// addendum).
//
// **WRITTEN-UNRUN.** This module has never been compiled or executed — the
// no-build/no-test bans (program conventions §1) make execution a HUMAN
// GATE. Nothing here is a conformance result: interop with a real DSO
// CGMES profile remains a HUMAN LAB GATE (crate conventions §7.3).
//
// WHAT THIS MODULE IS (the ADDITIVE remediation record 44a's §4 named):
// record 44a §3 verified the 15 declared attribute-key constants of
// [`crate::model`] against the primary sources and reached 11 MISMATCH
// (verified) verdicts — each a verified fact with its source: the declared
// spelling is ABSENT from the CGMES profile set and, where a replacement
// spelling was verified, the actual carrier is named. The verdicts were
// RECORDED, not applied (44a §4 point 1: applying them is the recorded
// edit that needs its own reviewed change). THIS module is that reviewed,
// ADDITIVE change:
//
// 1. **Accepted aliases** ([`ALIASES`]): an incoming attribute element
//    whose `Class.attribute` spelling matches a VERIFIED CGMES spelling is
//    normalized onto the canonical declared key that serves the same
//    attribute. Every existing declared key stays accepted EXACTLY as
//    before (the fixtures of `tests/cgmes_import.rs` stay true); the alias
//    only ADDS accepted spellings. Each alias carries its source citation
//    (the 44a record row and the [S-17x] source that verified it).
// 2. **The verified-absent register** ([`VERIFIED_ABSENT`]): the MISMATCH
//    verdicts where NO replacement spelling was verified — the declared
//    key is verified ABSENT from the CGMES profile set and the actual
//    carrier is either a different class' attribute or an UNVERIFIED
//    traversal. These are NOT aliases (record 44a's addendum; mapping them
//    would fabricate semantics the sources do not carry). Each entry
//    carries a named not-modelled statement (the same recorded-reason
//    pattern the class-level EXPLICITLY_UNMAPPED register uses in
//    [`crate::class_mapping`]).
//
// THE HONEST ARITHMETIC (pinned by the tests below): 2 accepted aliases +
// 9 verified-absent entries = 11 = every MISMATCH (verified) verdict of
// record 44a §3. NOTE THE RECORDED DISCREPANCY, reported to the wave
// orchestrator and left for a human correction: record 44a §3's summary
// line reads "9/15 MISMATCH (verified)" while its own table carries 11
// MISMATCH rows (4/15 VERIFIED / 9/15 MISMATCH / 2/15 UNVERIFIED sums to
// 15 but contradicts the per-row verdicts, which give 2 VERIFIED (match) /
// 11 MISMATCH / 2 UNVERIFIED). This module remediates the TABLE (all 11
// verdicts), never the summary line's arithmetic; the "9" appears here as
// the verified-absent count, and the 2 UNVERIFIED items (the vk/vkR
// short-circuit spellings) are UNTOUCHED — they stay labeled UNVERIFIED
// exactly as the record requires.
//
// THE REMEDIATION IS SPELLING-LEVEL, NEVER STRUCTURAL: an alias normalizes
// the incoming element spelling onto the canonical key. Where the verified
// carrier is a DIFFERENT structural object (e.g. ratedS lives on
// PowerTransformerEnd objects, not on the PowerTransformer object — record
// 44 §4 item 4), the end-object traversal stays the recorded UNVERIFIED
// work item; the alias does not claim it. The mapping still consumes only
// the declared canonical keys, and every classification still runs through
// the EXISTING register ([`crate::unmapped`]) — no parallel gate, no
// parallel mapping.

use crate::model::{
    ATTR_BASE_VOLTAGE_NOMINAL_KV, ATTR_EXTERNAL_GRID_BUS_REF, ATTR_EXTERNAL_GRID_VA_DEGREE,
    ATTR_EXTERNAL_GRID_VM_PU, ATTR_LINE_FROM_BUS_REF, ATTR_LINE_RATED_CURRENT_A,
    ATTR_LINE_TO_BUS_REF, ATTR_NODE_BASE_VOLTAGE_REF, ATTR_TRANSFORMER_HV_BUS_REF,
    ATTR_TRANSFORMER_LV_BUS_REF, ATTR_TRANSFORMER_RATED_S_VA,
};

/// The citation prefix of every entry below (the verification record).
pub const VERIFICATION_RECORD: &str =
    "docs/phases-11-20/phase-14-protocol-device-connectivity/records/\
     44a_p14_12_fim_class_spelling_verification.md";

/// One ACCEPTED alias: an incoming attribute-element spelling that record
/// 44a §3 VERIFIED as the actual CGMES spelling of the attribute one of the
/// declared constants serves. The parser normalizes it onto
/// [`canonical_key`] — the mapping vocabulary never grows a second
/// consumer-facing key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellingAlias {
    /// The verified CGMES spelling, exactly as the source carries it (the
    /// `Class.attribute` local name an RDF/XML profile element uses).
    pub incoming_spelling: &'static str,
    /// The canonical declared key of [`crate::model`] the alias maps onto.
    pub canonical_key: &'static str,
    /// The source citation: the 44a record row and the [S-17x] source that
    /// verified the spelling (the honesty rule: every remediation cites its
    /// record and its source class).
    pub citation: &'static str,
}

/// One VERIFIED-ABSENT declared key: record 44a §3 verified the declared
/// spelling ABSENT from the CGMES profile set and NO replacement spelling
/// was verified. NOT an alias — the declared key stays the DECLARED import
/// vocabulary (accepted as before, pinned by the existing fixtures), and
/// this register records WHY it is not a CGMES attribute spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedAbsentKey {
    /// The declared key of [`crate::model`] verified absent.
    pub declared_key: &'static str,
    /// The source citation (the 44a record row and its [S-17x] source).
    pub citation: &'static str,
    /// The named not-modelled statement: what the sources verified instead
    /// and why no alias is written (the recorded-reason pattern of the
    /// class-level EXPLICITLY_UNMAPPED register).
    pub not_modelled: &'static str,
}

// --- The accepted aliases (2) -------------------------------------------------
//
// Every alias below cites its 44a record row and the source that verified
// BOTH halves of the mapping: the declared spelling's absence AND the
// incoming spelling's presence.

/// The accepted-alias table. EXACT spelling matches only (case-sensitive,
/// full `Class.attribute` local name) — a prefix, suffix, or case-insensitive
/// match would be a guess, never an acceptance.
pub const ALIASES: [SpellingAlias; 2] = [
    SpellingAlias {
        // 44a §3 row 1 — MISMATCH (verified): [S-173] verbatim, "it is
        // characterized by attribute BaseVoltage.nominalVoltage, which
        // could optionally serve as name"; [S-174] carries
        // `BaseVoltage.nominalVoltage`. The declared key
        // `BaseVoltage.nominalV` serves the same nominal-level attribute
        // (kV).
        incoming_spelling: "BaseVoltage.nominalVoltage",
        canonical_key: ATTR_BASE_VOLTAGE_NOMINAL_KV,
        citation: "44a §3 #1; [S-173] verbatim BaseVoltage.nominalVoltage; \
                   [S-174] carries BaseVoltage.nominalVoltage",
    },
    SpellingAlias {
        // 44a §3 row 8 — MISMATCH (verified): [S-174] carries the ratedS
        // owners `PowerTransformerEnd.ratedS`, `AsynchronousMachine.ratedS`,
        // `RotatingMachine.ratedS`, `PowerElectronicsConnection.ratedS` —
        // NOT `PowerTransformer.ratedS`. Only the END class' rating is the
        // transformer's own rating (the machine/connection ratings are
        // DIFFERENT equipment's attributes — never aliased). The declared
        // key `PowerTransformer.ratedS` serves the transformer rating
        // attribute (VA; the mapping converts to canonical MVA). The
        // end-object traversal (ratedS on separate PowerTransformerEnd
        // objects, record 44 §4 item 4) stays UNVERIFIED — this alias
        // normalizes the SPELLING only, never the structure.
        incoming_spelling: "PowerTransformerEnd.ratedS",
        canonical_key: ATTR_TRANSFORMER_RATED_S_VA,
        citation: "44a §3 #8; [S-174] ratedS owners include \
                   PowerTransformerEnd.ratedS",
    },
];

// --- The verified-absent register (9) -----------------------------------------
//
// The MISMATCH verdicts with NO verified replacement spelling. Each entry
// is the named not-modelled statement a reviewer reads; none of these keys
// is an alias, and none of the verified carrier facts below is reinterpreted
// into a mapping the sources do not carry.

/// The verified-absent register. Order: the record's table order.
pub const VERIFIED_ABSENT: [VerifiedAbsentKey; 9] = [
    VerifiedAbsentKey {
        // 44a §3 row 2 — MISMATCH (verified): [S-174] carries
        // `ConnectivityNode.ConnectivityNodeContainer` and
        // `ConnectivityNode.TopologicalNode` — no direct BaseVoltage
        // attribute. The container/topological traversal to a node voltage
        // is UNVERIFIED in detail (record 44 §4 item 5).
        declared_key: ATTR_NODE_BASE_VOLTAGE_REF,
        citation: "44a §3 #2; [S-174] ConnectivityNode attributes carried are \
                   ConnectivityNodeContainer / TopologicalNode",
        not_modelled: "the declared direct BaseVoltage reference is the DECLARED import \
                       vocabulary, not a CGMES attribute spelling; the container/topological \
                       traversal is UNVERIFIED in detail, so no alias is written — a node \
                       voltage path through VoltageLevel/TopologicalNode would need its own \
                       reviewed mapping change",
    },
    VerifiedAbsentKey {
        // 44a §3 row 5 — MISMATCH (verified): [S-174] carries NO
        // `ACLineSegment.ratedCurrent`; the only `ratedCurrent` carried is
        // `Switch.ratedCurrent` — a DIFFERENT class' attribute (a switch's
        // rating is not a line's). The line's real carrier (OperationalLimitSet
        // per record 44 §4 item 3) is confirmed in direction only; the
        // limit-set attribute path is UNVERIFIED in detail.
        declared_key: ATTR_LINE_RATED_CURRENT_A,
        citation: "44a §3 #5; [S-174] no ACLineSegment.ratedCurrent; the only ratedCurrent \
                   carried is Switch.ratedCurrent",
        not_modelled: "no alias is written: Switch.ratedCurrent is another equipment class' \
                       attribute and mapping it onto the branch rating would fabricate a \
                       line rating the sources do not carry; the OperationalLimitSet carrier \
                       path is UNVERIFIED in detail and stays the recorded work item",
    },
    VerifiedAbsentKey {
        // 44a §3 row 6 — MISMATCH (verified): [S-174] has zero occurrences
        // of any fromBus qualified attribute; [S-173] carries connectivity
        // through Terminals. The declared key is an import-shorthand
        // decision, not a CIM attribute.
        declared_key: ATTR_LINE_FROM_BUS_REF,
        citation: "44a §3 #6; [S-174] zero fromBus occurrences; [S-173] connectivity \
                   through Terminals",
        not_modelled: "the declared from-bus reference is an import shorthand the profile \
                       set does not carry; the Terminal-based connectivity traversal is the \
                       real path and stays the recorded UNVERIFIED work item — no alias",
    },
    VerifiedAbsentKey {
        // 44a §3 row 7 — MISMATCH (verified): same evidence as row 6.
        declared_key: ATTR_LINE_TO_BUS_REF,
        citation: "44a §3 #7; [S-174] zero toBus occurrences; [S-173] connectivity \
                   through Terminals",
        not_modelled: "the declared to-bus reference is an import shorthand the profile set \
                       does not carry; see the from-bus statement — no alias",
    },
    VerifiedAbsentKey {
        // 44a §3 row 11 — MISMATCH (verified): [S-174] zero occurrences;
        // [S-173] names the two-winding ends through `PowerTransformerEnd` +
        // `TransformerEnd.endNumber` ("the high voltage PowerTransformerEnd …
        // the low voltage PowerTransformerEnd"). The end→hv/lv assignment
        // traversal is UNVERIFIED in detail.
        declared_key: ATTR_TRANSFORMER_HV_BUS_REF,
        citation: "44a §3 #11; [S-174] zero occurrences; [S-173] the two-winding ends are \
                   PowerTransformerEnd objects with TransformerEnd.endNumber",
        not_modelled: "the declared high-voltage bus reference is an import shorthand; the \
                       end-number → hv/lv traversal is UNVERIFIED in detail, so no alias is \
                       written and no end attribute is reinterpreted as a bus reference",
    },
    VerifiedAbsentKey {
        // 44a §3 row 12 — MISMATCH (verified): same evidence as row 11.
        declared_key: ATTR_TRANSFORMER_LV_BUS_REF,
        citation: "44a §3 #12; [S-174] zero occurrences; [S-173] the two-winding ends are \
                   PowerTransformerEnd objects with TransformerEnd.endNumber",
        not_modelled: "the declared low-voltage bus reference is an import shorthand; see the \
                       high-voltage statement — no alias",
    },
    VerifiedAbsentKey {
        // 44a §3 row 13 — MISMATCH (verified): [S-174] carries the
        // ExternalNetworkInjection attributes p / q / minP / maxP / minQ /
        // maxQ — no bus/reference attribute. [S-173] carries the external
        // injection at the TopologicalNode boundary (record 44 §4 item 9
        // context); the exact reference path is UNVERIFIED in detail.
        declared_key: ATTR_EXTERNAL_GRID_BUS_REF,
        citation: "44a §3 #13; [S-174] ExternalNetworkInjection attributes carried are \
                   p/q/minP/maxP/minQ/maxQ",
        not_modelled: "the declared slack-bus reference is the DECLARED import vocabulary; \
                       the TopologicalNode-boundary reference path is UNVERIFIED in detail, \
                       so no alias is written and no p/q setpoint is reinterpreted as a \
                       reference",
    },
    VerifiedAbsentKey {
        // 44a §3 row 14 — MISMATCH (verified): [S-174] zero occurrences of
        // `vmPu`; the magnitude the SV profile carries (candidate:
        // targetVsMagnitude) is NOT present in either source — UNVERIFIED as
        // to the actual spelling.
        declared_key: ATTR_EXTERNAL_GRID_VM_PU,
        citation: "44a §3 #14; [S-174] zero vmPu occurrences; the candidate \
                   targetVsMagnitude spelling is itself UNVERIFIED",
        not_modelled: "no alias is possible: the actual CGMES magnitude spelling is itself \
                       UNVERIFIED (the record's verdict names no verified carrier), and \
                       guessing one would violate the no-hallucinated-facts rule",
    },
    VerifiedAbsentKey {
        // 44a §3 row 15 — MISMATCH (verified): [S-174] zero occurrences of
        // `vaDegree`; actual angle spelling UNVERIFIED.
        declared_key: ATTR_EXTERNAL_GRID_VA_DEGREE,
        citation: "44a §3 #15; [S-174] zero vaDegree occurrences; actual angle spelling \
                   UNVERIFIED",
        not_modelled: "no alias is possible: the actual CGMES angle spelling is itself \
                       UNVERIFIED (the record's verdict names no verified carrier), and \
                       guessing one would violate the no-hallucinated-facts rule",
    },
];

/// Resolves ONE attribute-element local name onto its canonical declared
/// key: a verified alias first (exact match), then the DECLARED vocabulary
/// (every declared key accepted exactly as before this module existed).
/// `None` = not accepted — the parser drops the element with its recorded
/// warning, exactly as before (never guessed into a key).
pub fn resolve_attribute_key(local: &str) -> Option<&'static str> {
    for alias in ALIASES.iter() {
        if alias.incoming_spelling == local {
            return Some(alias.canonical_key);
        }
    }
    crate::xml_profile::declared_attribute_key(local)
}

/// The accepted-alias count (2 — the verified replacement spellings).
pub const fn alias_count() -> usize {
    ALIASES.len()
}

/// The verified-absent count (9 — the MISMATCH verdicts with no verified
/// replacement spelling).
pub const fn verified_absent_count() -> usize {
    VERIFIED_ABSENT.len()
}

/// The named not-modelled statements of the verified-absent register, as
/// human-readable records (the same travel-with-the-record pattern the
/// coverage-limitation statements use). Ordered by the record's table order.
pub fn verified_absent_statements() -> Vec<String> {
    VERIFIED_ABSENT
        .iter()
        .map(
            |absent| {
                format!(
                    "VERIFIED_ABSENT {}: {} ({})",
                    absent.declared_key, absent.not_modelled, absent.citation
                )
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_alias_table_counts_exactly_the_verified_replacement_spellings() {
        // THE HONEST COUNT (see the module header): 2 accepted aliases —
        // only the MISMATCH verdicts of record 44a §3 where a replacement
        // spelling was VERIFIED in a primary source. The wave task's
        // "alias count == 9" is NOT honestly satisfiable: 9 is the
        // VERIFIED-ABSENT count (asserted below), and 2 + 9 = 11 = every
        // MISMATCH (verified) verdict of the record's table. A 9-alias
        // table would have to map spellings the record never verified —
        // hallucinated facts, refused (AGENTS.md rule 1).
        assert_eq!(alias_count(), 2);
        assert_eq!(verified_absent_count(), 9);
        assert_eq!(
            alias_count() + verified_absent_count(),
            11,
            "every MISMATCH (verified) verdict of record 44a §3 is remediated exactly once"
        );
    }

    #[test]
    fn every_alias_maps_onto_a_declared_constant_never_a_new_key() {
        let declared_keys = [
            crate::model::ATTR_BASE_VOLTAGE_NOMINAL_KV,
            crate::model::ATTR_NODE_BASE_VOLTAGE_REF,
            crate::model::ATTR_LINE_R_OHM,
            crate::model::ATTR_LINE_X_OHM,
            crate::model::ATTR_LINE_RATED_CURRENT_A,
            crate::model::ATTR_LINE_FROM_BUS_REF,
            crate::model::ATTR_LINE_TO_BUS_REF,
            crate::model::ATTR_TRANSFORMER_RATED_S_VA,
            crate::model::ATTR_TRANSFORMER_VK_PERCENT,
            crate::model::ATTR_TRANSFORMER_VK_R_PERCENT,
            crate::model::ATTR_TRANSFORMER_HV_BUS_REF,
            crate::model::ATTR_TRANSFORMER_LV_BUS_REF,
            crate::model::ATTR_EXTERNAL_GRID_BUS_REF,
            crate::model::ATTR_EXTERNAL_GRID_VM_PU,
            crate::model::ATTR_EXTERNAL_GRID_VA_DEGREE,
        ];
        for alias in ALIASES.iter() {
            assert!(
                declared_keys.contains(&alias.canonical_key),
                "the alias '{}' must normalize onto an EXISTING declared constant, \
                 never a new key: {}",
                alias.incoming_spelling,
                alias.canonical_key
            );
            assert!(
                !declared_keys.contains(&alias.incoming_spelling),
                "an alias must not collide with (or duplicate) a declared key: {}",
                alias.incoming_spelling
            );
            assert!(
                alias.citation.contains("44a") && alias.citation.contains("[S-17"),
                "every alias cites the 44a record and its [S-17x] source: {}",
                alias.citation
            );
        }
    }

    #[test]
    fn every_verified_absent_entry_cites_the_record_and_names_the_statement() {
        for absent in VERIFIED_ABSENT.iter() {
            assert!(
                absent.citation.contains("44a") && absent.citation.contains("[S-17"),
                "every verified-absent entry cites the 44a record and its source: {}",
                absent.citation
            );
            assert!(
                absent.not_modelled.len() > 40
                    && (absent.not_modelled.contains("no alias")
                        || absent.not_modelled.contains("UNVERIFIED")),
                "the not-modelled statement names why no alias exists: {}",
                absent.not_modelled
            );
        }
    }

    #[test]
    fn the_declared_keys_are_accepted_exactly_as_before() {
        // The ADDITIVE rule: every declared constant resolves to ITSELF
        // (the pre-remediation behavior, unchanged).
        for key in [
            crate::model::ATTR_BASE_VOLTAGE_NOMINAL_KV,
            crate::model::ATTR_NODE_BASE_VOLTAGE_REF,
            crate::model::ATTR_LINE_R_OHM,
            crate::model::ATTR_LINE_X_OHM,
            crate::model::ATTR_LINE_RATED_CURRENT_A,
            crate::model::ATTR_LINE_FROM_BUS_REF,
            crate::model::ATTR_LINE_TO_BUS_REF,
            crate::model::ATTR_TRANSFORMER_RATED_S_VA,
            crate::model::ATTR_TRANSFORMER_VK_PERCENT,
            crate::model::ATTR_TRANSFORMER_VK_R_PERCENT,
            crate::model::ATTR_TRANSFORMER_HV_BUS_REF,
            crate::model::ATTR_TRANSFORMER_LV_BUS_REF,
            crate::model::ATTR_EXTERNAL_GRID_BUS_REF,
            crate::model::ATTR_EXTERNAL_GRID_VM_PU,
            crate::model::ATTR_EXTERNAL_GRID_VA_DEGREE,
        ] {
            assert_eq!(
                resolve_attribute_key(key),
                Some(key),
                "the declared key stays accepted unchanged: {key}"
            );
        }
    }

    #[test]
    fn the_verified_aliases_resolve_onto_the_canonical_keys() {
        assert_eq!(
            resolve_attribute_key("BaseVoltage.nominalVoltage"),
            Some(crate::model::ATTR_BASE_VOLTAGE_NOMINAL_KV)
        );
        assert_eq!(
            resolve_attribute_key("PowerTransformerEnd.ratedS"),
            Some(crate::model::ATTR_TRANSFORMER_RATED_S_VA)
        );
    }

    #[test]
    fn near_misses_and_case_swaps_are_never_accepted() {
        // Exact-match discipline: a prefix, a suffix, a case variant, or a
        // different class owner is NOT accepted (never guessed).
        assert_eq!(resolve_attribute_key("BaseVoltage.nominalvoltage"), None);
        assert_eq!(resolve_attribute_key("BaseVoltage.nominal"), None);
        assert_eq!(resolve_attribute_key("BaseVoltage.nominalVoltages"), None);
        assert_eq!(resolve_attribute_key("PowerTransformer.nominalVoltage"), None);
        assert_eq!(resolve_attribute_key("PowerTransformerEnd.rateds"), None);
        assert_eq!(resolve_attribute_key("AsynchronousMachine.ratedS"), None);
        assert_eq!(resolve_attribute_key("Switch.ratedCurrent"), None);
        assert_eq!(resolve_attribute_key("ConnectivityNode.TopologicalNode"), None);
        assert_eq!(resolve_attribute_key("ACLineSegment.bogusPredicate"), None);
    }

    #[test]
    fn the_verified_absent_statements_travel_as_named_records() {
        let statements = verified_absent_statements();
        assert_eq!(statements.len(), 9);
        for statement in &statements {
            assert!(
                statement.starts_with("VERIFIED_ABSENT "),
                "each statement is a named record: {statement}"
            );
        }
        // The register covers exactly the keys the record verified absent
        // (the task's not-alias list plus the line rated-current key whose
        // only verified carrier is another class' attribute).
        let keys: Vec<&str> = VERIFIED_ABSENT.iter().map(|a| a.declared_key).collect();
        for expected in [
            crate::model::ATTR_NODE_BASE_VOLTAGE_REF,
            crate::model::ATTR_LINE_RATED_CURRENT_A,
            crate::model::ATTR_LINE_FROM_BUS_REF,
            crate::model::ATTR_LINE_TO_BUS_REF,
            crate::model::ATTR_TRANSFORMER_HV_BUS_REF,
            crate::model::ATTR_TRANSFORMER_LV_BUS_REF,
            crate::model::ATTR_EXTERNAL_GRID_BUS_REF,
            crate::model::ATTR_EXTERNAL_GRID_VM_PU,
            crate::model::ATTR_EXTERNAL_GRID_VA_DEGREE,
        ] {
            assert!(
                keys.contains(&expected),
                "the verified-absent register covers {expected}"
            );
        }
    }
}
