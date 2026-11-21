// Patch B (P2.07) — The `SourceAuthenticator` port on the database
// adapter, plus the signing-key registry and sequence-watermark
// repository methods on [`PgUnitOfWork`].
//
// What lives here:
//
// - [`PgSourceAuthenticator`]: the production `SourceAuthenticator`.
//   It parses the envelope JSON (the consumer half of the wire codec
//   documented in `flexcore_ports::source_auth`; the producer half lives
//   in the simulator), looks the key up in `source_signing_keys`, and
//   verifies signature → digest → time window → sequence watermark, in
//   that order. The watermark ADVANCE does not happen here: it happens
//   inside the ingestion batch transaction
//   ([`PgUnitOfWork::advance_source_sequence`]), so a batch that fails
//   later never consumes a sequence number.
// - The inherent [`PgUnitOfWork`] methods [`PgUnitOfWork::lookup_source_key`],
//   [`PgUnitOfWork::read_source_sequence`], and
//   [`PgUnitOfWork::advance_source_sequence`] over the migration 0026
//   tables.
//
// Error discipline (P1.07/P1.08): the port error vocabulary has no
// backend variant, so a storage failure inside `authenticate` fails
// CLOSED as `UnknownSourceOrKey` — the frame is quarantined, never
// admitted on a storage error. The ingestion pipeline's quarantine
// reason for that case is therefore the stable unknown-source-or-key
// string, not a storage diagnostic; the storage error itself is not
// observable through this port (an operator diagnoses it from the
// database side, not from a quarantined frame).
//
// Key-lookup and watermark reads run in the authenticator's own
// short-lived unit of work (the reader pattern: the authenticator is
// pool-held, not bound to the caller's unit of work). The read and the
// batch advance are two transactions by design; the race between them
// is closed by the advisory lock + re-check inside
// `advance_source_sequence`, so a replay can never slip through the
// gap: the authenticator's read is an early filter, the advance is the
// authority.
//
// Rotation and revocation (migration 0026): rotation is a new `key_id`
// row (ACTIVE) while the old `key_id` stays valid until its
// `valid_until`; revocation appends a REVOKED term with a higher
// `term_seq` for the same (tenant, source, key_id). The lookup always
// reads the highest-`term_seq` row and accepts it only while ACTIVE
// and inside its validity window.
//
// Scope and control status: simulation-only (the entire 0.x line).
// This module stores PUBLIC keys only — never private keys — and grants
// no control authority.

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::source_auth::{
    canonical_signing_bytes, signable_identifier, SourceAuthError, SourceAuthMode,
    SourceAuthenticator, VerifiedEnvelope, ENVELOPE_VERSION,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::time::Duration;
use time::OffsetDateTime;

use crate::error::{decode_failure, map_backend};
use crate::store::FlexCoreStore;
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use crate::uuid_map::{uuid_from_text, uuid_text};

/// The `dev` key identifier: in `DevKey` mode it verifies against the
/// ephemeral in-process keypair; in `Enforced` mode it is rejected as
/// unknown. Never enrolled in the registry.
const DEV_KEY_ID: &str = "dev";

/// The wire envelope as JSON (the normative field list from
/// `flexcore_ports::source_auth`). Unknown extra fields are ignored by
/// the parser; every listed field is required and shape-checked below.
#[derive(Deserialize)]
struct EnvelopeWire {
    version: u8,
    key_id: String,
    source_id: String,
    tenant_id: String,
    sequence: u64,
    event_time_unix_millis: i64,
    payload_digest: String,
    payload: String,
    signature: String,
}

/// A parsed envelope with every field decoded to its binary form.
struct ParsedEnvelope {
    key_id: String,
    source_id: String,
    tenant_id: TenantId,
    sequence: u64,
    event_time_unix_millis: i64,
    payload_digest: [u8; 32],
    payload: Vec<u8>,
    signature: [u8; 64],
}

/// Decodes `s` as exactly `N` bytes of lowercase hex. Uppercase hex is
/// rejected (the wire format requires lowercase); any other shape is
/// malformed.
fn hex_decode<const N: usize>(s: &str) -> Option<[u8; N]> {
    let bytes = s.as_bytes();
    if bytes.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for i in 0..N {
        let hi = hex_nibble(bytes[2 * i])?;
        let lo = hex_nibble(bytes[2 * i + 1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

/// One lowercase hex digit's value; anything else (including uppercase)
/// is not a hex digit here.
fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// Parses raw frame bytes as a signed envelope. Malformed JSON, a wrong
/// version, a bad hex length or alphabet, bad base64, or an unparsable
/// tenant UUID are all `MalformedEnvelope` (the detail is diagnostic
/// only; it never enters the quarantine reason).
fn parse_envelope(payload: &[u8]) -> Result<ParsedEnvelope, SourceAuthError> {
    let malformed = |detail: &str| SourceAuthError::MalformedEnvelope(detail.to_string());
    let wire: EnvelopeWire = serde_json::from_slice(payload)
        .map_err(|_| malformed("the frame is not a JSON envelope object"))?;
    if wire.version != ENVELOPE_VERSION {
        return Err(malformed("the envelope version is not 1"));
    }
    if wire.key_id.is_empty() {
        return Err(malformed("the envelope key_id is empty"));
    }
    if wire.source_id.is_empty() {
        return Err(malformed("the envelope source_id is empty"));
    }
    // The typed over-long refusal (V-10, FIM wave 8, written-unrun):
    // an identifier beyond the u32 length prefix of the canonical signing
    // bytes has NO signing bytes, so such an envelope can never be signed
    // or verified — refused here, at the parse gate, as a wrong-shape
    // field (the existing `MalformedEnvelope` classification; the stable
    // quarantine reason `1-receive: source-auth: malformed-envelope` is
    // unchanged), before any signing path runs. This is the caller-side
    // half of `push_length_prefixed`'s typed error: callers handle it as
    // a refusal.
    signable_identifier(&wire.key_id)?;
    signable_identifier(&wire.source_id)?;
    let tenant_uuid = FlexUuid::parse(&wire.tenant_id)
        .map_err(|_| malformed("the envelope tenant_id is not a UUID"))?;
    let payload_digest = hex_decode::<32>(&wire.payload_digest)
        .ok_or_else(|| malformed("the payload_digest is not 64 lowercase hex chars"))?;
    let signature = hex_decode::<64>(&wire.signature)
        .ok_or_else(|| malformed("the signature is not 128 lowercase hex chars"))?;
    let payload_bytes = BASE64_STANDARD
        .decode(wire.payload.as_bytes())
        .map_err(|_| malformed("the payload is not standard-alphabet base64"))?;
    Ok(ParsedEnvelope {
        key_id: wire.key_id,
        source_id: wire.source_id,
        tenant_id: TenantId::new(tenant_uuid),
        sequence: wire.sequence,
        event_time_unix_millis: wire.event_time_unix_millis,
        payload_digest,
        payload: payload_bytes,
        signature,
    })
}

/// The wall-clock time now, milliseconds since the Unix epoch.
fn now_unix_millis() -> i64 {
    let now = OffsetDateTime::now_utc();
    now.unix_timestamp() * 1000 + now.millisecond() as i64
}

/// The production source authenticator (Patch B): pool-held on the
/// store; each `authenticate` call opens its own short-lived
/// tenant-scoped unit of work for the key lookup and the watermark read
/// (see the module header for the design decision).
pub struct PgSourceAuthenticator {
    store: FlexCoreStore,
    mode: SourceAuthMode,
    window: Duration,
    dev_keys: Vec<[u8; 32]>,
}

impl PgSourceAuthenticator {
    /// Builds the authenticator. `dev_keys` holds the verifying-key
    /// bytes of the ephemeral dev keypair (empty in `Enforced` mode);
    /// the corresponding signing key is never stored here — it lives
    /// only in the producer that signs.
    pub fn new(
        store: FlexCoreStore,
        mode: SourceAuthMode,
        window: Duration,
        dev_keys: Vec<[u8; 32]>,
    ) -> Self {
        Self {
            store,
            mode,
            window,
            dev_keys,
        }
    }
}

impl SourceAuthenticator for PgSourceAuthenticator {
    fn authenticate(
        &self,
        ctx: &TenantContext,
        payload: &[u8],
    ) -> Result<VerifiedEnvelope, SourceAuthError> {
        let parsed = parse_envelope(payload)?;
        // The envelope's tenant must be the pipeline's tenant scope: a
        // frame claiming another tenant is not this scope's frame, and
        // the registry lookup below is scoped to `ctx` either way.
        if parsed.tenant_id != ctx.tenant_id() {
            return Err(SourceAuthError::UnknownSourceOrKey);
        }
        // The `dev` identifier never reaches the registry: in Enforced
        // mode it is unknown by definition; in DevKey mode it verifies
        // against the in-process key set.
        let is_dev = parsed.key_id == DEV_KEY_ID;
        if is_dev && self.mode == SourceAuthMode::Enforced {
            return Err(SourceAuthError::UnknownSourceOrKey);
        }
        // One short-lived unit of work for the key lookup (registry
        // mode only) and the watermark read. A storage failure fails
        // closed as unknown-source-or-key (see the module header).
        let (registry_key, last_sequence) = {
            let unit = self
                .store
                .begin(Some(ctx))
                .map_err(|_| SourceAuthError::UnknownSourceOrKey)?;
            let registry_key = if is_dev {
                None
            } else {
                unit.lookup_source_key(ctx, &parsed.source_id, &parsed.key_id)
                    .map_err(|_| SourceAuthError::UnknownSourceOrKey)?
            };
            let last_sequence = unit
                .read_source_sequence(ctx, &parsed.source_id)
                .map_err(|_| SourceAuthError::UnknownSourceOrKey)?;
            unit.commit()
                .map_err(|_| SourceAuthError::UnknownSourceOrKey)?;
            (registry_key, last_sequence)
        };
        // Signature over the canonical signing bytes. A stored public
        // key of the wrong length is treated as unknown (the migration
        // CHECK enforces 32 bytes; this is defence in depth).
        let signing_bytes = canonical_signing_bytes(
            ENVELOPE_VERSION,
            &parsed.key_id,
            &parsed.source_id,
            &parsed.tenant_id,
            parsed.sequence,
            parsed.event_time_unix_millis,
            &parsed.payload_digest,
        );
        let signature = Signature::from_bytes(&parsed.signature);
        if is_dev {
            let mut verified = false;
            for key_bytes in &self.dev_keys {
                if let Ok(verifying_key) = VerifyingKey::from_bytes(key_bytes) {
                    if verifying_key
                        .verify(&signing_bytes, &signature)
                        .is_ok()
                    {
                        verified = true;
                        break;
                    }
                }
            }
            if !verified {
                return Err(SourceAuthError::BadSignature);
            }
        } else {
            let row = registry_key.ok_or(SourceAuthError::UnknownSourceOrKey)?;
            let key_bytes: [u8; 32] = row
                .public_key
                .try_into()
                .map_err(|_| SourceAuthError::UnknownSourceOrKey)?;
            let verifying_key = VerifyingKey::from_bytes(&key_bytes)
                .map_err(|_| SourceAuthError::UnknownSourceOrKey)?;
            verifying_key
                .verify(&signing_bytes, &signature)
                .map_err(|_| SourceAuthError::BadSignature)?;
        }
        // Digest: SHA-256 of the inner payload must equal the signed
        // digest (checked after the signature, so a forged digest
        // cannot pass with a valid signature over it — the signature
        // covers the digest, and the digest covers the payload).
        let computed: [u8; 32] = Sha256::digest(&parsed.payload).into();
        if computed != parsed.payload_digest {
            return Err(SourceAuthError::BadDigest);
        }
        // Time window: ±window around the receiver's clock. The window
        // is specified in whole SECONDS (port contract:
        // `DEFAULT_AUTH_WINDOW_SECS` = 300, "within ±300 s of the
        // receiver's clock"), so the comparison is in whole seconds,
        // ROUNDED to the nearest second. A strict-millisecond comparison
        // flaked the edge-accept test: the sender signs at its own
        // `now - 300_000 ms` and the receiver reads its clock a few
        // milliseconds later (after the key lookup), so the past-edge
        // frame was 300_000 + delta ms old and rejected as outside the
        // window (the test file's TOLERANCE NOTE names the implementation
        // as the thing to fix). Rounding keeps the specified semantics —
        // exactly ±300 s accepts, ±301 s rejects — and absorbs the
        // sub-second receiver skew on both sides (a plain truncation
        // widened the future side by up to 999 ms and let a +301 s frame
        // through). The residual flake window is a sign-to-check latency
        // above 500 ms, far outside the normal millisecond key lookup.
        let now_millis = now_unix_millis();
        let window_secs = self.window.as_secs() as i128;
        let age_millis =
            (parsed.event_time_unix_millis as i128 - now_millis as i128).abs();
        let age_secs = (age_millis + 500) / 1000;
        if age_secs > window_secs {
            return Err(SourceAuthError::OutsideTimeWindow);
        }
        // Watermark (early filter only — the batch advance is the
        // authority, under its advisory lock).
        if let Some(last) = last_sequence {
            if parsed.sequence <= last {
                return Err(SourceAuthError::ReplayOrStaleSequence);
            }
        }
        Ok(VerifiedEnvelope {
            key_id: parsed.key_id,
            source_id: parsed.source_id,
            tenant_id: parsed.tenant_id,
            sequence: parsed.sequence,
            event_time_unix_millis: parsed.event_time_unix_millis,
            was_enveloped: true,
            payload: parsed.payload,
        })
    }
}

/// One row of the signing-key registry (`source_signing_keys`,
/// migration 0026): the CURRENT term (highest `term_seq`) for a
/// (tenant, source, key) triple, as read by
/// [`PgUnitOfWork::lookup_source_key`]. The caller treats a missing row
/// and an unacceptable row identically (both are `None`): revoked,
/// expired, and unknown keys are indistinguishable by design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceKeyRow {
    pub key_id: String,
    pub source_id: String,
    pub tenant_id: TenantId,
    /// The Ed25519 verifying key (32 bytes; public-key material only).
    pub public_key: Vec<u8>,
    /// The current term's status: `ACTIVE`, `REVOKED`, or `EXPIRED`.
    pub status: String,
    /// The validity window, milliseconds since the Unix epoch.
    pub valid_from_unix_millis: i64,
    pub valid_until_unix_millis: Option<i64>,
}

/// Whether [`PgUnitOfWork::advance_source_sequence`] moved the
/// watermark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvanceOutcome {
    /// The sequence was strictly greater than the watermark and the
    /// event row was inserted.
    Advanced,
    /// The sequence was at or below the watermark: a replay or a stale
    /// frame. Nothing was inserted.
    Stale,
}

impl PgUnitOfWork {
    /// Reads the CURRENT term (highest `term_seq`) of the signing key
    /// for (`ctx` tenant, `source_id`, `key_id`) inside THIS unit of
    /// work's transaction. Returns `None` when no term exists OR when
    /// the current term is not acceptable — status is not `ACTIVE`, or
    /// `valid_from` is in the future, or `valid_until` has passed — so
    /// the caller reports revoked/expired/unknown identically as
    /// `UnknownSourceOrKey`. Storage failures surface as `BackendError`
    /// (the authenticator maps them fail-closed; see the module header).
    pub fn lookup_source_key(
        &self,
        ctx: &TenantContext,
        source_id: &str,
        key_id: &str,
    ) -> Result<Option<SourceKeyRow>, BackendError> {
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let source_id = source_id.to_string();
        let key_id = key_id.to_string();
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await?;
            let row = sqlx::query(
                "SELECT key_id, source_id, tenant_id::text AS tenant_id, public_key, \
                        status, valid_from_unix_millis, valid_until_unix_millis \
                 FROM source_signing_keys \
                 WHERE tenant_id = $1::uuid AND source_id = $2 AND key_id = $3 \
                 ORDER BY term_seq DESC LIMIT 1",
            )
            .bind(&tenant_text)
            .bind(&source_id)
            .bind(&key_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(map_backend)?;
            let Some(row) = row else {
                return Ok(None);
            };
            let tenant_raw: String = row
                .try_get("tenant_id")
                .map_err(|_| decode_failure("source_signing_keys.tenant_id"))?;
            let tenant_uuid = uuid_from_text(&tenant_raw)
                .map_err(|_| decode_failure("source_signing_keys.tenant_id"))?;
            let public_key: Vec<u8> = row
                .try_get("public_key")
                .map_err(|_| decode_failure("source_signing_keys.public_key"))?;
            let status: String = row
                .try_get("status")
                .map_err(|_| decode_failure("source_signing_keys.status"))?;
            let valid_from_unix_millis: i64 = row
                .try_get("valid_from_unix_millis")
                .map_err(|_| decode_failure("source_signing_keys.valid_from_unix_millis"))?;
            let valid_until_unix_millis: Option<i64> = row
                .try_get("valid_until_unix_millis")
                .map_err(|_| decode_failure("source_signing_keys.valid_until_unix_millis"))?;
            let now_millis = now_unix_millis();
            let acceptable = status == "ACTIVE"
                && valid_from_unix_millis <= now_millis
                && valid_until_unix_millis
                    .map(|until| now_millis < until)
                    .unwrap_or(true);
            if !acceptable {
                return Ok(None);
            }
            Ok(Some(SourceKeyRow {
                key_id: row
                    .try_get("key_id")
                    .map_err(|_| decode_failure("source_signing_keys.key_id"))?,
                source_id: row
                    .try_get("source_id")
                    .map_err(|_| decode_failure("source_signing_keys.source_id"))?,
                tenant_id: TenantId::new(tenant_uuid),
                public_key,
                status,
                valid_from_unix_millis,
                valid_until_unix_millis,
            }))
        })
    }

    /// Reads the highest accepted sequence for (`ctx` tenant,
    /// `source_id`) from `source_sequence_events` inside THIS unit of
    /// work's transaction. `None` means no sequence has been accepted
    /// yet for this source.
    pub fn read_source_sequence(
        &self,
        ctx: &TenantContext,
        source_id: &str,
    ) -> Result<Option<u64>, BackendError> {
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let source_id = source_id.to_string();
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await?;
            let row = sqlx::query(
                "SELECT MAX(sequence) AS max_sequence \
                 FROM source_sequence_events \
                 WHERE tenant_id = $1::uuid AND source_id = $2",
            )
            .bind(&tenant_text)
            .bind(&source_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(map_backend)?;
            let max: Option<i64> = row
                .try_get("max_sequence")
                .map_err(|_| decode_failure("source_sequence_events.sequence"))?;
            match max {
                None => Ok(None),
                Some(value) if value < 0 => {
                    Err(decode_failure("source_sequence_events.sequence"))
                }
                Some(value) => Ok(Some(value as u64)),
            }
        })
    }

    /// Advances the sequence watermark for (`ctx` tenant, `source_id`)
    /// to `sequence`, inside the CALLER's transaction (the ingestion
    /// batch transaction): the advance commits exactly when the batch
    /// commits, so a rolled-back batch never consumes a sequence
    /// number. Concurrency is serialized per source with a
    /// transaction-scoped advisory lock keyed on
    /// `hashtext(tenant_uuid || ':' || source_id)`; the MAX is then
    /// re-read under the lock, so two concurrent advances for one
    /// source cannot both pass. `Stale` (nothing inserted) is returned
    /// when `sequence` is at or below the watermark. No UPDATE exists
    /// anywhere on this path: the watermark moves only by INSERT.
    pub fn advance_source_sequence(
        &self,
        ctx: &TenantContext,
        source_id: &str,
        sequence: u64,
    ) -> Result<AdvanceOutcome, BackendError> {
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let lock_key = format!("{}:{}", tenant_text, source_id);
        let source_id = source_id.to_string();
        let sequence_i64 = i64::try_from(sequence)
            .map_err(|_| decode_failure("source_sequence_events.sequence"))?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await?;
            // Serialize concurrent advances for this one source for the
            // life of this transaction. `hashtext` is stable within a
            // database major version; the key is advisory only (a
            // collision across sources would serialize, never corrupt).
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1)::bigint)")
                .bind(&lock_key)
                .execute(&mut **tx)
                .await
                .map_err(map_backend)?;
            let row = sqlx::query(
                "SELECT MAX(sequence) AS max_sequence \
                 FROM source_sequence_events \
                 WHERE tenant_id = $1::uuid AND source_id = $2",
            )
            .bind(&tenant_text)
            .bind(&source_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(map_backend)?;
            let max: Option<i64> = row
                .try_get("max_sequence")
                .map_err(|_| decode_failure("source_sequence_events.sequence"))?;
            if let Some(max) = max {
                if sequence_i64 <= max {
                    return Ok(AdvanceOutcome::Stale);
                }
            }
            let recorded_at = now_unix_millis();
            sqlx::query(
                "INSERT INTO source_sequence_events \
                 (tenant_id, source_id, sequence, recorded_at_unix_millis) \
                 VALUES ($1::uuid, $2, $3, $4)",
            )
            .bind(&tenant_text)
            .bind(&source_id)
            .bind(sequence_i64)
            .bind(recorded_at)
            .execute(&mut **tx)
            .await
            .map_err(map_backend)?;
            Ok(AdvanceOutcome::Advanced)
        })
    }
}
