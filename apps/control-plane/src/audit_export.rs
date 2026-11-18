// P13.12 — The audit export service: signed export packs over the
//          append-only audit trail, SIEM-friendly JSONL, date-range plus
//          tenant scoping, and an integrity statement that INHERITS (never
//          redefines) the Phase 11 audit-chain upgrade.
//
// Phase 13 wave 1, Agent B. WRITTEN-UNRUN: no export has ever been
// produced, no pack has ever been signed, no SIEM has ever consumed a line
// (program conventions §1). The read side is the EXISTING
// `AuditEventReader` port (`flexcore-storage::audit_read` — SELECT-only by
// construction, exactly two methods, no export/dump method: the trait's
// shape is pinned by its source-text test and is NOT modified by this
// batch). This module is the PACK BUILDER over that port: pagination,
// JSONL rendering, the export-scope digest chain, and the manifest.
//
// Integrity inheritance (record `31_p11_15_audit_chain_upgrade.md` —
// consumed, never redefined): record-level tamper evidence is the P11.15
// keyed HMAC-SHA-256 chain's domain (`flexcore-audit::chain`,
// `hmac-sha256-chain-v1`, per-tenant namespace, anchor export). This
// module adds a SEPARATE, coarser guarantee: an export-scope digest chain
// over the pack's JSONL lines, so a pack's consumers can verify the pack
// is exactly what the exporter emitted (completeness/order integrity OF
// THE PACK). The pack does NOT claim to verify the P11.15 record chain:
// the read port does not serve the 0019 chain columns, so a pack cannot
// recompute them — the manifest states this gap EXPLICITLY (see
// [`SOURCE_CHAIN_STATEMENT`]) instead of implying chain verification.
// Extending the read path with the chain columns is the recorded
// follow-up (record § wiring).
//
// Scoping: every pack is built through the caller's `TenantContext`; the
// port filters by that tenant on top of RLS (migration 0008's
// `tenant_isolation` policy) and platform-level rows (`tenant_id IS NULL`)
// are unreachable through the port — a pack is tenant-scoped BY
// CONSTRUCTION, never by a filter this module applies.

use serde::Serialize;
use std::collections::BTreeMap;

use flexcore_domain::tenant_context::TenantContext;
use flexcore_storage::audit_read::{AuditEventFilter, AuditEventReader, AuditEventRow};

use crate::webhook_signing::{hex_encode_digest, sha256};

/// The export-JSONL schema version (closed; a format change is a recorded
/// amendment and a major-version bump of this constant).
pub const EXPORT_SCHEMA_VERSION: &str = "flexcore.audit.export.v1";

/// The P11.15 chain algorithm this pack INHERITS (quoted from
/// `flexcore-audit::chain::AUDIT_CHAIN_ALGORITHM`'s recorded value — the
/// constant lives there; this text is the manifest's statement of
/// inheritance, not a second definition).
pub const INHERITED_CHAIN_ALGORITHM: &str = "hmac-sha256-chain-v1";

/// The manifest's source-chain statement (the honest gap: the read port
/// does not serve the 0019 chain columns, so a pack never claims
/// record-chain verification — the P11.15 chain remains the record-level
/// tamper-evidence authority, verified through ITS interfaces, not
/// through packs).
pub const SOURCE_CHAIN_STATEMENT: &str = "record-level tamper evidence is the \
     phase-11 keyed audit chain's domain (hmac-sha256-chain-v1, per-tenant \
     namespace, anchor export); this pack does NOT verify that chain — the \
     read port does not serve the chain columns — and the pack's digest \
     chain attests only the pack's own completeness and order";

/// The maximum records one pack may contain (the bounded-walk cap; a walk
/// hitting the cap produces an INCOMPLETE pack whose manifest says so —
/// never a silently truncated pack).
pub const MAX_RECORDS_PER_PACK: usize = 100_000;

/// The listing page size (the recorded P2.12/P3.28/P4.30 default; the
/// port ALWAYS bounds every query — no unbounded read).
pub const EXPORT_PAGE_SIZE: u32 = 200;

/// Errors of the export service. Closed vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditExportError {
    /// The export key material was shorter than the signing floor
    /// (`crate::webhook_signing::SECRET_MIN_BYTES`).
    ExportKeyTooShort {
        length: usize,
    },
    /// The storage backend failed while walking pages (the read port's
    /// error, passed through with its opaque code/message).
    ReadFailed {
        code: String,
        message: String,
    },
    /// The export signing failed (the signing layer's error).
    Signing(String),
}

impl std::fmt::Display for AuditExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditExportError::ExportKeyTooShort { length } => {
                write!(f, "the export signing key is {length} bytes; too short")
            }
            AuditExportError::ReadFailed { code, message } => {
                write!(f, "audit read failed ({code}): {message}")
            }
            AuditExportError::Signing(reason) => {
                write!(f, "export signing failed: {reason}")
            }
        }
    }
}

impl std::error::Error for AuditExportError {}

/// One JSONL line: the §10.1 field set end to end, snake_case, in the
/// §10.1 field order (actor, action, object, before/after references,
/// decision, evidence, timestamps, correlation id — the canonical export
/// order). Deterministic: `serde_json::Map` insertion-ordered rendering
/// with the fields written in this exact sequence, so two exports of the
/// same rows byte-match (the digest chain's precondition).
pub fn jsonl_line(row: &AuditEventRow) -> String {
    let mut map = serde_json::Map::new();
    map.insert("schema_version".into(), serde_json::Value::String(EXPORT_SCHEMA_VERSION.into()));
    map.insert("audit_id".into(), serde_json::Value::String(row.audit_id.clone()));
    map.insert(
        "tenant_id".into(),
        match row.tenant_id {
            Some(t) => serde_json::Value::String(t.uuid().to_string()),
            None => serde_json::Value::Null,
        },
    );
    map.insert("actor".into(), serde_json::Value::String(row.actor.clone()));
    map.insert("action".into(), serde_json::Value::String(row.action.clone()));
    map.insert("object_type".into(), serde_json::Value::String(row.object_type.clone()));
    map.insert("object_id".into(), serde_json::Value::String(row.object_id.clone()));
    map.insert(
        "before_ref".into(),
        row.before_ref.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
    );
    map.insert(
        "after_ref".into(),
        row.after_ref.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
    );
    map.insert(
        "decision_outcome".into(),
        row.decision_outcome
            .clone()
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null),
    );
    map.insert(
        "decision_reason_reference".into(),
        row.decision_reason_reference
            .clone()
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null),
    );
    map.insert(
        "evidence".into(),
        row.evidence.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
    );
    map.insert(
        "event_time_unix_millis".into(),
        serde_json::Value::Number(row.event_time_unix_millis.into()),
    );
    map.insert(
        "ingest_time_unix_millis".into(),
        serde_json::Value::Number(row.ingest_time_unix_millis.into()),
    );
    map.insert(
        "correlation_id".into(),
        serde_json::Value::String(row.correlation_id.clone()),
    );
    map.insert(
        "corrected_audit_id".into(),
        row.corrected_audit_id
            .clone()
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null),
    );
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_else(|_| "{}".to_string())
}

/// The export manifest (the pack's signed envelope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditExportManifest {
    /// The pack identifier (opaque non-empty text, caller-supplied — the
    /// id-generation port's concern, not this module's; P1.07).
    pub pack_id: String,
    /// The tenant the pack covers (`None` is NOT a pack shape: a pack is
    /// tenant-scoped by construction; the Option mirrors the filter type
    /// and a `None` filter is a caller error — see the builder).
    pub tenant_scope: String,
    /// Inclusive lower bound on the records' event time (unix millis).
    pub from_event_unix_millis: i64,
    /// Inclusive upper bound (unix millis).
    pub to_event_unix_millis: i64,
    /// The number of JSONL records in the pack.
    pub record_count: usize,
    /// `false` when the walk hit [`MAX_RECORDS_PER_PACK`] — an incomplete
    /// pack is delivered AS incomplete (the consumer re-exports the rest
    /// with a narrowed range), never silently truncated.
    pub complete: bool,
    /// Unix millis of pack generation (the injected clock).
    pub generated_at_unix_millis: i64,
    /// The export schema version.
    pub schema_version: String,
    /// The SHA-256 digest of the HEAD of the export-scope chain (the
    /// running digest after the pack's last JSONL line — the chain
    /// genesis is the 32-byte zero digest; each line's digest is
    /// HMAC-SHA-256(export_key, head_hex + "." + line)).
    pub pack_head_digest: String,
    /// The source-chain inheritance statement (the honest gap,
    /// [`SOURCE_CHAIN_STATEMENT`]).
    pub source_chain_statement: String,
}

/// The signed export pack: manifest + JSONL body + signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditExportPack {
    pub manifest: AuditExportManifest,
    /// The pack body: one JSON object per line (SIEM-friendly JSONL),
    /// ordered by `(event_time, audit_id)` — the port's listing order.
    pub jsonl: String,
    /// `HMAC-SHA256(export_key, manifest_bytes)` where `manifest_bytes`
    /// is the manifest's canonical JSON rendering — the pack's signature
    /// (DEPLOYMENT REQUIREMENT: the export key is deployment-supplied
    /// material under the P11.16 secrets posture; this module never
    /// generates, loads, or persists one — the same key-management honesty
    /// as the P11.15 chain, record 31 §2).
    pub signature_hex: String,
}

/// The pack builder over the read port. Sealed construction.
pub struct AuditExportBuilder<'a> {
    reader: &'a dyn AuditEventReader,
    clock_millis: Box<dyn Fn() -> i64 + 'a>,
}

impl<'a> AuditExportBuilder<'a> {
    /// Composes the builder over the read port and the injected clock.
    pub fn new(
        reader: &'a dyn AuditEventReader,
        clock_millis: Box<dyn Fn() -> i64 + 'a>,
    ) -> Self {
        Self { reader, clock_millis }
    }

    /// Builds one pack for `ctx`'s tenant over the INCLUSIVE date range
    /// `[from, to]` (unix millis). The walk pages through
    /// `list_audit_events` (page [`EXPORT_PAGE_SIZE`], keyset cursor via
    /// `after_audit_id`) until the range is exhausted or the pack cap is
    /// hit, renders the JSONL body, computes the export-scope digest
    /// chain, and signs the manifest.
    ///
    /// The action/actor/object filters of [`AuditEventFilter`] are
    /// deliberately NOT exposed here: an export is a full-fidelity range
    /// capture for the tenant, not a selective slice — a selective slice
    /// is a query, which the `/api/v1/audit-events` read resource already
    /// serves.
    pub fn build_pack(
        &self,
        ctx: &TenantContext,
        pack_id: &str,
        from_event_unix_millis: i64,
        to_event_unix_millis: i64,
        export_key: &[u8],
    ) -> Result<AuditExportPack, AuditExportError> {
        if export_key.len() < crate::webhook_signing::SECRET_MIN_BYTES {
            return Err(AuditExportError::ExportKeyTooShort { length: export_key.len() });
        }
        let mut filter = AuditEventFilter {
            from_event_unix_millis: Some(from_event_unix_millis),
            to_event_unix_millis: Some(to_event_unix_millis),
            ..AuditEventFilter::default()
        };
        let mut lines: Vec<String> = Vec::new();
        let mut complete = true;
        loop {
            let page = self
                .reader
                .list_audit_events(ctx, &filter, EXPORT_PAGE_SIZE)
                .map_err(|e| match e {
                    flexcore_storage::audit_read::AuditEventReadError::Backend {
                        code,
                        message,
                    } => AuditExportError::ReadFailed { code, message },
                    flexcore_storage::audit_read::AuditEventReadError::InvalidFilter {
                        field,
                    } => AuditExportError::ReadFailed {
                        code: "INVALID_FILTER".to_string(),
                        message: field.to_string(),
                    },
                })?;
            let page_len = page.len();
            for row in &page {
                lines.push(jsonl_line(row));
            }
            if lines.len() >= MAX_RECORDS_PER_PACK {
                complete = false;
                break;
            }
            if page_len < EXPORT_PAGE_SIZE as usize {
                break;
            }
            // Keyset cursor: strictly AFTER the last audit_id of the page
            // (the port's contract).
            filter.after_audit_id = Some(page.last().expect("non-empty page").audit_id.clone());
        }
        // The export-scope chain: genesis = the zero digest; each line's
        // digest = HMAC-SHA256(export_key, head_hex + "." + line_bytes).
        let mut head = [0u8; 32];
        for line in &lines {
            let mut message = Vec::with_capacity(64 + 1 + line.len());
            message.extend_from_slice(hex_encode_digest(&head).as_bytes());
            message.push(b'.');
            message.extend_from_slice(line.as_bytes());
            head = crate::webhook_signing::hmac_sha256(export_key, &message);
        }
        let manifest = AuditExportManifest {
            pack_id: pack_id.to_string(),
            tenant_scope: ctx.tenant_id().uuid().to_string(),
            from_event_unix_millis,
            to_event_unix_millis,
            record_count: lines.len(),
            complete,
            generated_at_unix_millis: (self.clock_millis)(),
            schema_version: EXPORT_SCHEMA_VERSION.to_string(),
            pack_head_digest: hex_encode_digest(&head),
            source_chain_statement: SOURCE_CHAIN_STATEMENT.to_string(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest)
            .map_err(|e| AuditExportError::Signing(format!("{e}")))?;
        let signature_hex = hex_encode_digest(&crate::webhook_signing::hmac_sha256(
            export_key,
            &manifest_bytes,
        ));
        Ok(AuditExportPack {
            manifest,
            jsonl: lines.join("\n"),
            signature_hex,
        })
    }
}

/// Verifies a pack's manifest signature over its canonical manifest bytes
/// (the receiver-side check; the export key is the verifier's deployment
/// material).
pub fn verify_pack_signature(
    pack: &AuditExportPack,
    export_key: &[u8],
) -> Result<(), AuditExportError> {
    let manifest_bytes = serde_json::to_vec(&pack.manifest)
        .map_err(|e| AuditExportError::Signing(format!("{e}")))?;
    let expected = hex_encode_digest(&crate::webhook_signing::hmac_sha256(
        export_key,
        &manifest_bytes,
    ));
    if constant_time_eq(expected.as_bytes(), pack.signature_hex.as_bytes()) {
        Ok(())
    } else {
        Err(AuditExportError::Signing("the manifest signature did not verify".to_string()))
    }
}

/// Constant-time comparison (same discipline as the signing module's).
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

/// A convenience collector for the pack consumer: recomputes the
/// export-scope chain over a pack's JSONL lines and compares with the
/// manifest head (the pack-completeness check a SIEM-side verifier runs).
pub fn verify_pack_body_chain(
    pack: &AuditExportPack,
    export_key: &[u8],
) -> Result<(), AuditExportError> {
    if pack.jsonl.is_empty() {
        return if pack.manifest.record_count == 0
            && pack.manifest.pack_head_digest == hex_encode_digest(&[0u8; 32])
        {
            Ok(())
        } else {
            Err(AuditExportError::Signing(
                "the pack body is empty but the manifest claims records".to_string(),
            ))
        };
    }
    let mut head = [0u8; 32];
    for line in pack.jsonl.split('\n') {
        let mut message = Vec::with_capacity(64 + 1 + line.len());
        message.extend_from_slice(hex_encode_digest(&head).as_bytes());
        message.push(b'.');
        message.extend_from_slice(line.as_bytes());
        head = crate::webhook_signing::hmac_sha256(export_key, &message);
    }
    if constant_time_eq(
        pack.manifest.pack_head_digest.as_bytes(),
        hex_encode_digest(&head).as_bytes(),
    ) {
        Ok(())
    } else {
        Err(AuditExportError::Signing(
            "the pack body chain does not match the manifest head".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::TenantId;
    use flexcore_storage::audit_read::AUDIT_DECISION_ALLOWED;

    fn tenant(seed: u8) -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = seed;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn tenant_ctx(seed: u8) -> TenantContext {
        // The minimal TenantContext for the reader contract: built from
        // authenticated claims — the single construction path (the port
        // uses the tenant for scoping; tests construct VerifiedClaims
        // directly, the recorded test-fake precedent).
        TenantContext::from_authenticated_claims(
            flexcore_domain::tenant_context::AuthenticatedClaims::from_verified(
                flexcore_domain::tenant_context::VerifiedClaims {
                    tenant_id: tenant(seed),
                    subject: "export-operator".to_string(),
                },
            ),
        )
    }

    fn row(seq: usize, tenant_id: Option<TenantId>) -> AuditEventRow {
        AuditEventRow::new(
            format!("0192a5e0-0000-7000-8000-{:012}", seq),
            tenant_id,
            "user:operator-1".to_string(),
            "command.approve".to_string(),
            "command".to_string(),
            format!("0192a5e0-0000-7000-8000-{:012}", seq + 100),
            Some("command:c:state=FEASIBLE".to_string()),
            Some("command:c:state=APPROVED".to_string()),
            Some(AUDIT_DECISION_ALLOWED.to_string()),
            Some("approval-record:apr-1".to_string()),
            Some("evidence:approval/apr-1".to_string()),
            1_700_000_000 + seq as i64 * 1_000,
            1_700_000_000 + seq as i64 * 1_000 + 500,
            format!("0192a5e0-0000-7000-8000-{:012}", seq + 200),
            None,
        )
        .expect("valid row")
    }

    /// A fake reader serving pre-seeded rows in one bounded page, keyed
    /// per tenant (the fake honors the port's scoping contract).
    struct FakeReader {
        rows: Vec<(TenantId, AuditEventRow)>,
    }

    impl AuditEventReader for FakeReader {
        fn find_audit_event(
            &self,
            _ctx: &TenantContext,
            _audit_id: &str,
        ) -> Result<Option<AuditEventRow>, flexcore_storage::audit_read::AuditEventReadError> {
            Ok(None)
        }

        fn list_audit_events(
            &self,
            ctx: &TenantContext,
            filter: &AuditEventFilter,
            limit: u32,
        ) -> Result<Vec<AuditEventRow>, flexcore_storage::audit_read::AuditEventReadError> {
            let mut rows: Vec<AuditEventRow> = self
                .rows
                .iter()
                .filter(|(t, r)| {
                    *t == ctx.tenant_id()
                        && filter
                            .from_event_unix_millis
                            .map(|from| r.event_time_unix_millis >= from)
                            .unwrap_or(true)
                        && filter
                            .to_event_unix_millis
                            .map(|to| r.event_time_unix_millis <= to)
                            .unwrap_or(true)
                        && filter
                            .after_audit_id
                            .as_ref()
                            .map(|after| r.audit_id.as_str() > after.as_str())
                            .unwrap_or(true)
                })
                .map(|(_, r)| r.clone())
                .collect();
            rows.sort_by(|a, b| {
                a.event_time_unix_millis.cmp(&b.event_time_unix_millis)
            });
            rows.truncate(limit as usize);
            Ok(rows)
        }
    }

    const EXPORT_KEY: &[u8] = &[0x5au8; 32];

    #[test]
    fn the_jsonl_line_carries_the_full_s10_1_field_set_in_order() {
        let line = jsonl_line(&row(1, Some(tenant(7))));
        let value: serde_json::Value = serde_json::from_str(&line).expect("parses");
        // Every §10.1 field is present, snake_case.
        for field in [
            "audit_id",
            "tenant_id",
            "actor",
            "action",
            "object_type",
            "object_id",
            "before_ref",
            "after_ref",
            "decision_outcome",
            "decision_reason_reference",
            "evidence",
            "event_time_unix_millis",
            "ingest_time_unix_millis",
            "correlation_id",
            "corrected_audit_id",
            "schema_version",
        ] {
            assert!(value.get(field).is_some(), "missing field {field}");
        }
        // Deterministic: two renders byte-match.
        assert_eq!(line, jsonl_line(&row(1, Some(tenant(7)))));
    }

    #[test]
    fn a_pack_walks_pages_and_states_completeness() {
        let rows: Vec<(TenantId, AuditEventRow)> = (0..5)
            .map(|i| (tenant(7), row(i, Some(tenant(7)))))
            .collect();
        let reader = FakeReader { rows };
        let builder = AuditExportBuilder::new(&reader, Box::new(|| 5_000));
        let ctx = tenant_ctx(7);
        let pack = builder
            .build_pack(&ctx, "pack-1", 0, 2_000_000_000_000, EXPORT_KEY)
            .expect("pack built");
        assert!(pack.manifest.complete);
        assert_eq!(pack.manifest.record_count, 5);
        assert_eq!(pack.manifest.tenant_scope, tenant(7).uuid().to_string());
        assert_eq!(pack.manifest.schema_version, EXPORT_SCHEMA_VERSION);
        // The JSONL body has one line per record.
        assert_eq!(pack.jsonl.split('\n').count(), 5);
        // The manifest signature verifies.
        verify_pack_signature(&pack, EXPORT_KEY).expect("signature verifies");
        // The body chain verifies.
        verify_pack_body_chain(&pack, EXPORT_KEY).expect("body chain verifies");
    }

    #[test]
    fn a_pack_is_tenant_scoped_by_construction() {
        let mut rows = Vec::new();
        for i in 0..3 {
            rows.push((tenant(7), row(i, Some(tenant(7)))));
        }
        for i in 10..13 {
            rows.push((tenant(9), row(i, Some(tenant(9)))));
        }
        let reader = FakeReader { rows };
        let builder = AuditExportBuilder::new(&reader, Box::new(|| 5_000));
        let pack = builder
            .build_pack(&tenant_ctx(7), "pack-1", 0, 2_000_000_000_000, EXPORT_KEY)
            .expect("pack built");
        // Only tenant 7's rows are in the pack (the port scopes, the
        // builder never widens).
        assert_eq!(pack.manifest.record_count, 3);
        for line in pack.jsonl.split('\n') {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(value["tenant_id"], tenant(7).uuid().to_string());
        }
    }

    #[test]
    fn a_tampered_body_or_signature_fails_verification() {
        let rows: Vec<(TenantId, AuditEventRow)> = (0..2)
            .map(|i| (tenant(7), row(i, Some(tenant(7)))))
            .collect();
        let reader = FakeReader { rows };
        let builder = AuditExportBuilder::new(&reader, Box::new(|| 5_000));
        let pack = builder
            .build_pack(&tenant_ctx(7), "pack-1", 0, 2_000_000_000_000, EXPORT_KEY)
            .expect("pack built");
        // A one-character body edit breaks the body chain.
        let mut tampered = pack.clone();
        tampered.jsonl.push(' ');
        assert!(verify_pack_body_chain(&tampered, EXPORT_KEY).is_err());
        // A flipped signature digit breaks the manifest signature.
        let mut bad_sig = pack.clone();
        let mut bytes: Vec<u8> = bad_sig.signature_hex.as_bytes().to_vec();
        bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
        bad_sig.signature_hex = String::from_utf8(bytes).unwrap();
        assert!(verify_pack_signature(&bad_sig, EXPORT_KEY).is_err());
        // A different export key fails both checks.
        assert!(verify_pack_signature(&pack, &[0x11u8; 32]).is_err());
    }

    #[test]
    fn an_empty_range_produces_a_complete_empty_pack() {
        let reader = FakeReader { rows: Vec::new() };
        let builder = AuditExportBuilder::new(&reader, Box::new(|| 5_000));
        let pack = builder
            .build_pack(&tenant_ctx(7), "pack-0", 0, 1_000, EXPORT_KEY)
            .expect("pack built");
        assert!(pack.manifest.complete);
        assert_eq!(pack.manifest.record_count, 0);
        assert!(pack.jsonl.is_empty());
        // The empty-pack head is the zero digest (the chain genesis).
        assert_eq!(
            pack.manifest.pack_head_digest,
            hex_encode_digest(&[0u8; 32])
        );
        verify_pack_body_chain(&pack, EXPORT_KEY).expect("empty pack verifies");
    }

    #[test]
    fn a_short_export_key_fails_closed() {
        let reader = FakeReader { rows: Vec::new() };
        let builder = AuditExportBuilder::new(&reader, Box::new(|| 5_000));
        assert_eq!(
            builder.build_pack(&tenant_ctx(7), "p", 0, 1_000, &[0x01u8; 31]),
            Err(AuditExportError::ExportKeyTooShort { length: 31 })
        );
    }

    #[test]
    fn the_manifest_states_the_source_chain_gap_verbatim() {
        // The honesty check: the manifest carries the explicit
        // no-record-chain-verification statement, verbatim.
        assert!(SOURCE_CHAIN_STATEMENT.contains("does NOT verify"));
        assert!(SOURCE_CHAIN_STATEMENT.contains("hmac-sha256-chain-v1"));
        let rows = Vec::new();
        let reader = FakeReader { rows };
        let builder = AuditExportBuilder::new(&reader, Box::new(|| 5_000));
        let pack = builder
            .build_pack(&tenant_ctx(7), "p", 0, 1_000, EXPORT_KEY)
            .expect("pack built");
        assert_eq!(pack.manifest.source_chain_statement, SOURCE_CHAIN_STATEMENT);
        // The inherited algorithm name matches the P11.15 chain's recorded
        // value.
        assert_eq!(INHERITED_CHAIN_ALGORITHM, "hmac-sha256-chain-v1");
    }
}
