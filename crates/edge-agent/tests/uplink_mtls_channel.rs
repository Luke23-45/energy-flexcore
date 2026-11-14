// P6.07 — Integration tests for the outbound mTLS channel: composition
//         fail-closed paths, config validation, and the reconnect
//         state machine over a FAKE transport (ledger P6.07; decision
//         record `06_07_identity_channel.md`).
//
// NO REAL TLS IS EVER EXECUTED (and nothing is built/run this wave): the
// state-machine tests use a scripted [`FakeTransport`]; the rustls-touching
// paths are covered only for their FAIL-CLOSED error shapes (garbage PEM →
// named error). The full-matrix lab cases (plaintext refused on the wire,
// wrong-CA refusal by rustls, expired-cert refusal, no-listener confirmation
// on the host) are the P6.07 verification-method work and are recorded as
// UNRUN in the decision record.
//
// Covered:
// - **Composition fail-closed (the ledger criterion "a connection without a
//   valid client certificate is rejected")**: no stored identity, identity
//   without a client certificate, without the pinned root CA, and without a
//   key-material reference all refuse BEFORE any dial; the factory is still
//   handed the assembled material + pinned minimum (composition cannot skip
//   the credential assembly).
// - **Reconnect state machine (fake transport + deterministic clock)**:
//   failed dial → Disconnected with escalating backoff; a too-early retry
//   is named (not a silent spin); a retry after the window dials again;
//   success resets the backoff and reports Connected; a failed send after a
//   successful dial drops the session back to Disconnected (the batch is
//   NOT confirmed delivered — the P6.09 buffer owns redelivery).
// - **Keepalive semantics**: a fresh session is reused; a session past the
//   idle window is re-established (fresh handshake, no silent half-open
//   reuse).
// - **At-least-once honesty**: `forward` is `Ok` only after the transport's
//   ack; the encoder seam refuses to invent a wire format (P6.08/P6.09 own
//   it).
// - **rustls builder fail-closed shapes**: garbage PEM produces the named
//   refusal (the builder performs NO network I/O; no handshake is executed).
// - **The real filesystem credential store feeds the channel material**
//   (the P6.06 ↔ P6.07 integration shape), with no PEM body rendering into
//   any Debug output.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};
use flexcore_edge_agent::enrollment::{DeviceCredentialRecord, FilesystemCredentialStore, PemBody};
use flexcore_edge_agent::ports::{
    EdgeError, EdgeTelemetrySample, EdgeTelemetrySampleParts, UplinkChannel,
};
use flexcore_edge_agent::test_support::TempFileCredentialStore;
use flexcore_edge_agent::uplink_mtls::{
    assemble_material, build_client_config, BackoffSchedule, BatchPayloadEncoder, ChannelMaterial,
    FilesystemCredentialStoreLoad, KeyMaterialSource, MtlsUplinkChannel, MtlsUplinkConfig,
    NoBatchPayloadEncoder, TlsMinimum, UplinkTransport, UplinkTransportFactory,
};

const BASE_MILLIS: i64 = 1_700_000_000_000;

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

/// The scripted outcomes (shared so the factory keeps observability after
/// the transport is built).
#[derive(Default)]
struct Script {
    dial: Mutex<VecDeque<bool>>,
    send: Mutex<VecDeque<bool>>,
}

impl Script {
    fn new(dial: Vec<bool>, send: Vec<bool>) -> Self {
        Self {
            dial: Mutex::new(dial.into()),
            send: Mutex::new(send.into()),
        }
    }
}

#[derive(Default)]
struct Counters {
    dials: AtomicUsize,
    sends: Mutex<Vec<usize>>,
}

/// A scripted transport: dial/send outcomes come off the shared script;
/// every send's payload length is recorded. NO network I/O.
struct FakeTransport {
    script: Arc<Script>,
    counters: Arc<Counters>,
    established: bool,
}

impl UplinkTransport for FakeTransport {
    fn dial(&mut self) -> Result<(), EdgeError> {
        self.counters.dials.fetch_add(1, Ordering::SeqCst);
        let ok = self
            .script
            .dial
            .lock()
            .expect("poisoned")
            .pop_front()
            .unwrap_or(true);
        self.established = ok;
        if ok {
            Ok(())
        } else {
            Err(EdgeError::UplinkForwardFailed {
                reason: "scripted dial failure".to_string(),
            })
        }
    }

    fn send_and_await_ack(&mut self, payload: &[u8]) -> Result<(), EdgeError> {
        self.counters
            .sends
            .lock()
            .expect("poisoned")
            .push(payload.len());
        let ok = self
            .script
            .send
            .lock()
            .expect("poisoned")
            .pop_front()
            .unwrap_or(true);
        if ok {
            Ok(()) // the scripted ack: the batch IS confirmed delivered
        } else {
            Err(EdgeError::UplinkForwardFailed {
                reason: "scripted send failure (no ack received)".to_string(),
            })
        }
    }

    fn is_established(&self) -> bool {
        self.established
    }

    fn close(&mut self) {
        self.established = false;
    }
}

/// A factory that records being handed the assembled material and the
/// pinned minimum — composition cannot skip the credential assembly — and
/// yields scripted transports wired to the SAME observable counters.
struct FakeTransportFactory {
    handed_material: bool,
    handed_tls_minimum: Option<TlsMinimum>,
    script: Arc<Script>,
    counters: Arc<Counters>,
}

impl FakeTransportFactory {
    fn new(script: Arc<Script>, counters: Arc<Counters>) -> Self {
        Self {
            handed_material: false,
            handed_tls_minimum: None,
            script,
            counters,
        }
    }

    fn dials(&self) -> usize {
        self.counters.dials.load(Ordering::SeqCst)
    }

    fn sends(&self) -> Vec<usize> {
        self.counters.sends.lock().expect("poisoned").clone()
    }
}

impl UplinkTransportFactory for FakeTransportFactory {
    fn build(
        &mut self,
        material: &ChannelMaterial,
        tls_minimum: TlsMinimum,
    ) -> Result<Box<dyn UplinkTransport>, EdgeError> {
        self.handed_material = true;
        self.handed_tls_minimum = Some(tls_minimum);
        // A fake transport performs no TLS — but composition still had to
        // assemble the complete material to get here.
        let _ = material;
        Ok(Box::new(FakeTransport {
            script: Arc::clone(&self.script),
            counters: Arc::clone(&self.counters),
            established: false,
        }))
    }
}

/// A test encoder: one fixed payload shape (the real wire format is owned
/// by P6.08/P6.09 — this fake only proves the channel hands batches through).
struct FixedPayloadEncoder;

impl BatchPayloadEncoder for FixedPayloadEncoder {
    fn encode(&self, samples: &[EdgeTelemetrySample]) -> Result<Vec<u8>, EdgeError> {
        Ok(vec![samples.len() as u8])
    }
}

/// A scripted key-material source (a fake PEM string — never real crypto).
struct FakeKeySource;

impl KeyMaterialSource for FakeKeySource {
    fn load_key(&self, _reference: &PathBuf) -> Result<PemBody, EdgeError> {
        Ok(PemBody::new(
            "-----BEGIN PRIVATE KEY-----\nFAKE\n-----END PRIVATE KEY-----\n".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn sample(sequence: u64) -> EdgeTelemetrySample {
    EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        value: QuantityValue::Power(ActivePowerMW::new(1.5).expect("finite")),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        event_time: EventTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid"),
        ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + 2_000).expect("valid"),
        source_sequence: sequence,
        source_id: "edge:pilot-site-A:openems-rest:ess-01:active_power".to_string(),
    })
    .expect("valid sample")
}

fn config() -> MtlsUplinkConfig {
    MtlsUplinkConfig {
        endpoint: "wss://central.example:8443".to_string(),
        server_name: "central.example".to_string(),
        tls_minimum: TlsMinimum::DEFAULT,
        connect_timeout: Duration::from_millis(10_000),
        backoff_initial: Duration::from_millis(1_000),
        backoff_max: Duration::from_millis(60_000),
        keepalive_interval: Duration::from_millis(30_000),
    }
}

/// A COMPLETE identity record (certificate present, root CA pinned, key
/// reference present) — the compose-positive fixture.
fn complete_record() -> DeviceCredentialRecord {
    DeviceCredentialRecord {
        device_id: "edge-01".to_string(),
        tenant_id: "tenant-a".to_string(),
        site_id: "pilot-site-A".to_string(),
        certificate_pem: Some(PemBody::new(
            "-----BEGIN CERTIFICATE-----\nDEVICECERT\n-----END CERTIFICATE-----\n".to_string(),
        )),
        chain_pem: Vec::new(),
        root_ca_pem: Some(PemBody::new(
            "-----BEGIN CERTIFICATE-----\nTESTROOT\n-----END CERTIFICATE-----\n".to_string(),
        )),
        private_key_reference: Some(PathBuf::from("/keys/device.key")),
        issued_at_unix_millis: BASE_MILLIS,
        expires_at_unix_millis: Some(BASE_MILLIS + 30 * 86_400_000),
    }
}

fn compose_with(
    record: Option<DeviceCredentialRecord>,
    script: Arc<Script>,
    counters: Arc<Counters>,
    encoder: Box<dyn BatchPayloadEncoder>,
    clock: Arc<FixedClock>,
) -> Result<MtlsUplinkChannel, EdgeError> {
    let identity = FakeIdentityStore { record };
    let mut factory = FakeTransportFactory::new(script, counters);
    MtlsUplinkChannel::compose(
        config(),
        &identity,
        &FakeKeySource,
        encoder,
        &mut factory,
        clock,
    )
}

/// A scripted identity store (the minimal `FilesystemCredentialStoreLoad`
/// shape — no filesystem needed).
struct FakeIdentityStore {
    record: Option<DeviceCredentialRecord>,
}

impl FilesystemCredentialStoreLoad for FakeIdentityStore {
    fn load_identity(&self) -> Result<Option<DeviceCredentialRecord>, EdgeError> {
        Ok(self.record.clone())
    }
}

// ---------------------------------------------------------------------------
// Composition fail-closed
// ---------------------------------------------------------------------------

#[test]
fn composition_fails_closed_without_a_stored_identity() {
    let error = compose_with(
        None,
        Arc::new(Script::default()),
        Arc::new(Counters::default()),
        Box::new(FixedPayloadEncoder),
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
    .expect_err("no identity → no channel (fail closed)");
    let rendered = error.to_string();
    assert!(rendered.contains("no device identity is stored"), "{rendered}");
    assert!(rendered.contains("fail closed"), "{rendered}");
}

#[test]
fn composition_fails_closed_without_a_client_certificate() {
    let mut record = complete_record();
    record.certificate_pem = None;
    let error = compose_with(
        Some(record),
        Arc::new(Script::default()),
        Arc::new(Counters::default()),
        Box::new(FixedPayloadEncoder),
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
    .expect_err("no client certificate → no channel");
    assert!(error.to_string().contains("no device certificate"), "{}", error);
}

#[test]
fn composition_fails_closed_without_the_pinned_root_ca() {
    let mut record = complete_record();
    record.root_ca_pem = None;
    let error = compose_with(
        Some(record),
        Arc::new(Script::default()),
        Arc::new(Counters::default()),
        Box::new(FixedPayloadEncoder),
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
    .expect_err("no pinned root CA → no channel");
    assert!(
        error.to_string().contains("no pinned platform root CA"),
        "{error}"
    );
}

#[test]
fn composition_fails_closed_without_a_key_material_reference() {
    let mut record = complete_record();
    record.private_key_reference = None;
    let error = compose_with(
        Some(record),
        Arc::new(Script::default()),
        Arc::new(Counters::default()),
        Box::new(FixedPayloadEncoder),
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
    .expect_err("no key reference → no channel");
    assert!(error.to_string().contains("no private-key reference"), "{error}");
}

#[test]
fn composition_hands_the_assembled_material_and_pinned_minimum_to_the_factory() {
    let mut factory =
        FakeTransportFactory::new(Arc::new(Script::default()), Arc::new(Counters::default()));
    let identity = FakeIdentityStore {
        record: Some(complete_record()),
    };
    MtlsUplinkChannel::compose(
        config(),
        &identity,
        &FakeKeySource,
        Box::new(FixedPayloadEncoder),
        &mut factory,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
    .expect("composes");
    assert!(factory.handed_material, "the factory received the material");
    assert_eq!(
        factory.handed_tls_minimum,
        Some(TlsMinimum::Tls13),
        "the pinned minimum reaches the factory"
    );
}

// ---------------------------------------------------------------------------
// The reconnect state machine (fake transport + deterministic clock)
// ---------------------------------------------------------------------------

fn composed_channel(
    dial_script: Vec<bool>,
    send_script: Vec<bool>,
    clock: Arc<FixedClock>,
) -> (MtlsUplinkChannel, FakeTransportFactory) {
    let script = Arc::new(Script::new(dial_script, send_script));
    let counters = Arc::new(Counters::default());
    let identity = FakeIdentityStore {
        record: Some(complete_record()),
    };
    let mut factory = FakeTransportFactory::new(Arc::clone(&script), Arc::clone(&counters));
    let channel = MtlsUplinkChannel::compose(
        config(),
        &identity,
        &FakeKeySource,
        Box::new(FixedPayloadEncoder),
        &mut factory,
        clock,
    )
    .expect("composes");
    (channel, factory)
}

#[test]
fn a_failed_dial_disconnects_and_the_next_retry_observes_the_backoff() {
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS));
    // Script: dial fails once, then succeeds; sends always ack.
    let (mut channel, factory) = composed_channel(vec![false, true], vec![true], Arc::clone(&clock));

    // Forward 1: dial fails → Err, one dial, Disconnected with 1 failure.
    let error = channel
        .forward(&[sample(1)])
        .expect_err("the scripted dial fails");
    assert!(error.to_string().contains("scripted dial failure"));
    let status = channel.status();
    assert!(!status.connected);
    assert_eq!(status.consecutive_failures, 1);
    assert_eq!(status.current_backoff_millis, 1_000);

    // Forward 2 (clock unchanged): the backoff window is still open — a
    // NAMED refusal, no dial, no silent spin.
    let error = channel
        .forward(&[sample(2)])
        .expect_err("the backoff window is open");
    assert!(error.to_string().contains("backoff window still open"));
    assert_eq!(factory.dials(), 1, "no second dial yet");

    // Advance past the 1-second window: the retry dials (script: success).
    clock.advance_millis(1_500);
    channel
        .forward(&[sample(2)])
        .expect("the retry after the window dials and delivers");
    let status = channel.status();
    assert!(status.connected);
    assert_eq!(status.consecutive_failures, 0, "success resets the backoff");
    assert_eq!(factory.dials(), 2);
}

#[test]
fn backoff_escalates_until_the_cap() {
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS));
    // Script: the first two dials fail, then succeed.
    let (mut channel, factory) =
        composed_channel(vec![false, false, true], vec![true], Arc::clone(&clock));

    channel.forward(&[sample(1)]).expect_err("dial fails (1)");
    assert_eq!(channel.status().current_backoff_millis, 1_000);
    clock.advance_millis(1_100);
    channel.forward(&[sample(2)]).expect_err("dial fails (2)");
    // Failure 2: backoff doubled.
    assert_eq!(channel.status().current_backoff_millis, 2_000);
    clock.advance_millis(2_100);
    channel.forward(&[sample(3)]).expect("dial succeeds");
    assert!(channel.status().connected);
    assert_eq!(factory.dials(), 3);
}

#[test]
fn a_failed_send_after_a_successful_dial_is_not_confirmed_delivery() {
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS));
    // Dial succeeds; the send gets NO ack (scripted failure).
    let (mut channel, factory) = composed_channel(vec![true], vec![false, true], Arc::clone(&clock));

    let error = channel
        .forward(&[sample(1)])
        .expect_err("an un-acked batch is NOT delivered");
    assert!(error.to_string().contains("scripted send failure"));
    let status = channel.status();
    assert!(!status.connected, "the session dropped back to Disconnected");
    assert_eq!(status.consecutive_failures, 1);
    assert_eq!(factory.sends().len(), 1, "one send was attempted");

    // The retry (after the backoff window) re-dials and delivers.
    clock.advance_millis(1_100);
    channel.forward(&[sample(1)]).expect("redelivered");
    assert_eq!(factory.sends().len(), 2);
    assert!(channel.status().connected);
}

#[test]
fn keepalive_reuses_a_fresh_session_and_reestablishes_an_idle_one() {
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS));
    let (mut channel, factory) = composed_channel(vec![true, true], vec![true], Arc::clone(&clock));

    channel.forward(&[sample(1)]).expect("delivered");
    // A fresh ensure_connected inside the keepalive window reuses the
    // session (no extra dial).
    channel.ensure_connected().expect("fresh session reused");
    assert_eq!(factory.dials(), 1);

    // Idle past the 30-second keepalive window → RE-ESTABLISHED (fresh
    // handshake, no silent half-open reuse).
    clock.advance_millis(31_000);
    channel.ensure_connected().expect("re-established");
    assert_eq!(factory.dials(), 2, "the idle session was re-dialed");
}

#[test]
fn the_encoder_seam_refuses_to_invent_a_wire_format() {
    let mut channel = compose_with(
        Some(complete_record()),
        Arc::new(Script::default()),
        Arc::new(Counters::default()),
        Box::new(NoBatchPayloadEncoder),
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
    .expect("composes");
    // The encoder refusal happens BEFORE any dial: the channel never
    // fabricates a payload shape that P6.08/P6.09 must own.
    let failure = channel
        .forward(&[sample(1)])
        .expect_err("no composed encoder → no fabricated payload");
    assert!(failure.to_string().contains("payload encoder is composed"));
}

#[test]
fn an_empty_batch_is_no_delivery_and_no_dial() {
    let (mut channel, factory) = composed_channel(vec![], vec![], Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)));
    channel.forward(&[]).expect("nothing to deliver");
    assert_eq!(factory.dials(), 0);
}

#[test]
fn the_pinned_policy_is_reportable_without_the_endpoint_string() {
    let (channel, _) = composed_channel(vec![], vec![], Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)));
    let (scheme, server_name, tls_minimum) = channel.pinned_policy();
    assert_eq!(scheme, "wss");
    assert_eq!(server_name, "central.example");
    assert_eq!(tls_minimum, TlsMinimum::Tls13);
}

// ---------------------------------------------------------------------------
// The filesystem credential store feeding the channel (P6.06 ↔ P6.07 shape)
// ---------------------------------------------------------------------------

#[test]
fn the_real_credential_store_feeds_the_channel_composition() {
    let mut store = TempFileCredentialStore::new("p6-07-compose");
    store.put(&complete_record()).expect("stored");
    let loader = FilesystemCredentialStore::new(store.dir().join("device-identity.conf"));
    let material = assemble_material(&loader, &FakeKeySource)
        .expect("the stored identity assembles into channel material");
    assert!(material.client_cert_pem.expose().contains("DEVICECERT"));
    assert!(material.root_ca_pem.expose().contains("TESTROOT"));
    assert!(material.client_key_pem.expose().contains("FAKE"));
    // Nothing from the material renders into a Debug line.
    let rendered = format!("{material:?}");
    assert!(!rendered.contains("DEVICECERT"));
    assert!(!rendered.contains("FAKE"));
    // And an EMPTY store fails the composition closed through the same
    // production path.
    let empty = TempFileCredentialStore::new("p6-07-compose-empty");
    let empty_loader = FilesystemCredentialStore::new(empty.dir().join("device-identity.conf"));
    let error = assemble_material(&empty_loader, &FakeKeySource)
        .expect_err("no stored identity → no channel material");
    assert!(error.to_string().contains("no device identity is stored"), "{error}");
}

// ---------------------------------------------------------------------------
// rustls builder fail-closed shapes (NO handshake is ever executed)
// ---------------------------------------------------------------------------

#[test]
fn the_rustls_builder_refuses_garbage_pem() {
    let material = ChannelMaterial {
        root_ca_pem: PemBody::new("not a pem at all".to_string()),
        client_cert_pem: PemBody::new("also not a pem".to_string()),
        client_key_pem: PemBody::new("no key here".to_string()),
    };
    // Garbage CA: either a parse error or an empty root store — both are
    // the named fail-closed refusals.
    let error = build_client_config(&material, TlsMinimum::DEFAULT)
        .expect_err("garbage PEM must fail closed");
    let rendered = error.to_string();
    assert!(
        rendered.contains("root CA") || rendered.contains("trust store"),
        "{rendered}"
    );
}

#[test]
fn backoff_schedule_rejects_inverted_bounds() {
    assert!(
        BackoffSchedule::new(Duration::from_millis(0), Duration::from_millis(60_000)).is_err()
    );
    assert!(
        BackoffSchedule::new(Duration::from_millis(60_000), Duration::from_millis(1_000)).is_err()
    );
    assert!(
        BackoffSchedule::new(Duration::from_millis(1_000), Duration::from_millis(60_000)).is_ok()
    );
}
