// FIM-F13.01 — ISO 15118 Plug & Charge V2G Root CA TRUST STORE (code
// surface): the anchor-record model, explicit add/remove management, and
// the fail-closed lookups every PnC operation consumes.
//
// HONESTY HEADER (FIM-F13.06; FIM standing decision 2,
// docs/releases/implementation_ledger/README.md):
//
// - **Implemented-under-FIM-F13, CODE SURFACE ONLY.** The owner authorized
//   the CODE SURFACE of feature F-13 (ledger_F13.md); NO PKI is stood up,
//   no certificate exists, no key material exists anywhere, and no anchor
//   ships in this repository. Interop with a real V2G PKI is UNPROVEN.
// - **The five record-41 §3 ungate conditions are NOT met and are NOT
//   claimed met** (a named PKI operator; trust-anchor distribution agreed;
//   an OCSP service at fail-closed reliability; the OCA-text verification
//   pass; D5 closed naming OCPP and the edition — record
//   docs/phases-11-20/phase-17-ev-fleet-charging/records/
//   41_p17_12_pnc_certificate_management_design.md §3). They are enforced
//   as fail-closed runtime/configuration gates in [`crate::
//   pnc_certificate_flows`]; every gate defaults to UNMET and the code
//   refuses while any is UNMET. Each condition is a HUMAN GATE.
// - **WRITTEN-UNRUN**: this module has never compiled or executed; the
//   first human build/test gates it (conventions §1.3; the FIM hard
//   boundary — no build, no test execution, no installation).
// - **Anchors are INJECTED CONFIGURATION, never bundled.** The store is
//   constructed EMPTY and stays empty until a deployment injects anchor
//   records through the explicit [`V2gRootCaTrustStore::add_anchor`]
//   calls; there is no bundled default, no embedded certificate, and no
//   constructor that could produce a non-empty store from constants.
// - **Material posture (the record-41 deviation, recorded honestly):**
//   record 41 §2 designed the trust store as an ANCHOR-LABEL-ONLY surface
//   (the P14.03 pattern). The FIM owner authorization (code surface for
//   F-13) requires the anchor CERTIFICATE BYTES so the OCSP path can
//   perform its pinned byte-equality-by-reference check
//   ([`crate::pnc_ocsp`]); this module therefore carries DER bytes of
//   INJECTED anchors. The deviation is additive to the record's design,
//   never a reinterpretation of it: issuance, custody, and the anchor
//   material itself remain deployment PKI operations (a HUMAN GATE); the
//   never-granted `secret.read` permission set is untouched; NO KEY
//   MATERIAL exists anywhere (an anchor is a public certificate, and the
//   test fixtures are hand-built non-certificates pinned by tests).
// - **Fail-closed everywhere.** A store with no anchors refuses every
//   operation that needs one with the named [`PncTrustStoreError`];
//   nothing defaults to trusted, nothing is guessed.

use std::collections::BTreeMap;

/// The sanity upper bound for one injected anchor certificate (a gate
/// against unbounded configuration input, NOT a protocol or X.509 size
/// claim — the real bound is a deployment PKI-operations fact).
pub const MAX_ANCHOR_CERTIFICATE_BYTES: usize = 8_192;

/// One V2G trust-anchor record (FIM-F13.01). The certificate DER bytes are
/// INJECTED CONFIGURATION supplied by the deployment's PKI operations; this
/// module never bundles, generates, fetches, or derives one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustAnchorRecord {
    /// The anchor label (the deployment's stable handle; the label the
    /// certificate-flow records cite).
    pub anchor_label: String,
    /// The anchor certificate DER bytes (injected; a PUBLIC certificate —
    /// no key material exists anywhere in this crate).
    pub certificate_der: Vec<u8>,
    /// The signer reference: the PKI-operator/contract reference that
    /// vouches for this anchor (mandatory non-empty — an anchor nobody
    /// vouches for is refused, never defaulted).
    pub signer_reference: String,
    /// The anchor's not-before instant (UTC unix millis; declared data from
    /// the injection, checked fail-closed at lookup time).
    pub not_before_millis: i64,
    /// The anchor's not-after instant (UTC unix millis).
    pub not_after_millis: i64,
    /// When the anchor was added (the injected clock; recorded data).
    pub added_at_millis: i64,
}

impl TrustAnchorRecord {
    /// Validates one anchor record: non-empty label, signer reference, and
    /// certificate bytes; the certificate within the sanity bound; and a
    /// well-ordered validity window. Fail-closed.
    pub fn new(
        anchor_label: String,
        certificate_der: Vec<u8>,
        signer_reference: String,
        not_before_millis: i64,
        not_after_millis: i64,
        added_at_millis: i64,
    ) -> Result<Self, PncTrustStoreError> {
        if anchor_label.trim().is_empty() {
            return Err(PncTrustStoreError::EmptyField("anchor_label"));
        }
        if signer_reference.trim().is_empty() {
            return Err(PncTrustStoreError::EmptyField("signer_reference"));
        }
        if certificate_der.is_empty() {
            return Err(PncTrustStoreError::EmptyField("certificate_der"));
        }
        if certificate_der.len() > MAX_ANCHOR_CERTIFICATE_BYTES {
            return Err(PncTrustStoreError::CertificateTooLarge {
                len: certificate_der.len(),
                max: MAX_ANCHOR_CERTIFICATE_BYTES,
            });
        }
        if not_before_millis > not_after_millis {
            return Err(PncTrustStoreError::InvalidValidityWindow {
                label: anchor_label,
            });
        }
        Ok(Self {
            anchor_label,
            certificate_der,
            signer_reference,
            not_before_millis,
            not_after_millis,
            added_at_millis,
        })
    }

    /// Whether the anchor's declared validity window covers `now_millis`
    /// (fail-closed: an anchor outside its window is not a trust point).
    pub fn is_valid_at(&self, now_millis: i64) -> bool {
        self.not_before_millis <= now_millis && now_millis <= self.not_after_millis
    }
}

/// The V2G Root CA trust store (FIM-F13.01). Deterministic `BTreeMap`
/// keyed by anchor label. **Constructed EMPTY** — anchors arrive only
/// through explicit [`V2gRootCaTrustStore::add_anchor`] calls (injected
/// configuration, never bundled).
#[derive(Debug, Default)]
pub struct V2gRootCaTrustStore {
    anchors: BTreeMap<String, TrustAnchorRecord>,
}

impl V2gRootCaTrustStore {
    /// An empty store (the ONLY construction path: no anchors are ever
    /// bundled, generated, or defaulted).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Adds one anchor through the explicit management call. A duplicate
    /// label is REFUSED (trust material is never silently overwritten —
    /// replacing an anchor is an explicit remove-then-add by the
    /// deployment's PKI operations).
    pub fn add_anchor(&mut self, record: TrustAnchorRecord) -> Result<(), PncTrustStoreError> {
        if self.anchors.contains_key(&record.anchor_label) {
            return Err(PncTrustStoreError::DuplicateAnchor {
                label: record.anchor_label,
            });
        }
        self.anchors.insert(record.anchor_label.clone(), record);
        Ok(())
    }

    /// Removes one anchor through the explicit management call. Removing
    /// an absent anchor is REFUSED (a remove that did nothing is a
    /// configuration error, never a silent success).
    pub fn remove_anchor(&mut self, anchor_label: &str) -> Result<(), PncTrustStoreError> {
        if self.anchors.remove(anchor_label).is_none() {
            return Err(PncTrustStoreError::AnchorMissing {
                label: anchor_label.to_string(),
            });
        }
        Ok(())
    }

    /// The named anchor, when present.
    pub fn anchor(&self, anchor_label: &str) -> Option<&TrustAnchorRecord> {
        self.anchors.get(anchor_label)
    }

    /// Fail-closed named-anchor lookup: every operation that needs a
    /// specific anchor refuses with the named
    /// [`PncTrustStoreError::AnchorMissing`] when the store holds none (or
    /// not that one).
    pub fn require_anchor(
        &self,
        anchor_label: &str,
        now_millis: i64,
    ) -> Result<&TrustAnchorRecord, PncTrustStoreError> {
        let anchor = self.anchors.get(anchor_label).ok_or_else(|| {
            if self.anchors.is_empty() {
                PncTrustStoreError::StoreEmpty
            } else {
                PncTrustStoreError::AnchorMissing {
                    label: anchor_label.to_string(),
                }
            }
        })?;
        if !anchor.is_valid_at(now_millis) {
            return Err(PncTrustStoreError::AnchorOutsideValidityWindow {
                label: anchor.anchor_label.clone(),
            });
        }
        Ok(anchor)
    }

    /// Fail-closed "at least one anchor is configured" gate for
    /// operations that need the V2G hierarchy to exist at all (an empty
    /// store refuses with the named [`PncTrustStoreError::StoreEmpty`]).
    pub fn require_any_anchor(&self) -> Result<(), PncTrustStoreError> {
        if self.anchors.is_empty() {
            Err(PncTrustStoreError::StoreEmpty)
        } else {
            Ok(())
        }
    }

    /// Byte-equality lookup by certificate DER (the pinned-subset check the
    /// OCSP path and the InstallCertificate handler consume: a certificate
    /// is trusted ONLY when its DER bytes byte-equal an injected anchor's
    /// bytes, and that anchor is inside its validity window at `now`).
    /// NO X.509 parsing and NO chain building happens here — that is the
    /// recorded limitation of this code surface (UNPROVEN PKI interop).
    pub fn find_anchor_by_certificate(
        &self,
        certificate_der: &[u8],
        now_millis: i64,
    ) -> Result<&TrustAnchorRecord, PncTrustStoreError> {
        if certificate_der.is_empty() {
            return Err(PncTrustStoreError::EmptyField("certificate_der"));
        }
        self.require_any_anchor()?;
        for anchor in self.anchors.values() {
            if anchor.certificate_der.as_slice() == certificate_der {
                if !anchor.is_valid_at(now_millis) {
                    return Err(PncTrustStoreError::AnchorOutsideValidityWindow {
                        label: anchor.anchor_label.clone(),
                    });
                }
                return Ok(anchor);
            }
        }
        Err(PncTrustStoreError::CertificateNotAnchored)
    }

    /// How many anchors the store holds (diagnostics; zero is the honest
    /// shipped state).
    pub fn anchor_count(&self) -> usize {
        self.anchors.len()
    }
}

/// The named trust-store errors. Every refusal names the rule; nothing
/// implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PncTrustStoreError {
    /// A record field is empty. Silence is never a trust anchor.
    EmptyField(&'static str),
    /// The injected certificate exceeds the sanity bound.
    CertificateTooLarge { len: usize, max: usize },
    /// The anchor's validity window is not well-ordered.
    InvalidValidityWindow { label: String },
    /// An anchor with this label is already present (never silently
    /// overwritten).
    DuplicateAnchor { label: String },
    /// The named anchor is absent.
    AnchorMissing { label: String },
    /// The store holds NO anchors (the honest shipped state; every
    /// operation that needs one refuses by name).
    StoreEmpty,
    /// The anchor exists but is outside its declared validity window at
    /// the requested instant.
    AnchorOutsideValidityWindow { label: String },
    /// The certificate's bytes do not byte-equal any injected anchor.
    CertificateNotAnchored,
}

impl PncTrustStoreError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            PncTrustStoreError::EmptyField(_) => "PNC_TRUST_ANCHOR_FIELD_EMPTY",
            PncTrustStoreError::CertificateTooLarge { .. } => "PNC_TRUST_ANCHOR_TOO_LARGE",
            PncTrustStoreError::InvalidValidityWindow { .. } => {
                "PNC_TRUST_ANCHOR_VALIDITY_INVALID"
            }
            PncTrustStoreError::DuplicateAnchor { .. } => "PNC_TRUST_ANCHOR_DUPLICATE",
            PncTrustStoreError::AnchorMissing { .. } => "PNC_TRUST_ANCHOR_MISSING",
            PncTrustStoreError::StoreEmpty => "PNC_TRUST_STORE_EMPTY",
            PncTrustStoreError::AnchorOutsideValidityWindow { .. } => {
                "PNC_TRUST_ANCHOR_OUTSIDE_VALIDITY"
            }
            PncTrustStoreError::CertificateNotAnchored => "PNC_TRUST_CERTIFICATE_NOT_ANCHORED",
        }
    }
}

impl std::fmt::Display for PncTrustStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PncTrustStoreError::EmptyField(field) => write!(f, "field '{field}' is empty"),
            PncTrustStoreError::CertificateTooLarge { len, max } => {
                write!(f, "the anchor certificate is {len} bytes; the {max}-byte sanity bound is exceeded")
            }
            PncTrustStoreError::InvalidValidityWindow { label } => {
                write!(f, "anchor '{label}' declares a not-after before its not-before")
            }
            PncTrustStoreError::DuplicateAnchor { label } => {
                write!(f, "anchor '{label}' is already present; trust material is never \
                          silently overwritten (explicit remove-then-add is the PKI \
                          operations' change path)")
            }
            PncTrustStoreError::AnchorMissing { label } => {
                write!(f, "no anchor '{label}' is configured in the trust store")
            }
            PncTrustStoreError::StoreEmpty => write!(
                f,
                "the V2G trust store holds NO anchors — anchors are injected configuration, \
                 never bundled, and every operation that needs one refuses (fail-closed)"
            ),
            PncTrustStoreError::AnchorOutsideValidityWindow { label } => {
                write!(f, "anchor '{label}' is outside its declared validity window at the \
                          requested instant")
            }
            PncTrustStoreError::CertificateNotAnchored => write!(
                f,
                "the certificate's bytes do not byte-equal any injected trust anchor — \
                 refused, never guessed trusted"
            ),
        }
    }
}

impl std::error::Error for PncTrustStoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: i64 = 1_700_000_000_000;

    fn anchor(label: &str, der: Vec<u8>) -> TrustAnchorRecord {
        TrustAnchorRecord::new(
            label.to_string(),
            der,
            "pki-operator-reference-1".to_string(),
            BASE - 1_000,
            BASE + 100_000,
            BASE,
        )
        .expect("valid anchor")
    }

    #[test]
    fn the_store_ships_empty_and_refuses_every_anchor_operation() {
        let store = V2gRootCaTrustStore::empty();
        assert_eq!(store.anchor_count(), 0, "no anchor is ever bundled");
        // Every operation that needs an anchor refuses with the named error.
        assert_eq!(store.require_any_anchor().unwrap_err().name(), "PNC_TRUST_STORE_EMPTY");
        assert_eq!(
            store
                .require_anchor("v2g-root-1", BASE)
                .unwrap_err()
                .name(),
            "PNC_TRUST_STORE_EMPTY"
        );
        assert_eq!(
            store
                .find_anchor_by_certificate(&[0x30, 0x03, 0x02, 0x01, 0x07], BASE)
                .unwrap_err()
                .name(),
            "PNC_TRUST_STORE_EMPTY"
        );
    }

    #[test]
    fn anchors_are_added_and_removed_only_through_explicit_calls() {
        let mut store = V2gRootCaTrustStore::empty();
        store
            .add_anchor(anchor("v2g-root-1", vec![0x30, 0x03, 0x02, 0x01, 0x07]))
            .expect("added");
        assert_eq!(store.anchor_count(), 1);
        // A duplicate label is refused, never overwritten.
        assert_eq!(
            store
                .add_anchor(anchor("v2g-root-1", vec![0x30, 0x03, 0x02, 0x01, 0x08]))
                .unwrap_err()
                .name(),
            "PNC_TRUST_ANCHOR_DUPLICATE"
        );
        assert_eq!(store.anchor_count(), 1);
        // Byte-equality lookup finds the anchor inside its window.
        let found = store
            .find_anchor_by_certificate(&[0x30, 0x03, 0x02, 0x01, 0x07], BASE)
            .expect("byte-equal anchor found");
        assert_eq!(found.anchor_label, "v2g-root-1");
        // A different certificate is not anchored.
        assert_eq!(
            store
                .find_anchor_by_certificate(&[0x30, 0x03, 0x02, 0x01, 0x09], BASE)
                .unwrap_err()
                .name(),
            "PNC_TRUST_CERTIFICATE_NOT_ANCHORED"
        );
        // Explicit remove; removing an absent anchor is refused.
        store.remove_anchor("v2g-root-1").expect("removed");
        assert_eq!(store.anchor_count(), 0);
        assert_eq!(
            store.remove_anchor("v2g-root-1").unwrap_err().name(),
            "PNC_TRUST_ANCHOR_MISSING"
        );
    }

    #[test]
    fn expired_anchors_are_fail_closed_and_bad_records_are_refused() {
        let mut store = V2gRootCaTrustStore::empty();
        store
            .add_anchor(anchor("v2g-root-1", vec![0x30, 0x03, 0x02, 0x01, 0x07]))
            .expect("added");
        // Outside the validity window: refuse by name, never trust.
        assert_eq!(
            store
                .find_anchor_by_certificate(&[0x30, 0x03, 0x02, 0x01, 0x07], BASE + 200_000)
                .unwrap_err()
                .name(),
            "PNC_TRUST_ANCHOR_OUTSIDE_VALIDITY"
        );
        assert_eq!(
            store
                .require_anchor("v2g-root-1", BASE - 2_000)
                .unwrap_err()
                .name(),
            "PNC_TRUST_ANCHOR_OUTSIDE_VALIDITY"
        );
        // Record-level refusals.
        assert!(TrustAnchorRecord::new(
            "  ".to_string(),
            vec![1],
            "ref".to_string(),
            0,
            1,
            1
        )
        .is_err());
        assert!(TrustAnchorRecord::new(
            "l".to_string(),
            Vec::new(),
            "ref".to_string(),
            0,
            1,
            1
        )
        .is_err());
        assert!(TrustAnchorRecord::new(
            "l".to_string(),
            vec![1],
            "  ".to_string(),
            0,
            1,
            1
        )
        .is_err());
        assert_eq!(
            TrustAnchorRecord::new("l".to_string(), vec![1], "ref".to_string(), 10, 5, 1)
                .unwrap_err()
                .name(),
            "PNC_TRUST_ANCHOR_VALIDITY_INVALID"
        );
        let oversized = TrustAnchorRecord::new(
            "l".to_string(),
            vec![0u8; MAX_ANCHOR_CERTIFICATE_BYTES + 1],
            "ref".to_string(),
            0,
            1,
            1,
        );
        assert_eq!(
            oversized.unwrap_err().name(),
            "PNC_TRUST_ANCHOR_TOO_LARGE"
        );
    }
}
