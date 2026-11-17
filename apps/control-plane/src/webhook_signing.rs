// P13.09 — Outbound webhook SIGNING: Stripe-grade signature scheme over the
//          RAW request body (plan §2.3), 5-minute tolerance window, secret
//          rolling with an overlap window.
//
// Phase 13 wave 1, Agent B. WRITTEN-UNRUN: this module has never been
// compiled or executed (the first human build/test is the gate; program
// conventions §1). No webhook is signed, delivered, or verified by this
// batch — the module is the signing/verification HALF of the delivery
// service (`webhook_service.rs`, same phase), not a delivery path.
//
// Contract (plan §2.3, webhook delivery contract — binding):
//
// | Element    | This module |
// |---|---|
// | Signature  | `HMAC-SHA256(secret, timestamp + "." + rawbody)` — signed over the RAW body bytes exactly as they will be transmitted, never a re-serialized form [S-83] |
// | Header     | `FlexCore-Signature: t=<unix_seconds>,v1=<hex digest>` (the Stripe scheme shape [S-83]: a timestamp element plus one or more signature elements; OUR scheme is our own — we do not claim Stripe wire compatibility) |
// | Tolerance  | receivers reject deliveries whose timestamp differs from the verifier's clock by more than [`TOLERANCE_SECONDS`] (5 minutes) — the replay window |
// | Secret rolling | the new secret is issued alongside the old with an overlap window during which BOTH verify; the old secret is revoked only after the overlap closes ([`WebhookSecretSet::rolling`]) |
//
// SECRET FORMAT OWNERSHIP (the P13.08 → P13.09 seam, plan §2.1) —
// RESOLVED: the format is defined ONCE by Agent A's record
// `08_p13_08_api_keys_webhook_secret_format.md` §4 —
// `whsec_<43 unpadded base64url characters>`, exactly 32 bytes of CSPRNG
// entropy, strict parse rule. This module consumes the record BY CITATION
// for the FORMAT DEFINITION (never restated as a second definition) and,
// since FRV1-R8-004 (final review R8), implements the record's STRICT
// PARSE RULE as the pure function [`parse_webhook_secret`] beside
// [`WebhookSigningKey`] — code, not prose, for the composition step to
// consume (the resolver composition binds the reference→material
// RESOLUTION to the P11.16 secrets posture — Vault DEFAULT, Compose
// file-per-host floor, `docs/phases-11-20/phase-11-operational-robustness/
// records/32_p11_16_secrets_management.md`; `webhook_service::
// WebhookSecretResolver`). The material never lives in the database
// (record 08 §4's storage rule; migration 0020 stores references +
// fingerprints only).
//
// Crypto provenance (the P11.15 precedent, recorded in
// `docs/phases-11-20/phase-11-operational-robustness/records/
// 31_p11_15_audit_chain_upgrade.md` §3): the program bans new dependency
// additions, so SHA-256 (FIPS 180-4) and HMAC-SHA-256 (RFC 2104) are
// hand-written here, std-only, and anchored to the published vectors
// (FIPS 180-4 "abc"/empty; RFC 4231 test cases 1, 2, 7) transcribed into
// the unit tests — the SAME published vectors the P11.15 chain module
// anchors. Correctness is verified ONLY at the first human test run; a
// vector mismatch is a build-blocking finding. An independent review of
// this module's crypto before any production secret is used is part of the
// DEPLOYMENT REQUIREMENT gate (program conventions §6.6): test vectors
// demonstrate algorithm conformance of a hand-written implementation, they
// are NOT a security review, and no security or conformance claim is made.
//
// Duplication note (recorded, same rationale as P11.15 §3 item 2): the
// HMAC machinery in `crates/flexcore-audit/src/chain.rs` is `pub(crate)`
// and cross-crate reuse would require both a visibility change and a new
// workspace edge (apps/control-plane → flexcore-audit is checker-legal but
// a manifest edit this batch forbids). Consolidation of the two hand-written
// copies is a recorded follow-up for a wave that owns both manifests and the
// visibility surface. Until then the two copies stay independently
// vector-anchored.

/// The signature-scheme constants (closed, documented here once).
pub mod scheme {
    /// The signature header name.
    pub const SIGNATURE_HEADER: &str = "FlexCore-Signature";
    /// The timestamp element prefix inside the header value.
    pub const TIMESTAMP_ELEMENT: &str = "t";
    /// The signature element prefix inside the header value.
    pub const SIGNATURE_ELEMENT: &str = "v1";
    /// The byte joining timestamp and body inside the signed message
    /// (`timestamp + "." + rawbody`, plan §2.3).
    pub const TIMESTAMP_BODY_SEPARATOR: u8 = b'.';
    /// The element separator inside the header value (`,`).
    pub const ELEMENT_SEPARATOR: char = ',';
    /// The element key/value separator inside the header value (`=`).
    pub const KEY_VALUE_SEPARATOR: char = '=';
}

/// The replay-window tolerance in seconds: receivers reject deliveries
/// whose header timestamp differs from the verification clock by more than
/// this (plan §2.3: 5 minutes [S-83]).
///
/// REGISTERED DEFAULT (X-8 discipline: written down before any run that
/// could depend on it). Changing the value is a recorded amendment to the
/// P13.09 decision record AND a coordinated change on every receiver.
pub const TOLERANCE_SECONDS: i64 = 300;

/// Minimum secret material length in bytes. Secrets below this length are
/// rejected at construction (fail closed) — the same floor the P11.15 chain
/// key enforces (`CHAIN_KEY_MIN_BYTES = 32`, `crates/flexcore-audit/src/
/// chain.rs`). The FORMAT above this floor is P13.08's (the ownership note
/// in the module header).
pub const SECRET_MIN_BYTES: usize = 32;

/// Errors of webhook signing and verification. Closed vocabulary; every
/// variant names the failure, never the secret material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookSigningError {
    /// The secret material was shorter than [`SECRET_MIN_BYTES`].
    SecretTooShort {
        /// The supplied length in bytes (never the material itself).
        length: usize,
    },
    /// The signature header was absent.
    HeaderMissing,
    /// The signature header was present but malformed (`reason` names the
    /// structural problem; the header VALUE is never echoed — it is
    /// attacker-controlled input).
    HeaderMalformed {
        reason: &'static str,
    },
    /// The header timestamp was outside [`TOLERANCE_SECONDS`] of the
    /// verification clock (`now_unix_seconds`). The offending timestamps
    /// are reported so the receiving operator can diagnose clock skew; the
    /// signature itself is never echoed.
    OutsideTolerance {
        header_timestamp: i64,
        now_unix_seconds: i64,
    },
    /// The signature did not verify against ANY active secret.
    SignatureMismatch,
}

impl std::fmt::Display for WebhookSigningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookSigningError::SecretTooShort { length } => write!(
                f,
                "webhook signing secret is {length} bytes; at least {SECRET_MIN_BYTES} required"
            ),
            WebhookSigningError::HeaderMissing => {
                write!(f, "the {} header is missing", scheme::SIGNATURE_HEADER)
            }
            WebhookSigningError::HeaderMalformed { reason } => {
                write!(f, "the {} header is malformed: {reason}", scheme::SIGNATURE_HEADER)
            }
            WebhookSigningError::OutsideTolerance {
                header_timestamp,
                now_unix_seconds,
            } => write!(
                f,
                "delivery timestamp {header_timestamp} is outside the {TOLERANCE_SECONDS}s \
                 tolerance of {now_unix_seconds} (replay window)"
            ),
            WebhookSigningError::SignatureMismatch => {
                write!(f, "the signature did not verify against any active secret")
            }
        }
    }
}

impl std::error::Error for WebhookSigningError {}

/// std-only SHA-256 (FIPS 180-4). Hand-written per the recorded P11.15
/// precedent (module header): no new dependency is added, and the
/// implementation is anchored to the standard's published vectors in the
/// tests. `pub` only because the sibling delivery module signs through it
/// and the in-crate tests pin the vectors.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    // The FIPS 180-4 initial hash value (the first 32 bits of the
    // fractional parts of the square roots of the first 8 primes).
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    // The K constants (the first 32 bits of the fractional parts of the
    // cube roots of the first 64 primes).
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let bit_len = (data.len() as u64).saturating_mul(8);
    let mut message = data.to_vec();
    // Padding: append 0x80, then zeros until length ≡ 56 (mod 64), then the
    // 8-byte big-endian bit length.
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut w = [0u32; 64];
    for block in message.chunks_exact(64) {
        for (i, word) in w.iter_mut().enumerate().take(16) {
            let b = &block[i * 4..i * 4 + 4];
            *word = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut digest = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

/// std-only HMAC-SHA-256 (RFC 2104). Hand-written per the recorded P11.15
/// precedent; anchored to RFC 4231 test cases 1, 2, 7 in the tests.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    // RFC 2104 §2: a key longer than the block size is replaced by its
    // hash; shorter keys are zero-padded to the block size.
    let key_block: [u8; BLOCK] = {
        let mut block = [0u8; BLOCK];
        if key.len() > BLOCK {
            block[..32].copy_from_slice(&sha256(key));
        } else {
            block[..key.len()].copy_from_slice(key);
        }
        block
    };
    let mut inner_pad = [0x36u8; BLOCK];
    let mut outer_pad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        inner_pad[i] ^= key_block[i];
        outer_pad[i] ^= key_block[i];
    }
    let mut inner_input = Vec::with_capacity(BLOCK + message.len());
    inner_input.extend_from_slice(&inner_pad);
    inner_input.extend_from_slice(message);
    let inner = sha256(&inner_input);
    let mut outer_input = Vec::with_capacity(BLOCK + 32);
    outer_input.extend_from_slice(&outer_pad);
    outer_input.extend_from_slice(&inner);
    sha256(&outer_input)
}

/// Lowercase hex rendering of a 32-byte digest (the `v1` element encoding).
pub fn hex_encode_digest(digest: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("high nibble"));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).expect("low nibble"));
    }
    out
}

/// Constant-time equality for digest comparison: always scans the full
/// length, accumulating the difference, so early-exit timing cannot leak
/// how many prefix bytes matched.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// One webhook signing secret as OPAQUE BYTES.
///
/// Sealed construction ([`WebhookSigningKey::from_bytes`] is the only way
/// in): the material is length-checked at construction, the type carries no
/// `Debug`/`Display` impl (the material can never be logged by accident),
/// and drop best-effort zeroizes the buffer (best-effort: the compiler may
/// copy the buffer; zeroization is hygiene, not a guarantee — the same
/// honesty the P11.15 `ChainKey` records).
pub struct WebhookSigningKey {
    material: Vec<u8>,
}

impl WebhookSigningKey {
    /// Wraps secret material (opaque bytes; the TEXT format is P13.08's).
    /// Fails closed below [`SECRET_MIN_BYTES`].
    pub fn from_bytes(material: &[u8]) -> Result<Self, WebhookSigningError> {
        if material.len() < SECRET_MIN_BYTES {
            return Err(WebhookSigningError::SecretTooShort { length: material.len() });
        }
        Ok(Self { material: material.to_vec() })
    }

    /// The material as bytes (for HMAC input; never for logging).
    pub fn as_bytes(&self) -> &[u8] {
        &self.material
    }
}

impl Drop for WebhookSigningKey {
    fn drop(&mut self) {
        for byte in self.material.iter_mut() {
            *byte = 0;
        }
    }
}

/// The literal `whsec_` secret prefix (record 08 §4: EXACT — case
/// sensitive, no alternate spelling accepted).
pub const WEBHOOK_SECRET_PREFIX: &str = "whsec_";
/// The base64url character count after the prefix (record 08 §4: exactly
/// 43; 32 bytes encode to 43 unpadded base64url characters).
pub const WEBHOOK_SECRET_BASE64URL_CHARS: usize = 43;
/// The total literal length (prefix + payload): record 08 §4's "total
/// length 49".
pub const WEBHOOK_SECRET_TEXT_LEN: usize = 49;
/// The decoded entropy length (record 08 §4: exactly 32 bytes — the same
/// value as [`SECRET_MIN_BYTES`], the [`WebhookSigningKey`] floor).
pub const WEBHOOK_SECRET_DECODED_BYTES: usize = 32;

/// Why a `whsec_` secret literal was refused (record 08 §4's strict parse
/// rule: "Anything else — wrong prefix, wrong length, padding characters,
/// non-alphabet characters, wrong decoded length — is a named parse
/// rejection, never a lenient accept", FRV1-R8-004). Closed vocabulary;
/// every refusal names its class and NEVER echoes the rejected value (it
/// may be live secret material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookSecretParseError {
    /// The literal did not start with the exact `whsec_` prefix.
    WrongPrefix,
    /// The part after the prefix was not exactly
    /// [`WEBHOOK_SECRET_BASE64URL_CHARS`] characters (the reported length
    /// is the payload length in characters, never the payload itself).
    WrongLength { payload_len: usize },
    /// The payload contained a base64 padding character (`=`) — the
    /// record's format is unpadded, and padding would also change the
    /// length class.
    PaddingCharacter,
    /// The payload contained a character outside the unpadded base64url
    /// alphabet (`A–Z a–z 0–9 - _`).
    NonAlphabetCharacter,
    /// The payload decoded to a byte length other than
    /// [`WEBHOOK_SECRET_DECODED_BYTES`]. Defense in depth: the
    /// exactly-43-character check already forces a 32-byte decode, so this
    /// class is unreachable through the current checks — it is kept as the
    /// record's named class so a future check reordering cannot turn the
    /// rule into a lenient accept.
    WrongDecodedLength { length: usize },
}

impl WebhookSecretParseError {
    /// The stable machine-readable refusal name (records, metrics).
    pub fn name(&self) -> &'static str {
        match self {
            WebhookSecretParseError::WrongPrefix => "WEBHOOK_SECRET_WRONG_PREFIX",
            WebhookSecretParseError::WrongLength { .. } => "WEBHOOK_SECRET_WRONG_LENGTH",
            WebhookSecretParseError::PaddingCharacter => "WEBHOOK_SECRET_PADDING_CHARACTER",
            WebhookSecretParseError::NonAlphabetCharacter => {
                "WEBHOOK_SECRET_NON_ALPHABET_CHARACTER"
            }
            WebhookSecretParseError::WrongDecodedLength { .. } => {
                "WEBHOOK_SECRET_WRONG_DECODED_LENGTH"
            }
        }
    }
}

impl std::fmt::Display for WebhookSecretParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookSecretParseError::WrongPrefix => write!(
                f,
                "webhook secret does not start with the exact '{WEBHOOK_SECRET_PREFIX}' prefix"
            ),
            WebhookSecretParseError::WrongLength { payload_len } => write!(
                f,
                "webhook secret payload is {payload_len} characters; exactly \
                 {WEBHOOK_SECRET_BASE64URL_CHARS} required (unpadded base64url)"
            ),
            WebhookSecretParseError::PaddingCharacter => write!(
                f,
                "webhook secret payload carries a base64 padding character; the record \
                 08 §4 format is unpadded"
            ),
            WebhookSecretParseError::NonAlphabetCharacter => write!(
                f,
                "webhook secret payload carries a character outside the unpadded base64url \
                 alphabet (A-Z a-z 0-9 - _)"
            ),
            WebhookSecretParseError::WrongDecodedLength { length } => write!(
                f,
                "webhook secret payload decodes to {length} bytes; exactly \
                 {WEBHOOK_SECRET_DECODED_BYTES} required"
            ),
        }
    }
}

impl std::error::Error for WebhookSecretParseError {}

/// The base64url value of one alphabet character, or `None` outside the
/// unpadded base64url alphabet (`A–Z a–z 0–9 - _`; record 08 §4).
fn base64url_value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some((byte - b'A') as u32),
        b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
        b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

/// The STRICT parse of a `whsec_` webhook signing secret (record 08 §4 §4
/// "Parsing rule", implemented as code per FRV1-R8-004 — the composition's
/// resolver consumes this instead of restating prose):
///
/// 1. the `whsec_` prefix is EXACT (case sensitive);
/// 2. the remainder is exactly [`WEBHOOK_SECRET_BASE64URL_CHARS`]
///    characters of the unpadded base64url alphabet (`A–Z a–z 0–9 - _`),
///    with padding characters (`=`) rejected as their own class;
/// 3. the remainder decodes to exactly [`WEBHOOK_SECRET_DECODED_BYTES`]
///    (32) bytes.
///
/// On success the decoded 32 bytes are wrapped as a [`WebhookSigningKey`]
/// (whose [`SECRET_MIN_BYTES`] floor equals the decoded length exactly).
/// Every failure is a named [`WebhookSecretParseError`] rejection — never
/// a lenient accept — and no error ever echoes the rejected value.
/// Recorded scope note: trailing-bit canonicality of the final 6-bit group
/// is NOT a rejection class in record 08 §4 (the rule is "decodes to
/// exactly 32 bytes", which a 43-alphabet-character payload always
/// satisfies); a canonical-encoding tightening would be a record
/// amendment, not a code-side silent addition.
pub fn parse_webhook_secret(
    text: &str,
) -> Result<WebhookSigningKey, WebhookSecretParseError> {
    let payload = text
        .strip_prefix(WEBHOOK_SECRET_PREFIX)
        .ok_or(WebhookSecretParseError::WrongPrefix)?;
    if payload.len() != WEBHOOK_SECRET_BASE64URL_CHARS {
        return Err(WebhookSecretParseError::WrongLength {
            payload_len: payload.len(),
        });
    }
    if payload.as_bytes().contains(&b'=') {
        return Err(WebhookSecretParseError::PaddingCharacter);
    }
    let mut decoded = [0u8; WEBHOOK_SECRET_DECODED_BYTES];
    let mut accumulator: u32 = 0;
    let mut bits: u32 = 0;
    let mut out_len: usize = 0;
    for byte in payload.as_bytes() {
        let value = base64url_value(*byte)
            .ok_or(WebhookSecretParseError::NonAlphabetCharacter)?;
        accumulator = (accumulator << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            if out_len >= WEBHOOK_SECRET_DECODED_BYTES {
                return Err(WebhookSecretParseError::WrongDecodedLength {
                    length: out_len + 1,
                });
            }
            decoded[out_len] = ((accumulator >> bits) & 0xFF) as u8;
            out_len += 1;
        }
    }
    if out_len != WEBHOOK_SECRET_DECODED_BYTES {
        return Err(WebhookSecretParseError::WrongDecodedLength { length: out_len });
    }
    WebhookSigningKey::from_bytes(&decoded)
        .map_err(|_| WebhookSecretParseError::WrongDecodedLength {
            length: decoded.len(),
        })
}

/// A rolling secret pair: the CURRENT signing secret and, during an
/// overlap window, the PREVIOUS secret which still VERIFIES.
///
/// Rolling semantics (plan §2.3, "secret rolling" row): the new secret is
/// issued alongside the old; deliveries are SIGNED with the current secret
/// only; verifiers accept a signature over EITHER secret for as long as
/// the previous is present; the old secret is revoked by REPLACING this set
/// with one whose `previous` is `None` — which happens only after the
/// overlap closes. The overlap DURATION is deployment configuration (a
/// receiver's processing-delay budget), never a constant in this module.
pub struct WebhookSecretSet {
    current: WebhookSigningKey,
    previous: Option<WebhookSigningKey>,
}

impl WebhookSecretSet {
    /// A single-secret set (no rolling overlap active).
    pub fn single(current: WebhookSigningKey) -> Self {
        Self { current, previous: None }
    }

    /// A rolling set: `current` signs, both `current` and `previous`
    /// verify until the previous is revoked.
    pub fn rolling(current: WebhookSigningKey, previous: WebhookSigningKey) -> Self {
        Self { current, previous: Some(previous) }
    }

    /// Signs `raw_body` at `timestamp_unix_seconds`: returns the header
    /// VALUE (`t=<ts>,v1=<hex>`) — the caller attaches it under
    /// [`scheme::SIGNATURE_HEADER`]. The message is
    /// `timestamp_ascii + "." + raw_body` over the RAW body bytes (plan
    /// §2.3): the caller MUST pass the exact byte sequence it transmits,
    /// never a re-serialized form.
    pub fn sign(&self, timestamp_unix_seconds: i64, raw_body: &[u8]) -> String {
        let digest = signature_digest(&self.current, timestamp_unix_seconds, raw_body);
        format!(
            "{}={},{}={}",
            scheme::TIMESTAMP_ELEMENT,
            timestamp_unix_seconds,
            scheme::SIGNATURE_ELEMENT,
            hex_encode_digest(&digest),
        )
    }

    /// Verifies a header value against this set at verification clock
    /// `now_unix_seconds` over the RAW body bytes. Order of checks: header
    /// structure, tolerance (fail fast on replay-window violations), then
    /// signature over current-then-previous (constant-time compares).
    pub fn verify(
        &self,
        header_value: &str,
        now_unix_seconds: i64,
        raw_body: &[u8],
    ) -> Result<(), WebhookSigningError> {
        let (header_timestamp, provided) = parse_signature_header(header_value)?;
        if (now_unix_seconds - header_timestamp).abs() > TOLERANCE_SECONDS {
            return Err(WebhookSigningError::OutsideTolerance {
                header_timestamp,
                now_unix_seconds,
            });
        }
        let provided_bytes = provided;
        if constant_time_eq(
            &signature_digest(&self.current, header_timestamp, raw_body),
            &provided_bytes,
        ) {
            return Ok(());
        }
        if let Some(previous) = &self.previous {
            if constant_time_eq(
                &signature_digest(previous, header_timestamp, raw_body),
                &provided_bytes,
            ) {
                return Ok(());
            }
        }
        Err(WebhookSigningError::SignatureMismatch)
    }
}

/// The HMAC over `timestamp + "." + rawbody` (plan §2.3).
fn signature_digest(
    key: &WebhookSigningKey,
    timestamp_unix_seconds: i64,
    raw_body: &[u8],
) -> [u8; 32] {
    let mut message = Vec::with_capacity(20 + 1 + raw_body.len());
    message.extend_from_slice(timestamp_unix_seconds.to_string().as_bytes());
    message.push(scheme::TIMESTAMP_BODY_SEPARATOR);
    message.extend_from_slice(raw_body);
    hmac_sha256(key.as_bytes(), &message)
}

/// Parses the signature header value into (timestamp, signature digest
/// bytes). Strict single-element parsing: exactly one `t` element and one
/// `v1` element, in either order, no duplicates, no unknown elements — a
/// receiver that accepted loose structure would widen the replay surface.
fn parse_signature_header(
    header_value: &str,
) -> Result<(i64, [u8; 32]), WebhookSigningError> {
    let mut timestamp: Option<i64> = None;
    let mut signature: Option<[u8; 32]> = None;
    for element in header_value.split(scheme::ELEMENT_SEPARATOR) {
        let (key, value) = element
            .split_once(scheme::KEY_VALUE_SEPARATOR)
            .ok_or(WebhookSigningError::HeaderMalformed {
                reason: "element without a key=value form",
            })?;
        match key.trim() {
            scheme::TIMESTAMP_ELEMENT => {
                if timestamp.is_some() {
                    return Err(WebhookSigningError::HeaderMalformed {
                        reason: "duplicate timestamp element",
                    });
                }
                timestamp = Some(value.trim().parse::<i64>().map_err(|_| {
                    WebhookSigningError::HeaderMalformed {
                        reason: "timestamp is not an integer",
                    }
                })?);
            }
            scheme::SIGNATURE_ELEMENT => {
                if signature.is_some() {
                    return Err(WebhookSigningError::HeaderMalformed {
                        reason: "duplicate signature element",
                    });
                }
                let hex = value.trim();
                if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(WebhookSigningError::HeaderMalformed {
                        reason: "signature is not 64 hex characters",
                    });
                }
                let mut bytes = [0u8; 32];
                for i in 0..32 {
                    let hi = hex.as_bytes()[i * 2];
                    let lo = hex.as_bytes()[i * 2 + 1];
                    bytes[i] = (hex_nibble(hi) << 4) | hex_nibble(lo);
                }
                signature = Some(bytes);
            }
            _ => {
                return Err(WebhookSigningError::HeaderMalformed {
                    reason: "unknown element",
                })
            }
        }
    }
    match (timestamp, signature) {
        (Some(t), Some(s)) => Ok((t, s)),
        (None, _) => Err(WebhookSigningError::HeaderMalformed {
            reason: "timestamp element missing",
        }),
        (_, None) => Err(WebhookSigningError::HeaderMalformed {
            reason: "signature element missing",
        }),
    }
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        // The caller pre-validates hex digits; this branch is unreachable
        // through `verify` and panics only on direct misuse.
        _ => panic!("non-hex digit reached hex_nibble"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- FIPS 180-4 SHA-256 vectors (the standard's published examples;
    //     the same published vectors the P11.15 chain module anchors) ---

    #[test]
    fn sha256_matches_the_fips_180_4_abc_vector() {
        let digest = sha256(b"abc");
        assert_eq!(
            hex_encode_digest(&digest),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_matches_the_fips_180_4_empty_vector() {
        let digest = sha256(b"");
        assert_eq!(
            hex_encode_digest(&digest),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // --- RFC 4231 HMAC-SHA-256 vectors (test cases 1, 2, 7; the same
    //     published vectors the P11.15 chain module anchors) -------------

    #[test]
    fn hmac_matches_rfc_4231_test_case_1() {
        let digest = hmac_sha256(&[0x0bu8; 20], b"Hi There");
        assert_eq!(
            hex_encode_digest(&digest),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn hmac_matches_rfc_4231_test_case_2() {
        let digest = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex_encode_digest(&digest),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn hmac_matches_rfc_4231_test_case_7_block_size_key_hashing() {
        // RFC 4231 test case 7: a 131-byte key (larger than the SHA-256
        // block size) — exercises the hash-the-key-first path.
        let key = [0xaau8; 131];
        let data = b"This is a test using a larger than block-size key and a \
                     larger than block-size data. The key needs to be hashed \
                     before being used by the HMAC algorithm.";
        let digest = hmac_sha256(&key, data);
        assert_eq!(
            hex_encode_digest(&digest),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"
        );
    }

    // --- The signing scheme ---------------------------------------------

    fn test_key(tag: u8) -> WebhookSigningKey {
        WebhookSigningKey::from_bytes(&[tag; 32]).expect("valid test key")
    }

    const RAW_BODY: &[u8] = b"{\"event\":\"ScheduleProposed\",\"schedule_id\":\"s-1\"}";
    const NOW: i64 = 1_700_000_000;

    #[test]
    fn a_signature_round_trips_through_verify() {
        let set = WebhookSecretSet::single(test_key(0x42));
        let header = set.sign(NOW, RAW_BODY);
        set.verify(&header, NOW, RAW_BODY).expect("verifies");
    }

    #[test]
    fn the_signature_is_over_the_raw_body_bytes() {
        // Signing and verifying agree ONLY over the identical byte
        // sequence: a re-serialized (but semantically equal) body fails
        // verification — the plan §2.3 "RAW body, never re-serialized" rule.
        let set = WebhookSecretSet::single(test_key(0x42));
        let header = set.sign(NOW, RAW_BODY);
        let reserialized = b"{ \"event\": \"ScheduleProposed\", \"schedule_id\": \"s-1\" }";
        assert_eq!(
            set.verify(&header, NOW, reserialized),
            Err(WebhookSigningError::SignatureMismatch)
        );
    }

    #[test]
    fn the_timestamp_is_inside_the_signed_message() {
        // A header re-stamped with a fresh timestamp (keeping the old
        // signature) fails: the timestamp is part of the HMAC input.
        let set = WebhookSecretSet::single(test_key(0x42));
        let header = set.sign(NOW, RAW_BODY);
        let (t, sig) = parse_signature_header(&header).expect("parses");
        let restamped = format!("t={},v1={}", NOW + 1, hex_encode_digest(&sig));
        let _ = t;
        // Within tolerance, but the digest no longer matches the stamped
        // timestamp's message.
        assert_eq!(
            set.verify(&restamped, NOW + 1, RAW_BODY),
            Err(WebhookSigningError::SignatureMismatch)
        );
    }

    #[test]
    fn a_timestamp_outside_the_tolerance_is_rejected_before_signature_checks() {
        let set = WebhookSecretSet::single(test_key(0x42));
        let header = set.sign(NOW - TOLERANCE_SECONDS - 1, RAW_BODY);
        assert_eq!(
            set.verify(&header, NOW, RAW_BODY),
            Err(WebhookSigningError::OutsideTolerance {
                header_timestamp: NOW - TOLERANCE_SECONDS - 1,
                now_unix_seconds: NOW,
            })
        );
        // Exactly AT the tolerance boundary is accepted (inclusive window).
        let edge = set.sign(NOW - TOLERANCE_SECONDS, RAW_BODY);
        set.verify(&edge, NOW, RAW_BODY).expect("boundary inclusive");
        // Future timestamps beyond the tolerance are equally rejected (the
        // window is absolute, not one-sided).
        let future = set.sign(NOW + TOLERANCE_SECONDS + 1, RAW_BODY);
        assert!(matches!(
            set.verify(&future, NOW, RAW_BODY),
            Err(WebhookSigningError::OutsideTolerance { .. })
        ));
    }

    #[test]
    fn a_tampered_signature_fails() {
        let set = WebhookSecretSet::single(test_key(0x42));
        let header = set.sign(NOW, RAW_BODY);
        let (t, mut sig) = parse_signature_header(&header).expect("parses");
        sig[0] ^= 0x01;
        let tampered = format!("t={t},v1={}", hex_encode_digest(&sig));
        assert_eq!(
            set.verify(&tampered, NOW, RAW_BODY),
            Err(WebhookSigningError::SignatureMismatch)
        );
    }

    #[test]
    fn a_wrong_secret_fails() {
        let signer = WebhookSecretSet::single(test_key(0x42));
        let verifier = WebhookSecretSet::single(test_key(0x43));
        let header = signer.sign(NOW, RAW_BODY);
        assert_eq!(
            verifier.verify(&header, NOW, RAW_BODY),
            Err(WebhookSigningError::SignatureMismatch)
        );
    }

    #[test]
    fn during_the_overlap_both_secrets_verify_and_the_current_signs() {
        let previous = test_key(0x01);
        let current = test_key(0x02);
        // The old secret's deliveries keep verifying during the overlap.
        let rolling = WebhookSecretSet::rolling(current, previous);
        let old_header = {
            let old_set = WebhookSecretSet::single(
                WebhookSigningKey::from_bytes(&[0x01; 32]).expect("old key"),
            );
            old_set.sign(NOW, RAW_BODY)
        };
        rolling.verify(&old_header, NOW, RAW_BODY).expect("previous verifies during overlap");
        // New signatures use the CURRENT secret (verified by signing with
        // the rolling set and checking against a current-only set).
        let new_header = rolling.sign(NOW, RAW_BODY);
        let current_only = WebhookSecretSet::single(
            WebhookSigningKey::from_bytes(&[0x02; 32]).expect("current key"),
        );
        current_only.verify(&new_header, NOW, RAW_BODY).expect("signed with current");
        // After the overlap closes (previous revoked), the old header no
        // longer verifies.
        let closed = WebhookSecretSet::single(
            WebhookSigningKey::from_bytes(&[0x02; 32]).expect("current key"),
        );
        assert_eq!(
            closed.verify(&old_header, NOW, RAW_BODY),
            Err(WebhookSigningError::SignatureMismatch)
        );
    }

    #[test]
    fn malformed_headers_fail_with_named_errors() {
        let set = WebhookSecretSet::single(test_key(0x42));
        assert_eq!(set.verify("", NOW, RAW_BODY), Err(WebhookSigningError::HeaderMalformed { reason: "element without a key=value form" }));
        assert_eq!(
            set.verify("t=abc,v1=00", NOW, RAW_BODY),
            Err(WebhookSigningError::HeaderMalformed { reason: "timestamp is not an integer" })
        );
        assert_eq!(
            set.verify(&format!("t={NOW},v1=short"), NOW, RAW_BODY),
            Err(WebhookSigningError::HeaderMalformed { reason: "signature is not 64 hex characters" })
        );
        assert_eq!(
            set.verify(&format!("t={NOW},v2=00"), NOW, RAW_BODY),
            Err(WebhookSigningError::HeaderMalformed { reason: "signature element missing" })
        );
        assert_eq!(
            set.verify(&format!("v1={},t={NOW}", hex_encode_digest(&[0u8; 32])), NOW, RAW_BODY),
            Err(WebhookSigningError::SignatureMismatch),
            "element order is free; both elements present is valid structure"
        );
        // No header at all is its own error (the caller distinguishes the
        // absent-header 401 from the bad-signature 401 record); the closed
        // error vocabulary carries it.
        let _ = WebhookSigningError::HeaderMissing;
    }

    #[test]
    fn secrets_below_the_floor_fail_closed() {
        assert_eq!(
            WebhookSigningKey::from_bytes(&[0x42; 31]),
            Err(WebhookSigningError::SecretTooShort { length: 31 })
        );
        WebhookSigningKey::from_bytes(&[0x42; 32]).expect("at the floor passes");
    }

    // --- The strict `whsec_` parse (record 08 §4; FRV1-R8-004) -----------
    // Every expected value below is hand-derived: 43 base64url characters
    // of 'A' (value 0) decode to 32 zero bytes; 43 '_' characters (value
    // 63) decode to 32 bytes of 0xFF (the two trailing bits are the
    // recorded non-rejected remainder — "decodes to exactly 32 bytes").

    const VALID_ZERO_PAYLOAD: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"; // 43 x 'A'
    const VALID_FF_PAYLOAD: &str = "___________________________________________"; // 43 x '_'

    #[test]
    fn a_valid_whsec_literal_parses_to_the_decoded_key() {
        let key = parse_webhook_secret(&format!("whsec_{VALID_ZERO_PAYLOAD}")).expect("parses");
        assert_eq!(key.as_bytes(), &[0u8; 32]);
        let key = parse_webhook_secret(&format!("whsec_{VALID_FF_PAYLOAD}")).expect("parses");
        assert_eq!(key.as_bytes(), &[0xFF; 32]);
        // The decoded key is a usable signing key (the composition consumes
        // code, not prose).
        let set = WebhookSecretSet::single(key);
        let header = set.sign(NOW, RAW_BODY);
        set.verify(&header, NOW, RAW_BODY).expect("verifies");
    }

    #[test]
    fn a_wrong_prefix_is_a_named_rejection() {
        let payload = VALID_ZERO_PAYLOAD;
        for literal in [
            format!("WHSEC_{payload}"),
            format!("whsec{payload}"),
            format!("whsecx_{payload}"),
        ] {
            assert!(matches!(
                parse_webhook_secret(&literal),
                Err(WebhookSecretParseError::WrongPrefix)
            ));
        }
    }

    #[test]
    fn a_wrong_payload_length_is_a_named_rejection() {
        assert!(matches!(
            parse_webhook_secret(&format!("whsec_{}", &VALID_ZERO_PAYLOAD[..42])),
            Err(WebhookSecretParseError::WrongLength { payload_len: 42 })
        ));
        assert!(matches!(
            parse_webhook_secret(&format!("whsec_{VALID_ZERO_PAYLOAD}A")),
            Err(WebhookSecretParseError::WrongLength { payload_len: 44 })
        ));
    }

    #[test]
    fn padding_characters_are_their_own_rejection_class() {
        assert!(matches!(
            parse_webhook_secret(&format!("whsec_{}=", &VALID_ZERO_PAYLOAD[..42])),
            Err(WebhookSecretParseError::PaddingCharacter)
        ));
        assert!(matches!(
            parse_webhook_secret(&format!("whsec_{}==", &VALID_ZERO_PAYLOAD[..41])),
            Err(WebhookSecretParseError::PaddingCharacter)
        ));
    }

    #[test]
    fn non_alphabet_characters_are_a_named_rejection() {
        for suffix in ["+", "/", " "] {
            assert!(matches!(
                parse_webhook_secret(&format!("whsec_{}{suffix}", &VALID_ZERO_PAYLOAD[..42])),
                Err(WebhookSecretParseError::NonAlphabetCharacter)
            ));
        }
    }

    #[test]
    fn the_refusal_names_are_pinned_and_no_error_echoes_the_value() {
        let payload = format!("whsec_{VALID_ZERO_PAYLOAD}");
        let cases = [
            (
                parse_webhook_secret(&format!("WHSEC_{VALID_ZERO_PAYLOAD}"))
                    .expect_err("wrong prefix refused"),
                "WEBHOOK_SECRET_WRONG_PREFIX",
            ),
            (
                parse_webhook_secret(&format!("whsec_{}", &VALID_ZERO_PAYLOAD[..42]))
                    .expect_err("wrong length refused"),
                "WEBHOOK_SECRET_WRONG_LENGTH",
            ),
            (
                parse_webhook_secret(&format!("whsec_{}=", &VALID_ZERO_PAYLOAD[..42]))
                    .expect_err("padding refused"),
                "WEBHOOK_SECRET_PADDING_CHARACTER",
            ),
            (
                parse_webhook_secret(&format!("whsec_{}+", &VALID_ZERO_PAYLOAD[..42]))
                    .expect_err("non-alphabet refused"),
                "WEBHOOK_SECRET_NON_ALPHABET_CHARACTER",
            ),
        ];
        for (error, expected_name) in cases {
            assert_eq!(error.name(), expected_name);
            // The refusal prose never echoes the rejected literal (it may
            // be live secret material).
            assert!(!error.to_string().contains(&payload));
            assert!(!error.to_string().contains(VALID_ZERO_PAYLOAD));
        }
    }
}
