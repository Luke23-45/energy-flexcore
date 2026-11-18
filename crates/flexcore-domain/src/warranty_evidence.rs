// P15.11 — The warranty evidence-pack generator (pure domain, no I/O).
//
// Decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 25_p15_11_warranty_evidence_pack.md`.
//
// WHAT THIS MODULE IS: the export path from the P15.07 degradation ledger
// (and the P15.10 warranty terms + headroom report) to a TIMESTAMPED,
// DIGEST-CHAINED claim-SUPPORT pack: an ordered, deterministic list of
// entries whose digests form a chain, sealed by a provenance record that
// mirrors the P8.32 model-artifact provenance-record shape.
//
// THE P8.32 MIRROR (by citation, NO dependency — the domain crate depends
// on flexcore-contracts only, and flexcore-research is out of reach): the
// provenance record carries the P8.32 record shape — the versioned schema
// id, the artifact ref (here: the pack id), the chain digest, the digest
// ALGORITHM, the signer key REFERENCE (scheme-prefixed only), and the
// generation time — with the source references LINKED TO THE RECORD, not
// stored beside it where they could drift (the P8.32 rule, mirrored).
// A key reference without a scheme prefix is refused as likely inline key
// material, and the offending value is NEVER echoed back (the P7.05
// secret-reference rule, mirrored).
//
// THE DIGEST IS TEST-GRADE (the P8.32 honesty, mirrored): the chain digest
// is FNV-1a 64 ([`fnv1a64_test`]) — NOT a security primitive, never claimed
// as a release signature. Wiring a production SHA-256 (or stronger) pack
// digest is a DEPLOYMENT REQUIREMENT recorded as a decision point; the
// algorithm name `sha-256` is pre-registered in the algorithm vocabulary so
// the deployment wiring does not need a schema change, and acceptance is
// meaningless until a real verifier exists.
//
// THE CLAIM BOUNDARY (the decisive line): the pack **SUPPORTS human warranty
// claims; it NEVER asserts them** ([`CLAIM_SUPPORT_STATEMENT`]). The pack is
// evidence-for-a-human; no automated claim, no warranty verdict, no
// compensation action, and no dispatch action is produced or authorized by
// its existence (ADR-010, conventions §6.3).
//
// DETERMINISM: entries are generated in a fixed key order (sorted); the
// digest chain is a pure function of (schema id, pack id, generation time,
// entries); the same inputs produce a byte-identical pack. This is the
// BTreeMap/sorted-iteration determinism posture the kernel patterns carry.
//
// Control status: pure domain records for the simulation-only 0.x line. No
// I/O, no solver coupling, no device-write path, no dispatch reach.

use crate::bess_degradation::{BessDegradationLedger, DEGRADATION_LEDGER_STATEMENT};
use crate::bess_soh::SOH_ESTIMATION_DEFERRAL_STATEMENT;
use crate::bess_warranty::{WarrantyHeadroomReport, WarrantyTerms, WARRANTY_REFERENCE_STRUCTURE_STATEMENT};

// --- Recorded identities and vocabularies --------------------------------------

/// The versioned evidence-pack envelope id. A pack whose schema field is not
/// exactly this value is not this pack shape.
pub const EVIDENCE_PACK_SCHEMA_ID: &str = "flexcore-warranty-evidence-pack-v1";

/// The versioned provenance-record envelope id (the P8.32 record shape,
/// mirrored for warranty evidence).
pub const EVIDENCE_PROVENANCE_SCHEMA_ID: &str = "flexcore-warranty-evidence-provenance-v1";

/// The TEST-GRADE digest algorithm carried by packs generated here: FNV-1a
/// 64 over the chained bytes. NOT a security primitive; never acceptable as
/// a release signature.
pub const EVIDENCE_DIGEST_ALGORITHM_TEST: &str = "fnv1a64-test";

/// The digest algorithm a PRODUCTION deployment must wire (DEPLOYMENT
/// REQUIREMENT — not implemented in this crate; the P8.32 posture,
/// mirrored). Pre-registered so the deployment wiring needs no schema
/// change; acceptance is meaningless until a real verifier exists.
pub const EVIDENCE_DIGEST_ALGORITHM_SHA256: &str = "sha-256";

/// The closed digest-algorithm vocabulary for the provenance record.
pub const EVIDENCE_DIGEST_ALGORITHMS: [&str; 2] =
    [EVIDENCE_DIGEST_ALGORITHM_TEST, EVIDENCE_DIGEST_ALGORITHM_SHA256];

/// The accepted signer-key-reference schemes (the P8.32 schemes, mirrored).
pub const EVIDENCE_SIGNER_KEY_REFERENCE_SCHEMES: [&str; 3] =
    ["key-store:", "env:", "file:"];

/// The claim-boundary statement (carried INSIDE every pack, not a comment).
pub const CLAIM_SUPPORT_STATEMENT: &str =
    "This pack SUPPORTS a human warranty claim; it does NOT assert one. It is \
evidence-for-a-human: no automated claim, warranty verdict, compensation \
action, or dispatch action is produced or authorized by its existence \
(ADR-010). The recipient evaluates the evidence; FlexCore records it.";

/// The pack's recorded digest-grade statement (visible in the pack, so no
/// reader can mistake the test-grade chain for a release signature).
pub const EVIDENCE_DIGEST_GRADE_STATEMENT: &str =
    "The digest chain is FNV-1a 64 (test-grade, NOT a security primitive). A \
production SHA-256 (or stronger) pack digest is a DEPLOYMENT REQUIREMENT \
recorded as a decision point; no cryptographic signature exists in this \
delivery. The chain provides tamper-EVIDENCE for review, not tamper-PROOF \
for release.";

// --- Errors ------------------------------------------------------------------

/// Errors of evidence-pack generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidencePackError {
    /// A required text field was empty or whitespace.
    EmptyField {
        /// The offending field.
        field: &'static str,
    },
    /// A text field exceeded its declared bound.
    FieldTooLong {
        /// The offending field.
        field: &'static str,
        /// The recorded bound.
        max_chars: usize,
    },
    /// The signer key reference lacked a scheme prefix — refused as likely
    /// inline key material. The offered value is NEVER echoed back (the
    /// P7.05 secret-reference rule, mirrored).
    InlineKeyMaterialRefused,
    /// The declared digest algorithm is outside the closed vocabulary.
    UnknownDigestAlgorithm {
        /// The offending algorithm name.
        algorithm: String,
    },
}

impl std::fmt::Display for EvidencePackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidencePackError::EmptyField { field } => {
                write!(f, "{field} must be non-empty and non-whitespace")
            }
            EvidencePackError::FieldTooLong { field, max_chars } => {
                write!(f, "{field} exceeds the {max_chars}-character bound")
            }
            EvidencePackError::InlineKeyMaterialRefused => write!(
                f,
                "the signer key reference lacks a scheme prefix (expected one of \
                 key-store:, env:, file:) and is refused as likely inline key \
                 material; the offered value is not echoed back (P7.05)"
            ),
            EvidencePackError::UnknownDigestAlgorithm { algorithm } => write!(
                f,
                "digest algorithm '{algorithm}' is outside the closed vocabulary"
            ),
        }
    }
}

impl std::error::Error for EvidencePackError {}

// --- The digest -----------------------------------------------------------------

/// The TEST-GRADE FNV-1a 64 digest (the repo's non-cryptographic
/// integrity-checksum pattern, P4.28/P6.14/P7.14 — mirrored from the P8.32
/// test digest). NOT a security primitive; never claimed as a release
/// signature.
pub fn fnv1a64_test(bytes: &[u8]) -> u64 {
    // FNV-1a 64-bit: offset basis 14695981039346656037, prime 1099511628211.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// The canonical entry byte form the chain digests over:
/// `{seq}|{key}|{value}`. Part of the module's API contract; `verify_chain`
/// re-digests exactly this form.
pub fn canonical_entry(seq: u64, key: &str, value: &str) -> Vec<u8> {
    format!("{seq}|{key}|{value}").into_bytes()
}

// --- Pack types -------------------------------------------------------------------

/// One evidence entry: its sequence, its fixed-vocabulary key, its rendered
/// value, and the cumulative chain digest AFTER absorbing this entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceEntry {
    /// The 0-based sequence in the pack (entries are chain-ordered).
    pub seq: u64,
    /// The fixed-vocabulary key.
    pub key: String,
    /// The rendered value (deterministic float formatting).
    pub value: String,
    /// The cumulative chain digest after this entry (16 lowercase hex chars).
    pub digest_hex: String,
}

/// The provenance record sealing the pack (the P8.32 record shape,
/// mirrored): the schema id, the artifact ref, the chain digest, the digest
/// algorithm, the signer key REFERENCE, the generation time, and the source
/// references LINKED TO THE RECORD (not stored beside it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProvenanceRecord {
    /// Always [`EVIDENCE_PROVENANCE_SCHEMA_ID`].
    pub provenance_schema: &'static str,
    /// The artifact reference (the pack id).
    pub artifact_ref: String,
    /// The pack's chain-head digest (16 lowercase hex chars for the
    /// test-grade algorithm).
    pub digest: String,
    /// The digest algorithm (one of [`EVIDENCE_DIGEST_ALGORITHMS`]).
    pub digest_algorithm: String,
    /// The scheme-prefixed signer key REFERENCE (key material is
    /// structurally impossible here: there is no field a key could hide in).
    pub signer_key_reference: String,
    /// The generation time (unix millis; the caller-supplied timestamp —
    /// this module never reads a clock).
    pub generated_at_unix_ms: i64,
    /// The source references, LINKED TO THE RECORD: the degradation-ledger
    /// identity and the warranty identity the pack was generated from.
    pub source_refs: Vec<String>,
}

/// The evidence pack (P15.11): timestamped, digest-chained, provenance-
/// sealed. A SUPPORT artifact for human warranty claims — never an asserted
/// claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidencePack {
    /// Always [`EVIDENCE_PACK_SCHEMA_ID`].
    pub schema_id: &'static str,
    /// The caller-supplied pack identifier (non-empty, bounded).
    pub pack_id: String,
    /// The generation timestamp (unix millis; caller-supplied — this module
    /// never reads a clock).
    pub generated_at_unix_ms: i64,
    /// The entries in chain order.
    pub entries: Vec<EvidenceEntry>,
    /// The chain-head digest (16 lowercase hex chars for the test-grade
    /// algorithm).
    pub chain_head_digest: String,
    /// The sealing provenance record.
    pub provenance: EvidenceProvenanceRecord,
    /// Always [`CLAIM_SUPPORT_STATEMENT`] — the claim boundary travels in
    /// the pack.
    pub claim_support_statement: &'static str,
    /// Always [`EVIDENCE_DIGEST_GRADE_STATEMENT`].
    pub digest_grade_statement: &'static str,
}

impl EvidencePack {
    /// Re-digests the chain from the entries and compares against the
    /// recorded digests: `true` when every entry digest and the chain head
    /// match. Tamper-EVIDENCE for review, not tamper-PROOF for release
    /// ([`EVIDENCE_DIGEST_GRADE_STATEMENT`]).
    pub fn verify_chain(&self) -> bool {
        let mut chain = chain_seed(self.schema_id, &self.pack_id, self.generated_at_unix_ms);
        for entry in &self.entries {
            chain = chain_step(chain, entry.seq, &entry.key, &entry.value);
            if chain_hex(chain) != entry.digest_hex {
                return false;
            }
        }
        chain_hex(chain) == self.chain_head_digest
    }
}

fn chain_seed(schema_id: &str, pack_id: &str, generated_at_unix_ms: i64) -> u64 {
    fnv1a64_test(format!("{schema_id}|{pack_id}|{generated_at_unix_ms}").as_bytes())
}

fn chain_step(previous: u64, seq: u64, key: &str, value: &str) -> u64 {
    let mut bytes = format!("{previous:016x}").into_bytes();
    bytes.extend_from_slice(&canonical_entry(seq, key, value));
    fnv1a64_test(&bytes)
}

fn chain_hex(chain: u64) -> String {
    format!("{chain:016x}")
}

// --- Generation --------------------------------------------------------------------

/// Generates the evidence pack from the P15.07 ledger, the P15.10 warranty
/// terms, and the P15.10 headroom report. Pure function of the supplied
/// data and the caller-supplied timestamp — this module never reads a clock.
///
/// Determinism: the entry list is generated in a fixed key vocabulary and
/// sorted by key; the digest chain is a pure function of the inputs. The
/// same inputs produce a byte-identical pack.
pub fn generate_evidence_pack(
    ledger: &BessDegradationLedger,
    terms: &WarrantyTerms,
    headroom: &WarrantyHeadroomReport,
    pack_id: String,
    generated_at_unix_ms: i64,
    signer_key_reference: &str,
) -> Result<EvidencePack, EvidencePackError> {
    // --- caller-supplied identity validation -------------------------------
    if pack_id.trim().is_empty() {
        return Err(EvidencePackError::EmptyField { field: "pack_id" });
    }
    if pack_id.len() > 256 {
        return Err(EvidencePackError::FieldTooLong { field: "pack_id", max_chars: 256 });
    }
    let scheme_ok = EVIDENCE_SIGNER_KEY_REFERENCE_SCHEMES
        .iter()
        .any(|scheme| signer_key_reference.starts_with(scheme));
    if !scheme_ok {
        // Refuse WITHOUT echoing the offered value (P7.05, mirrored).
        return Err(EvidencePackError::InlineKeyMaterialRefused);
    }

    // --- entries -------------------------------------------------------------
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut push = |key: &str, value: String| {
        entries.push((key.to_string(), value));
    };

    // Pack identity and the claim boundary (the statements travel IN the
    // pack).
    push("statement.claim_support", CLAIM_SUPPORT_STATEMENT.to_string());
    push("statement.digest_grade", EVIDENCE_DIGEST_GRADE_STATEMENT.to_string());
    push("statement.degradation_ledger", DEGRADATION_LEDGER_STATEMENT.to_string());
    push("statement.soh_deferral", SOH_ESTIMATION_DEFERRAL_STATEMENT.to_string());
    push(
        "statement.warranty_reference_structures",
        WARRANTY_REFERENCE_STRUCTURE_STATEMENT.to_string(),
    );

    // Ledger identity.
    push("pack.asset_id", format!("{}", ledger.asset_id));
    push("pack.chemistry", ledger.chemistry.label());
    push(
        "pack.chemistry_in_scope",
        ledger.chemistry.is_in_scope().to_string(),
    );
    push(
        "pack.usable_energy_mwh",
        format!(
            "value={}; provenance={}",
            ledger.usable_energy_mwh.value,
            ledger.usable_energy_mwh.provenance.name()
        ),
    );

    // Counters (P15.07) — every counter with its merged provenance and
    // observation count, so an UNKNOWN-provenance counter is visible as
    // such in the export (never silently presented as known data).
    push("counters.creditable", ledger.counters_creditable().to_string());
    for (key, counter) in [
        ("counters.cumulative_charge_mwh", &ledger.cumulative_charge_mwh),
        ("counters.cumulative_discharge_mwh", &ledger.cumulative_discharge_mwh),
        ("counters.cumulative_throughput_mwh", &ledger.cumulative_throughput_mwh),
        ("counters.full_equivalent_cycles", &ledger.full_equivalent_cycles),
    ] {
        push(
            key,
            format!(
                "value={}; provenance={}; observations={}",
                counter.value(),
                counter.provenance().name(),
                counter.observations()
            ),
        );
    }

    // Histogram (P15.07) — totals, provenance, and per-cell counts.
    let histogram = &ledger.dod_mean_soc_histogram;
    push(
        "histogram.total_cycles",
        histogram.total_cycles().to_string(),
    );
    push(
        "histogram.provenance",
        histogram.provenance().name().to_string(),
    );
    for (index, count) in histogram.counts().iter().enumerate() {
        let _dod_bins = histogram.dod_bin_edges().len() - 1;
        let soc_bins = histogram.mean_soc_bin_edges().len() - 1;
        let dod_bin = index / soc_bins.max(1);
        let soc_bin = index % soc_bins.max(1);
        push(
            &format!("histogram.cell.dod{dod_bin}.soc{soc_bin}"),
            count.to_string(),
        );
    }

    // Temperature exposure and dwell.
    let exposure = &ledger.temperature_exposure;
    push(
        "temperature.provenance",
        exposure.provenance().name().to_string(),
    );
    for (band, seconds) in exposure.seconds_per_band().iter().enumerate() {
        push(
            &format!("temperature.band{band}_seconds"),
            seconds.to_string(),
        );
    }
    push(
        "dwell.threshold_soc",
        format!("{}", ledger.high_soc_dwell.soc_threshold()),
    );
    push(
        "dwell.seconds",
        ledger.high_soc_dwell.dwell_seconds().to_string(),
    );
    push(
        "dwell.provenance",
        ledger.high_soc_dwell.provenance().name().to_string(),
    );

    // SoH observations (append order).
    for (index, observation) in ledger.soh_observations.iter().enumerate() {
        push(
            &format!("soh.observation{index}"),
            format!(
                "soh={}; observed_at={}; provenance={}; source={}",
                observation.soh_fraction,
                observation.observed_at_unix_ms,
                observation.provenance.name(),
                observation.source
            ),
        );
    }

    // Warranty terms (P15.10).
    push("warranty.id", terms.warranty_id.clone());
    push("warranty.citation", terms.citation.clone());
    push(
        "warranty.provenance",
        terms.provenance.name().to_string(),
    );
    push(
        "warranty.valid_from_unix_ms",
        terms.valid_from_unix_ms.to_string(),
    );
    push(
        "warranty.valid_until_unix_ms",
        terms.valid_until_unix_ms.to_string(),
    );
    for point in &terms.retention_schedule {
        push(
            &format!("warranty.retention.year{}", point.year_index),
            format!("{}", point.min_retention_fraction),
        );
    }
    if let Some(limit) = terms.max_full_equivalent_cycles {
        push("warranty.max_full_equivalent_cycles", format!("{limit}"));
    }
    if let Some(limit) = terms.max_cumulative_throughput_mwh {
        push("warranty.max_cumulative_throughput_mwh", format!("{limit}"));
    }
    if let Some(rte) = &terms.rte {
        push(
            "warranty.rte_declared",
            format!(
                "min={}; basis={}",
                rte.min_round_trip_efficiency_fraction, rte.measurement_basis
            ),
        );
    }
    if let Some(availability) = &terms.availability {
        push(
            "warranty.availability_declared",
            format!(
                "min={}; basis={}",
                availability.min_availability_fraction, availability.measurement_basis
            ),
        );
    }

    // Headroom report (P15.10) — the headroom values are RENDERED RECORDS;
    // the pack never asserts a claim from them.
    push(
        "headroom.evaluated_at_unix_ms",
        headroom.evaluated_at_unix_ms.to_string(),
    );
    push(
        "headroom.window",
        format!("{:?}", headroom.window),
    );
    push(
        "headroom.capacity_retention",
        format!("{:?}", headroom.capacity_retention),
    );
    push("headroom.cycles", format!("{:?}", headroom.cycles));
    push("headroom.throughput", format!("{:?}", headroom.throughput));
    push(
        "headroom.chemistry_in_scope",
        headroom.chemistry_in_scope.to_string(),
    );

    // --- deterministic ordering and the digest chain ------------------------
    entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut chain = chain_seed(EVIDENCE_PACK_SCHEMA_ID, &pack_id, generated_at_unix_ms);
    let mut sealed = Vec::with_capacity(entries.len());
    for (seq, (key, value)) in entries.iter().enumerate() {
        chain = chain_step(chain, seq as u64, key, value);
        sealed.push(EvidenceEntry {
            seq: seq as u64,
            key: key.clone(),
            value: value.clone(),
            digest_hex: chain_hex(chain),
        });
    }

    let chain_head = chain_hex(chain);
    let provenance = EvidenceProvenanceRecord {
        provenance_schema: EVIDENCE_PROVENANCE_SCHEMA_ID,
        artifact_ref: pack_id.clone(),
        digest: chain_head.clone(),
        digest_algorithm: EVIDENCE_DIGEST_ALGORITHM_TEST.to_string(),
        signer_key_reference: signer_key_reference.to_string(),
        generated_at_unix_ms,
        source_refs: vec![
            format!("degradation-ledger:{}", ledger.asset_id),
            format!("warranty:{}", terms.warranty_id),
        ],
    };

    Ok(EvidencePack {
        schema_id: EVIDENCE_PACK_SCHEMA_ID,
        pack_id,
        generated_at_unix_ms,
        entries: sealed,
        chain_head_digest: chain_head,
        provenance,
        claim_support_statement: CLAIM_SUPPORT_STATEMENT,
        digest_grade_statement: EVIDENCE_DIGEST_GRADE_STATEMENT,
    })
}

// --- Tests ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bess_degradation::{
        DegradationChemistry, DegradationLedgerError, SohObservation as LedgerSohObservation,
    };
    use flexcore_contracts::capability::Provenance;
    use crate::bess_model::ProvenancedValue;
    use crate::bess_soh::SohStalenessConfig;
    use crate::bess_warranty::{
        CapacityRetentionPoint, RteCommitment, WarrantyError,
    };
    use crate::ids::AssetId;
    use flexcore_contracts::ids::FlexUuid;

    const KEY_REF: &str = "key-store:warranty-evidence-signer";

    fn asset() -> AssetId {
        AssetId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8e").unwrap())
    }

    fn ledger() -> BessDegradationLedger {
        let mut ledger = BessDegradationLedger::new(
            asset(),
            DegradationChemistry::Lfp,
            ProvenancedValue::configured(10.0),
            vec![0.0, 0.5, 1.0],
            vec![0.0, 1.0],
            vec![0.0, 45.0],
            0.9,
        )
        .unwrap();
        ledger.record_throughput(100.0, 100.0, Provenance::Measured).unwrap();
        ledger
            .record_soh_observation(
                LedgerSohObservation::new(0.95, 1_000, Provenance::Measured, "bms-7".to_string())
                    .unwrap(),
            )
            .unwrap();
        ledger
    }

    fn terms() -> WarrantyTerms {
        WarrantyTerms::new(
            "warranty-2027-001".to_string(),
            0,
            10 * crate::bess_warranty::WARRANTY_YEAR_MILLIS,
            vec![CapacityRetentionPoint { year_index: 1, min_retention_fraction: 0.96 }],
            Some(6_000.0),
            Some(60_000.0),
            Some(RteCommitment {
                min_round_trip_efficiency_fraction: 0.86,
                measurement_basis: "declared basis".to_string(),
            }),
            None,
            "declared warranty citation".to_string(),
            Provenance::Configured,
        )
        .unwrap()
    }

    fn headroom(ledger: &BessDegradationLedger) -> WarrantyHeadroomReport {
        crate::bess_warranty::compute_headroom(
            &terms(),
            ledger,
            2_000,
            &SohStalenessConfig::default(),
        )
        .unwrap()
    }

    fn pack(ledger: &BessDegradationLedger) -> EvidencePack {
        generate_evidence_pack(
            ledger,
            &terms(),
            &headroom(ledger),
            "pack-001".to_string(),
            5_000,
            KEY_REF,
        )
        .unwrap()
    }

    // --- the FNV-1a 64 test digest -----------------------------------------

    #[test]
    fn fnv1a64_matches_the_recorded_test_vectors() {
        // Standard FNV-1a 64 test vectors ("", "a", "foobar").
        assert_eq!(fnv1a64_test(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64_test(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64_test(b"foobar"), 0x85944171f73967e8);
    }

    // --- generation and determinism -------------------------------------------

    #[test]
    fn pack_is_deterministic_for_identical_inputs() {
        let ledger = ledger();
        let a = pack(&ledger);
        let b = pack(&ledger);
        assert_eq!(a, b, "the same inputs must produce a byte-identical pack");
    }

    #[test]
    fn a_different_generation_timestamp_changes_the_chain() {
        let ledger = ledger();
        let a = pack(&ledger);
        let b = generate_evidence_pack(
            &ledger,
            &terms(),
            &headroom(&ledger),
            "pack-001".to_string(),
            6_000, // different timestamp
            KEY_REF,
        )
        .unwrap();
        assert_ne!(a.chain_head_digest, b.chain_head_digest);
    }

    #[test]
    fn every_entry_digest_absorbs_its_predecessor() {
        let pack = pack(&ledger());
        assert!(pack.verify_chain(), "the recorded chain must verify");
        // A tampered value breaks the chain from that entry onward.
        let mut tampered = pack.clone();
        tampered.entries[3].value.push('x');
        assert!(!tampered.verify_chain());
        // A tampered chain head fails verification.
        let mut tampered_head = pack.clone();
        tampered_head.chain_head_digest = "0000000000000000".to_string();
        assert!(!tampered_head.verify_chain());
    }

    #[test]
    fn entries_are_chain_ordered_and_key_sorted() {
        let pack = pack(&ledger());
        for (a, b) in pack.entries.iter().zip(pack.entries.iter().skip(1)) {
            assert_eq!(b.seq, a.seq + 1);
            assert!(
                a.key < b.key || (a.key == b.key && a.value <= b.value),
                "entries must be key-sorted for determinism (found '{}' then '{}')",
                a.key,
                b.key
            );
        }
    }

    // --- the claim boundary and statements ----------------------------------------

    #[test]
    fn the_pack_carries_the_claim_support_boundary_inside_it() {
        let pack = pack(&ledger());
        assert_eq!(pack.claim_support_statement, CLAIM_SUPPORT_STATEMENT);
        assert!(CLAIM_SUPPORT_STATEMENT.contains("does NOT assert one"));
        let support = pack
            .entries
            .iter()
            .find(|e| e.key == "statement.claim_support")
            .expect("the claim-support statement is an entry");
        assert_eq!(support.value, CLAIM_SUPPORT_STATEMENT);
    }

    #[test]
    fn the_digest_grade_statement_is_test_grade_and_not_a_release_claim() {
        let pack = pack(&ledger());
        assert_eq!(pack.provenance.digest_algorithm, EVIDENCE_DIGEST_ALGORITHM_TEST);
        assert!(pack.digest_grade_statement.contains("NOT a security primitive"));
        assert!(pack.digest_grade_statement.contains("DEPLOYMENT REQUIREMENT"));
        assert!(EVIDENCE_DIGEST_ALGORITHMS.contains(&EVIDENCE_DIGEST_ALGORITHM_SHA256));
    }

    // --- provenance record (P8.32 mirror) ------------------------------------------

    #[test]
    fn the_provenance_record_links_sources_to_the_record() {
        let pack = pack(&ledger());
        let provenance = &pack.provenance;
        assert_eq!(provenance.provenance_schema, EVIDENCE_PROVENANCE_SCHEMA_ID);
        assert_eq!(provenance.artifact_ref, "pack-001");
        assert_eq!(provenance.digest, pack.chain_head_digest);
        assert_eq!(provenance.generated_at_unix_ms, 5_000);
        assert_eq!(
            provenance.source_refs,
            vec![
                format!("degradation-ledger:{}", asset()),
                "warranty:warranty-2027-001".to_string(),
            ]
        );
    }

    #[test]
    fn inline_key_material_is_refused_without_echo() {
        let error = generate_evidence_pack(
            &ledger(),
            &terms(),
            &headroom(&ledger()),
            "pack-001".to_string(),
            5_000,
            "raw-inline-key-material-without-a-scheme",
        )
        .unwrap_err();
        assert_eq!(error, EvidencePackError::InlineKeyMaterialRefused);
        // The error rendering never echoes the offered value (P7.05).
        let rendered = error.to_string();
        assert!(
            !rendered.contains("raw-inline-key-material"),
            "the refusal must not leak the offered reference"
        );
        for scheme in EVIDENCE_SIGNER_KEY_REFERENCE_SCHEMES {
            let with_scheme = format!("{scheme}some-reference");
            assert!(
                generate_evidence_pack(
                    &ledger(),
                    &terms(),
                    &headroom(&ledger()),
                    "pack-002".to_string(),
                    5_000,
                    &with_scheme,
                )
                .is_ok(),
                "scheme-prefixed references are accepted"
            );
        }
    }

    #[test]
    fn pack_id_is_validated() {
        assert_eq!(
            generate_evidence_pack(
                &ledger(),
                &terms(),
                &headroom(&ledger()),
                "  ".to_string(),
                5_000,
                KEY_REF,
            )
            .unwrap_err(),
            EvidencePackError::EmptyField { field: "pack_id" }
        );
        let long = "x".repeat(257);
        assert_eq!(
            generate_evidence_pack(
                &ledger(),
                &terms(),
                &headroom(&ledger()),
                long,
                5_000,
                KEY_REF,
            )
            .unwrap_err(),
            EvidencePackError::FieldTooLong { field: "pack_id", max_chars: 256 }
        );
    }

    // --- provenance visibility in the export ------------------------------------------

    #[test]
    fn unknown_provenance_data_is_exported_with_its_label_visible() {
        let mut ledger = ledger();
        // An UNKNOWN-provenance increment narrows the counters; the pack must
        // show the UNKNOWN label — never silently present the value as
        // known data.
        ledger.record_throughput(0.0, 0.0, Provenance::Unknown).unwrap();
        let pack = pack(&ledger);
        let fec = pack
            .entries
            .iter()
            .find(|e| e.key == "counters.full_equivalent_cycles")
            .unwrap();
        assert!(fec.value.contains("provenance=UNKNOWN"), "found: {}", fec.value);
        let creditable = pack
            .entries
            .iter()
            .find(|e| e.key == "counters.creditable")
            .unwrap();
        assert_eq!(creditable.value, "false");
    }

    #[test]
    fn ledger_and_warranty_errors_surface_their_own_types() {
        // The evidence module surfaces construction errors of its inputs
        // through their own error types (no swallowing).
        assert!(matches!(
            WarrantyTerms::new(
                "w".to_string(),
                10,
                5,
                vec![CapacityRetentionPoint { year_index: 1, min_retention_fraction: 0.9 }],
                None,
                None,
                None,
                None,
                "c".to_string(),
                Provenance::Configured,
            ),
            Err(WarrantyError::InvertedWindow { .. })
        ));
        assert!(matches!(
            crate::bess_degradation::SohObservation::new(
                2.0,
                0,
                Provenance::Measured,
                "bms".to_string(),
            ),
            Err(DegradationLedgerError::ValueOutOfRange { .. })
        ));
    }
}
