// FIM-F12.01 + FIM-F12.02 (Wave 6, final implementation milestone) — the
//         Ed25519 signing/verification primitives for the secure-OTA
//         pipeline: the domain-separated signature domains, the trust
//         anchors, the release signing key (injected seed — NEVER
//         generated, NEVER shipped), and the REAL `UpdateVerifier` behind
//         the P9.34 port (`Ed25519UpdateSignatureVerifier`, Ed25519 over
//         trust anchors).
//
// IMPLEMENTED-UNDER-FIM-F12, WRITTEN-UNRUN: this module has never been
// compiled or run (the recorded loop bans; execution is the first human
// `cargo test`). No update bundle has ever been signed by it.
//
// DOMAIN SEPARATION (the Patch B discipline, `crates/flexcore-ports/src/
// source_auth.rs`, followed exactly): every signature is over bytes that
// START with a domain string, so a signature made for any other purpose
// can never verify here:
//
// - [`OTA_BUNDLE_SIGNATURE_DOMAIN`] (`flexcore-ota-update-v1`) prefixes the
//   bytes whose signature rides the update manifest (`bundle_digest_hex`
//   and the per-payload hex entries when the manifest declares the
//   `ed25519` algorithm). One domain serves the bundle level AND the
//   payload level because the verified object is always EXACTLY the
//   presented byte string: a payload signature could only verify as a
//   bundle signature if the payload slice IS the whole bundle, in which
//   case the manifest covers it either way. Telemetry envelopes
//   (`flexcore-telemetry-v1`), the update-evidence chain
//   (`flexcore-update-evidence-v1`), and the metadata roles below are all
//   disjoint domains.
// - [`OTA_METADATA_SIGNATURE_DOMAIN`] (`flexcore-ota-metadata-v1`) starts
//   each repository-metadata role's canonical bytes (root / targets /
//   timestamp / snapshot — see `crate::ota_bundle_produce`); the role code
//   byte inside the canonical bytes further separates the roles from each
//   other.
//
// THE TUF-SHAPED HONESTY BOUNDARY (recorded, never a conformance claim):
// the metadata roles are SHAPED BY the TUF role model (root / targets /
// timestamp / snapshot, [S-161] as summarized in the phase-18 records) —
// this is a FlexCore format, NOT a TUF encoding, and every
// TUF-conformance claim is UNPROVEN and UNMADE. The role semantics are
// UNVERIFIED against the TUF specification text.
//
// KEYS (the P18.03 honesty, verbatim): the signing key is constructed
// ONLY from an INJECTED 32-byte seed (deployment configuration). Nothing
// here generates key material at build time, embeds key material, or
// ships key material; the root-key and per-class targets-key ceremonies
// are HUMAN GATES (P18.03, record 03 §5 — NOT STARTED, no key material
// exists anywhere in this repository's history). The test seeds used by
// the written-unrun tests are fixed, loudly-labelled TEST-GRADE material
// and are never a release signature.
//
// THE VERIFIER (FIM-F12.02): [`Ed25519UpdateSignatureVerifier`] implements
// the P9.34 [`crate::update_verification::UpdateVerifier`] port with
// algorithm name `ed25519` (added to the closed
// `UPDATE_DIGEST_ALGORITHMS` vocabulary ADDITIVELY — the existing `sha-256`
// and `fnv1a64-test` entries are untouched). It is an injected OPTION like
// `crate::update_session::Sha256UpdateVerifier`: the shipped default
// remains [`crate::update_verification::NoUpdateVerifier`] (fail-closed),
// and a verifier composed with ZERO anchors refuses EVERYTHING (the
// absence of trust anchors can never look like success).

use ed25519_dalek::{Signer, Signature, SigningKey, Verifier, VerifyingKey};

use crate::update_verification::{
    UpdateDigestVerdict, UpdateVerifier, UPDATE_DIGEST_ALGORITHMS,
};

// --- Signature domains -----------------------------------------------------------------

/// The domain string prefixing every signed update-bundle byte string (the
/// manifest's bundle signature and the per-payload signatures under the
/// `ed25519` algorithm). See the module header for the domain-separation
/// discipline.
pub const OTA_BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"flexcore-ota-update-v1";

/// The domain string that STARTS every repository-metadata role's
/// canonical bytes (root / targets / timestamp / snapshot). The role code
/// byte follows it inside the canonical bytes (see
/// `crate::ota_bundle_produce`).
pub const OTA_METADATA_SIGNATURE_DOMAIN: &[u8] = b"flexcore-ota-metadata-v1";

/// The algorithm name the [`Ed25519UpdateSignatureVerifier`] reports — the
/// name a manifest declares in `digest_algorithm` to be verified by it.
/// Present in the closed [`UPDATE_DIGEST_ALGORITHMS`] vocabulary (added
/// additively by FIM-F12).
pub const OTA_ED25519_ALGORITHM: &str = "ed25519";

/// The exact byte length of an Ed25519 signature and public key.
pub const OTA_ED25519_SIGNATURE_BYTES: usize = 64;
/// The exact byte length of an Ed25519 public key (a trust anchor's key).
pub const OTA_ED25519_PUBLIC_KEY_BYTES: usize = 32;

/// The OTA signing/verification honesty statement (pinned so tests and
/// reviewers can read what IS and IS NOT claimed).
pub const OTA_SIGNING_STATEMENT: &str = "the FIM-F12 OTA signing layer signs and verifies \
     with Ed25519 over domain-separated canonical bytes (the Patch B discipline); the \
     metadata roles are TUF-SHAPED, not TUF-conformant, and the role semantics are \
     UNVERIFIED against the TUF spec text. Signing keys arrive as INJECTED seeds — no \
     key material is generated at build time, embedded, or shipped, and the P18.03 key \
     ceremonies are HUMAN GATES. The shipped update-gate default remains NoUpdateVerifier \
     (fail-closed); the Ed25519 verifier is an injected option that refuses EVERYTHING \
     when no trust anchors are configured. No bundle has ever been signed; no \
     certification or security claim is made.";

// --- Errors -----------------------------------------------------------------------

/// Named errors of the OTA signing/verification primitives. Every failure
/// is named ([`OtaSigningError::name`]); nothing is a silent pass. No
/// error ever carries key material; signer REFERENCE values are never
/// echoed (the P9.34 discipline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OtaSigningError {
    /// An input was unusable (empty reference, wrong key length, …).
    InvalidInput { reason: String },
    /// A trust anchor's bytes are not a valid Ed25519 public key — refused
    /// at construction, never stored half-formed.
    InvalidAnchorKey { reason: String },
    /// A signature hex/string input was not exactly 64 bytes.
    SignatureMalformed { reason: String },
    /// The signature's signer reference matches NO configured trust
    /// anchor (the reference value is deliberately not echoed).
    UnknownSignerReference,
    /// The signature does not verify over the presented bytes under the
    /// referenced anchor's key.
    SignatureInvalid,
}

impl OtaSigningError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            OtaSigningError::InvalidInput { .. } => "OTA_SIGNING_INPUT_INVALID",
            OtaSigningError::InvalidAnchorKey { .. } => "OTA_SIGNING_ANCHOR_KEY_INVALID",
            OtaSigningError::SignatureMalformed { .. } => "OTA_SIGNING_SIGNATURE_MALFORMED",
            OtaSigningError::UnknownSignerReference => "OTA_SIGNING_SIGNER_UNKNOWN",
            OtaSigningError::SignatureInvalid => "OTA_SIGNING_SIGNATURE_INVALID",
        }
    }
}

impl std::fmt::Display for OtaSigningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OtaSigningError::InvalidInput { reason } => {
                write!(f, "the OTA signing input is invalid: {reason}")
            }
            OtaSigningError::InvalidAnchorKey { reason } => write!(
                f,
                "the trust-anchor bytes are not a valid Ed25519 public key — refused at \
                 construction, never stored half-formed: {reason}"
            ),
            OtaSigningError::SignatureMalformed { reason } => {
                write!(f, "the signature input is malformed: {reason}")
            }
            OtaSigningError::UnknownSignerReference => write!(
                f,
                "the signature's signer reference matches NO configured trust anchor — \
                 refused, never verified against an unconfigured key (the reference value \
                 is not echoed)"
            ),
            OtaSigningError::SignatureInvalid => {
                write!(f, "the Ed25519 signature does not verify over the presented bytes \
                           under the referenced anchor's key — refused")
            }
        }
    }
}

impl std::error::Error for OtaSigningError {}

// --- Trust anchors -----------------------------------------------------------------------

/// One trust anchor: a signer key REFERENCE (scheme-prefixed, the P9.34
/// `signer_key_reference` rule — never key material in the reference) plus
/// that signer's Ed25519 PUBLIC key (32 bytes). The anchors are the
/// device-side instantiation of the root role's trust-anchor set (record
/// 03 §1: the `UpdateGate` constructor's trusted signer references,
/// role-qualified, now with the matching public keys for signature
/// verification). Anchors arrive as DEPLOYMENT CONFIGURATION — the root
/// key ceremony that minted them is a HUMAN GATE (P18.03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaTrustAnchor {
    /// The signer key reference (e.g. `key-store:flexcore-release-targets`).
    pub signer_reference: String,
    /// The signer's Ed25519 public key (32 raw bytes).
    pub public_key: [u8; OTA_ED25519_PUBLIC_KEY_BYTES],
}

impl OtaTrustAnchor {
    /// Constructs and validates one anchor (fail-closed: a reference that
    /// is empty, scheme-less, or a key that is not a valid Ed25519 public
    /// key is refused — never stored half-formed).
    pub fn new(signer_reference: &str, public_key: [u8; OTA_ED25519_PUBLIC_KEY_BYTES])
    -> Result<Self, OtaSigningError> {
        let scheme_ok = crate::update_verification::UPDATE_KEY_REFERENCE_SCHEMES
            .iter()
            .any(|scheme| {
                signer_reference
                    .strip_prefix(*scheme)
                    .map(|rest| !rest.is_empty())
                    .unwrap_or(false)
            });
        if !scheme_ok {
            return Err(OtaSigningError::InvalidInput {
                reason: "the trust-anchor signer reference must use the key-store:, env:, or \
                         file: scheme and name a location (anchors carry key REFERENCES plus \
                         the anchor PUBLIC key; a scheme-less reference is refused as likely \
                         inline key material)"
                    .to_string(),
            });
        }
        if VerifyingKey::from_bytes(&public_key).is_err() {
            return Err(OtaSigningError::InvalidAnchorKey {
                reason: "the 32 anchor bytes are not a valid Ed25519 public point".to_string(),
            });
        }
        Ok(Self {
            signer_reference: signer_reference.to_string(),
            public_key,
        })
    }
}

// --- The release signing key (injected seed; never generated, never shipped) ---------

/// The OTA release SIGNING key: constructed ONLY from an injected 32-byte
/// seed (deployment configuration). NO key generation happens anywhere in
/// this module (`SigningKey::generate` is deliberately not used — the
/// `rand_core` feature stays a workspace-level concern, and key GENERATION
/// belongs to the P18.03 human key ceremonies, not to code). No key
/// material leaves this struct except the PUBLIC key.
pub struct OtaEd25519ReleaseKey {
    signing_key: SigningKey,
    signer_reference: String,
}

impl OtaEd25519ReleaseKey {
    /// Builds the release key from the INJECTED seed and its signer
    /// reference (the reference the produced manifests declare).
    pub fn from_injected_seed(
        seed: &[u8; OTA_ED25519_PUBLIC_KEY_BYTES],
        signer_reference: &str,
    ) -> Result<Self, OtaSigningError> {
        if signer_reference.trim().is_empty() {
            return Err(OtaSigningError::InvalidInput {
                reason: "the release key's signer reference is mandatory (a manifest must \
                         name the key that signed it)"
                    .to_string(),
            });
        }
        // The seed's validity as an Ed25519 scalar is checked by
        // constructing the verifying key below (an all-zero or clamped
        // degenerate seed still yields a usable key; the anchor-side
        // construction validates the PUBLIC half where it is consumed).
        let signing_key = SigningKey::from_bytes(seed);
        let public = signing_key.verifying_key();
        // The public half must itself be a valid anchor key (a seed whose
        // public key is degenerate is refused — fail-closed, never a key
        // that cannot be anchored).
        if VerifyingKey::from_bytes(&public.to_bytes()).is_err() {
            return Err(OtaSigningError::InvalidAnchorKey {
                reason: "the injected seed's public key is not a valid Ed25519 public \
                         point — refuse the key rather than produce unverifiable \
                         signatures"
                    .to_string(),
            });
        }
        Ok(Self {
            signing_key,
            signer_reference: signer_reference.to_string(),
        })
    }

    /// The signer reference the produced manifests declare.
    pub fn signer_reference(&self) -> &str {
        &self.signer_reference
    }

    /// The PUBLIC key bytes (the trust-anchor half; the private seed never
    /// leaves this struct).
    pub fn public_key_bytes(&self) -> [u8; OTA_ED25519_PUBLIC_KEY_BYTES] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Signs the EXACT presented bytes (the caller supplies the
    /// domain-prefixed byte string — `bundle_signing_bytes` for bundle /
    /// payload levels, or a metadata role's canonical bytes, which already
    /// start with the metadata domain). The signature is over nothing
    /// else: no hashing step of ours, no normalization, no padding.
    pub fn sign_exact(&self, signed_bytes: &[u8]) -> Signature {
        self.signing_key.sign(signed_bytes)
    }
}

/// Builds the exact byte string whose signature rides the manifest for
/// `bundle` (the bundle level) or a payload slice (the payload level):
/// [`OTA_BUNDLE_SIGNATURE_DOMAIN`] || bytes. The signer and the verifier
/// both go through THIS function — one construction, no drift.
pub fn bundle_signing_bytes(bundle: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(OTA_BUNDLE_SIGNATURE_DOMAIN.len() + bundle.len());
    out.extend_from_slice(OTA_BUNDLE_SIGNATURE_DOMAIN);
    out.extend_from_slice(bundle);
    out
}

/// Verifies one signature over the EXACT presented bytes under the anchor
/// NAMED by `signer_reference` (the anchor is looked up by exact reference
/// match; a signature naming an unconfigured signer is refused BEFORE any
/// cryptographic work — never verified against an unconfigured key).
pub fn verify_signature_under_anchor(
    signed_bytes: &[u8],
    signature: &Signature,
    signer_reference: &str,
    anchors: &[OtaTrustAnchor],
) -> Result<(), OtaSigningError> {
    let anchor = anchors
        .iter()
        .find(|anchor| anchor.signer_reference == signer_reference)
        .ok_or(OtaSigningError::UnknownSignerReference)?;
    let verifying_key = VerifyingKey::from_bytes(&anchor.public_key)
        .map_err(|error| OtaSigningError::InvalidAnchorKey {
            reason: format!("the configured anchor key failed to load: {error}"),
        })?;
    verifying_key
        .verify(signed_bytes, signature)
        .map_err(|_| OtaSigningError::SignatureInvalid)
}

// --- The REAL update verifier (FIM-F12.02) ----------------------------------------------

/// The REAL Ed25519 update verifier behind the P9.34
/// [`UpdateVerifier`] port (FIM-F12.02): the manifest declares
/// `digest_algorithm = "ed25519"` and carries the bundle's signature (and
/// each payload's signature) in the hex digest fields; this verifier
/// checks the signature over the domain-separated presented bytes against
/// the configured trust anchors.
///
/// TRUST MODEL (recorded): the verifier holds the DEVICE-CONFIGURED trust
/// anchors (deployment configuration — the root role's anchor set). A
/// signature is accepted when it verifies under ANY configured anchor
/// (single-signature any-of; the manifest's `signer_key_reference` is
/// independently checked by the P9.34 gate's step 4 against the gate's
/// trusted-REFERENCE set — two independent checks, the record 03 §2
/// defense-in-depth pattern). Multi-key thresholds are the designed-in
/// next step (record 03 §3), not implemented.
///
/// FAIL-CLOSED POSTURE (unchanged from P9.34): a verifier composed with
/// ZERO anchors refuses EVERYTHING (`UpdateDigestVerdict::Refused`); the
/// SHIPPED default remains `NoUpdateVerifier`. This struct is not composed
/// anywhere by default. WIRING it under reviewed key management is the
/// standing DEPLOYMENT REQUIREMENT; the P18.03 key ceremonies are HUMAN
/// GATES.
pub struct Ed25519UpdateSignatureVerifier {
    anchors: Vec<OtaTrustAnchor>,
}

impl Ed25519UpdateSignatureVerifier {
    /// Composes the verifier over the device-configured anchors. The
    /// anchors are validated at construction (an invalid anchor refuses
    /// the composition — fail-closed, never a half-formed trust set).
    pub fn new(anchors: Vec<OtaTrustAnchor>) -> Result<Self, OtaSigningError> {
        // Re-validate every anchor through the constructing constructor
        // (the anchors may have arrived from configuration parsing that
        // did not validate).
        for anchor in &anchors {
            OtaTrustAnchor::new(&anchor.signer_reference, anchor.public_key)?;
        }
        Ok(Self { anchors })
    }

    /// The configured anchor references (diagnostics; references only).
    pub fn anchor_references(&self) -> Vec<&str> {
        self.anchors.iter().map(|a| a.signer_reference.as_str()).collect()
    }
}

impl UpdateVerifier for Ed25519UpdateSignatureVerifier {
    fn algorithm_name(&self) -> &'static str {
        OTA_ED25519_ALGORITHM
    }

    fn verify(&self, bundle: &[u8], declared_digest_hex: &str) -> UpdateDigestVerdict {
        // Zero anchors: refuse EVERYTHING (the absence of trust anchors
        // can never look like success — the NoUpdateVerifier posture).
        if self.anchors.is_empty() {
            return UpdateDigestVerdict::Refused {
                reason: "no OTA trust anchors are configured; the Ed25519 update verifier \
                         refuses every bundle (fail-closed — compose anchors from the \
                         deployment's root-key ceremony output, a HUMAN GATE, P18.03)"
                    .to_string(),
            };
        }
        // The declared hex must be exactly one Ed25519 signature.
        let signature = match decode_signature_hex(declared_digest_hex) {
            Ok(signature) => signature,
            Err(error) => {
                return UpdateDigestVerdict::Refused {
                    reason: error.to_string(),
                }
            }
        };
        // Any-of: the signature must verify under at least one configured
        // anchor (see the trust-model note in the type doc).
        for anchor in &self.anchors {
            let verifying_key = match VerifyingKey::from_bytes(&anchor.public_key) {
                Ok(key) => key,
                Err(error) => {
                    return UpdateDigestVerdict::Refused {
                        reason: format!(
                            "a configured trust anchor failed to load (fail-closed): {error}"
                        ),
                    }
                }
            };
            if verifying_key.verify(&bundle_signing_bytes(bundle), &signature).is_ok() {
                return UpdateDigestVerdict::Matches;
            }
        }
        UpdateDigestVerdict::Mismatch
    }
}

/// Decodes a lowercase-hex string into exactly one Ed25519 signature
/// (64 bytes). Uppercase, wrong length, and non-hex input are all named
/// refusals — never a guess.
pub fn decode_signature_hex(hex: &str) -> Result<Signature, OtaSigningError> {
    if hex.len() != OTA_ED25519_SIGNATURE_BYTES * 2 {
        return Err(OtaSigningError::SignatureMalformed {
            reason: format!(
                "the signature hex is {} characters; an Ed25519 signature is exactly {} \
                 bytes = {} hex characters",
                hex.len(),
                OTA_ED25519_SIGNATURE_BYTES,
                OTA_ED25519_SIGNATURE_BYTES * 2,
            ),
        });
    }
    let mut bytes = [0u8; OTA_ED25519_SIGNATURE_BYTES];
    for index in 0..OTA_ED25519_SIGNATURE_BYTES {
        let high = hex_hex_nibble(hex.as_bytes()[index * 2])
            .ok_or(OtaSigningError::SignatureMalformed {
                reason: "the signature hex carries a non-hex character".to_string(),
            })?;
        let low = hex_hex_nibble(hex.as_bytes()[index * 2 + 1])
            .ok_or(OtaSigningError::SignatureMalformed {
                reason: "the signature hex carries a non-hex character".to_string(),
            })?;
        bytes[index] = (high << 4) | low;
    }
    // The in-repo ed25519-dalek 3 pattern (apps/worker
    // source_auth_pipeline.rs): `Signature::from_bytes(&[u8; 64])` —
    // infallible over the fixed-length array.
    Ok(Signature::from_bytes(&bytes))
}

fn hex_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Renders one signature as the lowercase hex the manifest's hex fields
/// carry (the producer's counterpart to [`decode_signature_hex`]).
pub fn signature_to_hex(signature: &Signature) -> String {
    let bytes = signature.to_bytes();
    let mut out = String::with_capacity(OTA_ED25519_SIGNATURE_BYTES * 2);
    for byte in bytes {
        out.push(char_from_nibble(byte >> 4));
        out.push(char_from_nibble(byte & 0x0f));
    }
    out
}

fn char_from_nibble(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + (nibble - 10)) as char,
    }
}

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_verification::UPDATE_KEY_REFERENCE_SCHEMES;

    /// The FIXED TEST-GRADE signing seed (loudly labelled; never a release
    /// key; the P18.03 ceremonies are HUMAN GATES). Deterministic: the
    /// tests are socket-free and randomness-free.
    const TEST_GRADE_SEED_A: [u8; 32] = [0xA5u8; 32];
    const TEST_GRADE_SEED_B: [u8; 32] = [0x5Au8; 32];
    const TEST_SIGNER_REFERENCE: &str = "key-store:flexcore-release-targets";

    #[test]
    fn the_algorithm_name_is_in_the_closed_vocabulary() {
        assert_eq!(OTA_ED25519_ALGORITHM, "ed25519");
        assert!(
            UPDATE_DIGEST_ALGORITHMS.contains(&OTA_ED25519_ALGORITHM),
            "the ed25519 verifier cannot verify any manifest unless the closed vocabulary \
             names the algorithm"
        );
        // The existing entries are preserved (additive change only).
        assert!(UPDATE_DIGEST_ALGORITHMS.contains(&"sha-256"));
        assert!(UPDATE_DIGEST_ALGORITHMS.contains(&"fnv1a64-test"));
    }

    #[test]
    fn a_produced_signature_verifies_under_its_own_anchor() {
        let key = OtaEd25519ReleaseKey::from_injected_seed(
            &TEST_GRADE_SEED_A,
            TEST_SIGNER_REFERENCE,
        )
        .expect("test-grade key constructs");
        let anchor = OtaTrustAnchor::new(TEST_SIGNER_REFERENCE, key.public_key_bytes())
            .expect("anchor constructs");
        let message = bundle_signing_bytes(b"image-bytes");
        let signature = key.sign_exact(&message);
        verify_signature_under_anchor(&message, &signature, TEST_SIGNER_REFERENCE, &[anchor])
            .expect("the signature verifies under its own anchor");
    }

    #[test]
    fn a_flipped_byte_breaks_the_signature() {
        let key = OtaEd25519ReleaseKey::from_injected_seed(
            &TEST_GRADE_SEED_A,
            TEST_SIGNER_REFERENCE,
        )
        .expect("key constructs");
        let anchor = OtaTrustAnchor::new(TEST_SIGNER_REFERENCE, key.public_key_bytes())
            .expect("anchor constructs");
        let mut image = b"image-bytes".to_vec();
        let signature = key.sign_exact(&bundle_signing_bytes(&image));
        image[0] ^= 0x01;
        assert!(matches!(
            verify_signature_under_anchor(
                &bundle_signing_bytes(&image),
                &signature,
                TEST_SIGNER_REFERENCE,
                &[anchor],
            ),
            Err(OtaSigningError::SignatureInvalid)
        ));
    }

    #[test]
    fn a_wrong_signer_is_refused_before_any_crypto() {
        let key_a = OtaEd25519ReleaseKey::from_injected_seed(
            &TEST_GRADE_SEED_A,
            "key-store:flexcore-release-targets",
        )
        .expect("key A constructs");
        let key_b = OtaEd25519ReleaseKey::from_injected_seed(
            &TEST_GRADE_SEED_B,
            "key-store:flexcore-release-other",
        )
        .expect("key B constructs");
        let anchor_a = OtaTrustAnchor::new(
            "key-store:flexcore-release-targets",
            key_a.public_key_bytes(),
        )
        .expect("anchor A constructs");
        let message = bundle_signing_bytes(b"image-bytes");
        // Signed by B, anchored on A: the named signer does not match the
        // configured anchor set.
        let signature_b = key_b.sign_exact(&message);
        assert!(matches!(
            verify_signature_under_anchor(&message, &signature_b, "key-store:flexcore-release-other", &[anchor_a]),
            Err(OtaSigningError::UnknownSignerReference)
        ));
        // Even with BOTH anchors configured, the signature still does not
        // verify under A (any-of never accepts a foreign key).
        let anchor_b = OtaTrustAnchor::new(
            "key-store:flexcore-release-other",
            key_b.public_key_bytes(),
        )
        .expect("anchor B constructs");
        assert!(matches!(
            verify_signature_under_anchor(&message, &signature_b, "key-store:flexcore-release-targets", &[anchor_a, anchor_b]),
            Err(OtaSigningError::SignatureInvalid)
        ));
    }

    #[test]
    fn the_verifier_is_the_real_update_verifier_port() {
        let key = OtaEd25519ReleaseKey::from_injected_seed(
            &TEST_GRADE_SEED_A,
            TEST_SIGNER_REFERENCE,
        )
        .expect("key constructs");
        let anchor = OtaTrustAnchor::new(TEST_SIGNER_REFERENCE, key.public_key_bytes())
            .expect("anchor constructs");
        let verifier = Ed25519UpdateSignatureVerifier::new(vec![anchor]).expect("composes");
        assert_eq!(verifier.algorithm_name(), "ed25519");
        let image = b"image-bytes".to_vec();
        let signature_hex = signature_to_hex(&key.sign_exact(&bundle_signing_bytes(&image)));
        assert_eq!(
            verifier.verify(&image, &signature_hex),
            UpdateDigestVerdict::Matches
        );
        // One flipped byte refuses.
        let mut tampered = image.clone();
        tampered[3] ^= 0x80;
        assert_eq!(
            verifier.verify(&tampered, &signature_hex),
            UpdateDigestVerdict::Mismatch
        );
        // A malformed signature hex refuses (never a guess).
        assert!(matches!(
            verifier.verify(&image, "00112233"),
            UpdateDigestVerdict::Refused { .. }
        ));
    }

    #[test]
    fn zero_anchors_refuse_everything_fail_closed() {
        let verifier = Ed25519UpdateSignatureVerifier::new(Vec::new()).expect("composes");
        assert!(matches!(
            verifier.verify(b"anything", "00"),
            UpdateDigestVerdict::Refused { .. }
        ));
    }

    #[test]
    fn anchor_validation_is_fail_closed() {
        // A scheme-less reference is refused (the P9.34 inline-key-material
        // rule).
        assert!(matches!(
            OtaTrustAnchor::new("no-scheme-here", [0x11u8; 32]),
            Err(OtaSigningError::InvalidInput { .. })
        ));
        // A degenerate public key is refused at construction.
        assert!(matches!(
            OtaTrustAnchor::new(TEST_SIGNER_REFERENCE, [0u8; 32]),
            Err(OtaSigningError::InvalidAnchorKey { .. })
        ));
        // The anchor schemes are the P9.34 closed set (cited, never forked).
        assert_eq!(UPDATE_KEY_REFERENCE_SCHEMES.len(), 3);
    }

    #[test]
    fn the_statement_pins_the_honesty_bounds() {
        assert!(OTA_SIGNING_STATEMENT.contains("TUF-SHAPED, not TUF-conformant"));
        assert!(OTA_SIGNING_STATEMENT.contains("HUMAN GATES"));
        assert!(OTA_SIGNING_STATEMENT.contains("NoUpdateVerifier"));
        assert!(OTA_SIGNING_STATEMENT.contains("No bundle has ever been signed"));
        // The domains are disjoint from every other signing purpose in the
        // workspace (the Patch B rule).
        assert_ne!(OTA_BUNDLE_SIGNATURE_DOMAIN, OTA_METADATA_SIGNATURE_DOMAIN);
        assert_ne!(
            OTA_BUNDLE_SIGNATURE_DOMAIN,
            flexcore_ports::source_auth::ENVELOPE_DOMAIN
        );
    }
}
