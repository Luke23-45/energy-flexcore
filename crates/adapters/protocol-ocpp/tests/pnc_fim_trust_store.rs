//! FIM-F13.01/FIM-F13.05 — WRITTEN-UNRUN integration tests for the PnC V2G
//! Root CA trust store: the shipped-empty refusal discipline, explicit
//! add/remove management, byte-equality lookup, and the validity-window
//! fail-closed rule (record 41 §3; the FIM hard boundary: these tests are
//! written, their execution is a HUMAN GATE).

use flexcore_ocpp_adapter::pnc_trust_store::{
    PncTrustStoreError, TrustAnchorRecord, V2gRootCaTrustStore, MAX_ANCHOR_CERTIFICATE_BYTES,
};

const BASE: i64 = 1_700_000_000_000;

/// The test-grade anchor certificate: hand-built bytes, NOT a real
/// certificate (no CA is stood up; no certificate exists).
const ANCHOR_ROOT_DER: &[u8] = &[0x30, 0x03, 0x02, 0x01, 0x07];
const OTHER_DER: &[u8] = &[0x30, 0x03, 0x02, 0x01, 0x09];

fn anchor(label: &str, der: &[u8]) -> TrustAnchorRecord {
    TrustAnchorRecord::new(
        label.to_string(),
        der.to_vec(),
        "v2g-pki-operator-contract-1".to_string(),
        BASE - 1_000,
        BASE + 100_000,
        BASE,
    )
    .expect("valid anchor record")
}

#[test]
fn the_store_ships_empty_and_every_anchor_operation_refuses_by_name() {
    let store = V2gRootCaTrustStore::empty();
    assert_eq!(store.anchor_count(), 0, "anchors are injected, never bundled");
    assert_eq!(
        store.require_any_anchor().unwrap_err().name(),
        "PNC_TRUST_STORE_EMPTY"
    );
    assert_eq!(
        store.require_anchor("v2g-root-1", BASE).unwrap_err().name(),
        "PNC_TRUST_STORE_EMPTY"
    );
    assert_eq!(
        store
            .find_anchor_by_certificate(ANCHOR_ROOT_DER, BASE)
            .unwrap_err()
            .name(),
        "PNC_TRUST_STORE_EMPTY"
    );
    assert!(store.anchor("v2g-root-1").is_none());
}

#[test]
fn anchors_enter_and_leave_only_through_explicit_calls() {
    let mut store = V2gRootCaTrustStore::empty();
    store
        .add_anchor(anchor("v2g-root-1", ANCHOR_ROOT_DER))
        .expect("added through the explicit call");
    store
        .add_anchor(anchor("mo-sub-ca-1", OTHER_DER))
        .expect("second anchor added");
    assert_eq!(store.anchor_count(), 2);

    // A duplicate label is refused, never overwritten.
    let duplicate =
        store.add_anchor(anchor("v2g-root-1", b"\x30\x03\x02\x01\x0a".as_slice()));
    assert_eq!(duplicate.unwrap_err().name(), "PNC_TRUST_ANCHOR_DUPLICATE");
    assert_eq!(store.anchor_count(), 2);

    // Byte-equality lookup by certificate DER.
    let found = store
        .find_anchor_by_certificate(ANCHOR_ROOT_DER, BASE)
        .expect("byte-equal anchor found");
    assert_eq!(found.anchor_label, "v2g-root-1");
    assert_eq!(found.signer_reference, "v2g-pki-operator-contract-1");
    // A certificate no anchor byte-equals is refused.
    assert_eq!(
        store
            .find_anchor_by_certificate(b"\x30\x03\x02\x01\x0b", BASE)
            .unwrap_err()
            .name(),
        "PNC_TRUST_CERTIFICATE_NOT_ANCHORED"
    );

    // Explicit remove; removing an absent anchor is a named refusal.
    store.remove_anchor("mo-sub-ca-1").expect("removed");
    assert_eq!(
        store.remove_anchor("mo-sub-ca-1").unwrap_err().name(),
        "PNC_TRUST_ANCHOR_MISSING"
    );
    assert_eq!(store.anchor_count(), 1);
}

#[test]
fn expired_anchors_refuse_and_incomplete_records_never_enter() {
    let mut store = V2gRootCaTrustStore::empty();
    store
        .add_anchor(anchor("v2g-root-1", ANCHOR_ROOT_DER))
        .expect("added");
    // Outside the declared validity window at BOTH ends: refuse by name.
    assert_eq!(
        store
            .find_anchor_by_certificate(ANCHOR_ROOT_DER, BASE + 200_000)
            .unwrap_err()
            .name(),
        "PNC_TRUST_ANCHOR_OUTSIDE_VALIDITY"
    );
    assert_eq!(
        store
            .require_anchor("v2g-root-1", BASE - 5_000)
            .unwrap_err()
            .name(),
        "PNC_TRUST_ANCHOR_OUTSIDE_VALIDITY"
    );
    // The exact window boundaries are inside (inclusive).
    store
        .find_anchor_by_certificate(ANCHOR_ROOT_DER, BASE + 100_000)
        .expect("not-after is inclusive");

    // Incomplete records are refused at construction.
    for record in [
        TrustAnchorRecord::new("  ".to_string(), vec![1], "ref".to_string(), 0, 1, 1),
        TrustAnchorRecord::new("l".to_string(), Vec::new(), "ref".to_string(), 0, 1, 1),
        TrustAnchorRecord::new("l".to_string(), vec![1], " ".to_string(), 0, 1, 1),
        TrustAnchorRecord::new("l".to_string(), vec![1], "ref".to_string(), 10, 5, 1),
        TrustAnchorRecord::new(
            "l".to_string(),
            vec![0u8; MAX_ANCHOR_CERTIFICATE_BYTES + 1],
            "ref".to_string(),
            0,
            1,
            1,
        ),
    ] {
        assert!(record.is_err(), "an incomplete record must be refused");
    }
    let _ = PncTrustStoreError::StoreEmpty; // the named error type is public
}
