// P6.05 — `edge-agent`: the edge agent binary (composition root; report
// §18.1, §8.2; ledger P6.05).
//
// Composition (P6.10 state, honestly recorded — the wave plan is in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md`; the P6.10 decision record
// is `ledger/phase-06-edge-protocol/10_11_openems.md`):
//
// ```text
// EdgeConfig (fail-fast env parse; command path DEFAULT FALSE; P6.10 adds
//   the OpenEMS base-URL + versioned-mapping-profile config points)
//   → tracing
//   → adapter registry check (P6.12 allowlist; since P6.10 the DEFAULT
//     table carries `openems-rest` as AVAILABLE — the explicit
//     `with_openems_available()` call is retained as an idempotent
//     re-registration documenting the composition's intent)
//   → config-selected EMS read path (P6.10 step 6):
//       `openems-rest`   → OpenEmsRestAdapter (EdgeConfig discovery over a
//                           read-only HTTP/1.1 GET client; versioned mapping
//                           profile; unmapped/ambiguous register) → ACL →
//                           AdapterIngestSource
//       `simulated-test` → SimulatedEmsSource (TEST/simulation-ONLY, loud
//                           warning) → ACL → AdapterIngestSource
//       anything else    → abort (no silent fallback)
//   → EdgeRunner composed over:
//       EmsSource     = AdapterIngestSource (the real read path, above)
//       UplinkChannel = FailClosedUplink    (the P6.07 channel exists, but
//                          the wire-payload encoder seam REFUSES encoding —
//                          fail-closed until the wire-format owner lands the
//                          encoder; NO wire format is invented here)
//       LocalBuffer   = FileLocalBuffer     (P6.09 durable journal; bounded,
//                          audited drop-oldest, age retention; encryption at
//                          rest §18.3 is a recorded open item)
//       AuditSink     = InMemoryAuditSink   (durable/offline audit is P6.18)
//   → supervision loop (poll → buffer → forward, degraded-mode safe)
//   → signal-driven graceful shutdown
// ```
//
// Startup is fail-fast: configuration errors abort before the loop starts
// (exit 2); a non-allowlisted adapter id aborts startup (exit 1); a failed
// adapter composition, discovery, ACL table, or buffer open aborts startup
// — a half-composed edge never runs. No inbound listener is opened (§3.3:
// outbound-only; the command path does not exist in this wave, P6.14
// implements the disabled-by-default receiver behind its gate).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_edge_agent::adapters::acl::{
    AntiCorruptionLayer, NativeSign, NativeUnit, PointMapping,
};
use flexcore_edge_agent::adapters::openems::{
    ChannelMappingStatus, OpenEmsRestAdapter, OpenemsMappingProfile, TcpHttpGetTransport,
    OPENEMS_REST_ADAPTER_ID,
};
use flexcore_edge_agent::adapters::port::{EmsAdapter, PointKey};
use flexcore_edge_agent::adapters::registry::AdapterRegistry;
use flexcore_edge_agent::adapters::simulated::{
    SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture, SIMULATED_ADAPTER_ID,
};
use flexcore_edge_agent::buffer_file::FileLocalBuffer;
use flexcore_edge_agent::clock::WallClock;
use flexcore_edge_agent::config::{EdgeConfig, ENV_OPENEMS_BASE_URL};
use flexcore_edge_agent::ingest::AdapterIngestSource;
use flexcore_edge_agent::ports::{FailClosedUplink, InMemoryAuditSink};
use flexcore_edge_agent::runner::{EdgeRunner, RunnerParams};
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_ports::clock::Clock;
use flexcore_ports::telemetry_source::TelemetryQuantity;
use tokio::sync::watch;

/// P6.10 (recorded DEFAULT): the OpenEMS REST transport timeouts. The poll
/// cadence is supervisory (§3.5, minutes-scale); these are composition
/// constants this wave — promoting them to per-site configuration is a
/// reviewed follow-up (decision record §6).
const OPENEMS_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OPENEMS_READ_TIMEOUT: Duration = Duration::from_secs(10);

fn main() {
    // Fail fast: parse the configuration before anything else.
    let config = match EdgeConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("edge-agent: configuration error: {error}");
            std::process::exit(2);
        }
    };

    // Structured tracing at the configured level (the worker composition
    // pattern; the plain parsed level is passed to EnvFilter, no free-form
    // filter expression).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .parse_lossy(config.log_level.as_str()),
        )
        .init();
    tracing::info!(
        site_id = %config.site_id,
        tenant_id = %config.tenant_id,
        adapter_id = %config.adapter_id,
        command_path_enabled = config.command_path_enabled,
        "edge-agent starting (read-only/test-lab default, v0.4.0; outbound-only)"
    );

    // The command-path flag is RECORDED, never acted on: no command
    // receiver exists in this wave (P6.14 implements the disabled-by-
    // default receiver and its written enablement gate). A `true` value is
    // logged loudly as the inert state it is.
    if config.command_path_enabled {
        tracing::warn!(
            "FLEXCORE_EDGE_COMMAND_PATH_ENABLED=true is recorded but INERT: no command receiver \
             exists in this wave (P6.14 owns the disabled-by-default implementation and the \
             test-env enablement gate); nothing receives, verifies, or forwards commands"
        );
    }

    // Adapter registry (P6.12): the configured id must be allowlisted.
    // Since P6.10 the DEFAULT table already carries `openems-rest` as
    // AVAILABLE; `with_openems_available()` is retained as an explicit,
    // idempotent statement of the composition's intent — the concrete
    // EdgeConfig-discovery/read adapter constructs through the registry
    // below.
    let registry = AdapterRegistry::with_default_allowlist().with_openems_available();
    match registry.status_of(&config.adapter_id) {
        Err(error) => {
            tracing::error!(
                error = %error,
                "the configured adapter id is not allowlisted; aborting startup"
            );
            std::process::exit(1);
        }
        Ok(status) => {
            tracing::info!(
                adapter_id = %config.adapter_id,
                status = %status,
                "adapter id allowlisted (the read-path composition wires the concrete adapter below)"
            );
        }
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("edge-agent: could not start the async runtime: {error}");
            std::process::exit(1);
        }
    };
    let exit_code = runtime.block_on(run(config));
    std::process::exit(exit_code);
}

/// P6.10 step 6 — composes the config-selected EMS read path behind the
/// [`flexcore_edge_agent::ports::EmsSource`] port. Fail-fast: every
/// composition failure aborts startup; a half-composed edge never runs
/// and nothing is fabricated in its place.
fn compose_ems_source(config: &EdgeConfig) -> Box<dyn flexcore_edge_agent::ports::EmsSource> {
    let registry = AdapterRegistry::with_default_allowlist().with_openems_available();
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(WallClock);
    match config.adapter_id.as_str() {
        OPENEMS_REST_ADAPTER_ID => compose_openems(config, &registry, clock),
        SIMULATED_ADAPTER_ID => compose_simulated(config, &registry, clock),
        other => {
            tracing::error!(
                adapter_id = %other,
                "the configured adapter id has no composition in this wave; aborting startup \
                 (there is no fallback source — an unconfigured edge degrades loudly at \
                 startup, never passes silently)"
            );
            std::process::exit(1);
        }
    }
}

/// Composes the `openems-rest` read path: transport + versioned mapping
/// profile → registry-constructed adapter → EdgeConfig discovery → ACL
/// mapping table → [`AdapterIngestSource`]. The registry flip (AVAILABLE)
/// is the actual construction path — the factory inside
/// [`AdapterRegistry::construct`] builds AND discovers the concrete
/// adapter and hands the validated mapping table out through a slot.
fn compose_openems(
    config: &EdgeConfig,
    registry: &AdapterRegistry,
    clock: Arc<dyn Clock + Send + Sync>,
) -> Box<dyn flexcore_edge_agent::ports::EmsSource> {
    // Fail-closed: no default URL is invented. The value is never logged
    // (it may embed credentials — the config redaction rule).
    let base_url = config.openems_base_url.clone().unwrap_or_else(|| {
        tracing::error!(
            env = %ENV_OPENEMS_BASE_URL,
            "adapter 'openems-rest' requires an OpenEMS REST base URL (fail-closed: no \
             default URL is invented); aborting startup"
        );
        std::process::exit(2);
    });
    // The VERSIONED mapping profile (P6.10 step 4): a missing or refused
    // profile aborts startup — the adapter never runs unmapped.
    let profile = OpenemsMappingProfile::from_path(&config.openems_mapping_profile_path)
        .unwrap_or_else(|error| {
            tracing::error!(
                error = %error,
                path = %config.openems_mapping_profile_path.display(),
                "the versioned OpenEMS mapping profile could not be loaded (fail-closed: \
                 the adapter never runs without its mapping table); aborting startup"
            );
            std::process::exit(2);
        });
    // The profile's recorded UNVERIFIED-ASSUMPTION register travels with
    // the profile and is announced at startup (never silently dropped).
    for assumption in profile.assumptions() {
        tracing::warn!(
            assumption = %assumption,
            "mapping-profile recorded assumption (UNVERIFIED until the P6.11 lab run)"
        );
    }
    let transport = TcpHttpGetTransport::from_base_url(
        &base_url,
        OPENEMS_CONNECT_TIMEOUT,
        OPENEMS_READ_TIMEOUT,
    )
    .unwrap_or_else(|error| {
        tracing::error!(
            error = %error,
            "the OpenEMS REST transport could not be composed; aborting startup"
        );
        std::process::exit(2);
    });

    // The registry is the construction path: `openems-rest` is AVAILABLE
    // (P6.10), so the factory below runs ONCE, builds the adapter,
    // performs EdgeConfig discovery, and hands the validated mapping table
    // out through the slots for the ACL + ingest composition.
    let prepared: Arc<Mutex<Option<(TcpHttpGetTransport, OpenemsMappingProfile)>>> =
        Arc::new(Mutex::new(Some((transport, profile))));
    let mappings_slot: Arc<Mutex<Vec<PointMapping>>> = Arc::new(Mutex::new(Vec::new()));
    let unmapped_slot: Arc<Mutex<Vec<PointKey>>> = Arc::new(Mutex::new(Vec::new()));
    let prepared_for_factory = Arc::clone(&prepared);
    let mappings_for_factory = Arc::clone(&mappings_slot);
    let unmapped_for_factory = Arc::clone(&unmapped_slot);
    let clock_for_factory = Arc::clone(&clock);
    let adapter = registry
        .construct(
            OPENEMS_REST_ADAPTER_ID,
            &move || {
                let Some((transport, profile)) =
                    prepared_for_factory.lock().expect("prepared slot").take()
                else {
                    return Err(
                        "the prepared OpenEMS composition was already consumed".to_string(),
                    );
                };
                let mut concrete = OpenEmsRestAdapter::new(
                    Box::new(transport),
                    profile,
                    clock_for_factory,
                )
                .map_err(|error| format!("the OpenEMS adapter could not be composed: {error}"))?;
                let descriptors = concrete
                    .discover()
                    .map_err(|error| format!("EdgeConfig discovery failed: {error}"))?;
                let register = concrete.mapping_register();
                let mapped = register
                    .iter()
                    .filter(|entry| matches!(entry.status, ChannelMappingStatus::Mapped))
                    .count();
                let ambiguous = register
                    .iter()
                    .filter(|entry| matches!(entry.status, ChannelMappingStatus::Ambiguous { .. }))
                    .count();
                let unmapped = register.len() - mapped - ambiguous;
                *mappings_for_factory.lock().expect("mappings slot") =
                    concrete.mapped_point_mappings();
                *unmapped_for_factory.lock().expect("unmapped slot") =
                    concrete.declared_unmapped_points();
                tracing::info!(
                    discovered = descriptors.len(),
                    mapped,
                    ambiguous,
                    unmapped,
                    "EdgeConfig discovery succeeded: every channel is mapped, explicitly \
                     unmapped-with-reason, or flagged ambiguous — nothing is mapped by \
                     guessing"
                );
                Ok(Box::new(concrete) as Box<dyn EmsAdapter>)
            },
        )
        .unwrap_or_else(|error| {
            tracing::error!(
                error = %error,
                "the OpenEMS adapter could not be constructed through the registry; \
                 aborting startup"
            );
            std::process::exit(1);
        });

    let mappings = mappings_slot.lock().expect("mappings slot").clone();
    let declared_unmapped = unmapped_slot.lock().expect("unmapped slot").clone();
    let source_prefix = format!("edge:{}:{}", config.site_id, config.adapter_id);
    let acl = AntiCorruptionLayer::new(mappings.clone(), source_prefix, declared_unmapped)
        .unwrap_or_else(|error| {
            tracing::error!(
                error = %error,
                "the anti-corruption mapping table was refused; aborting startup"
            );
            std::process::exit(1);
        });
    let source = AdapterIngestSource::new(
        adapter,
        acl,
        mappings,
        clock,
        config.dedup_window,
    )
    .unwrap_or_else(|error| {
        tracing::error!(
            error = %error,
            "the ingest read path could not be composed; aborting startup"
        );
        std::process::exit(2);
    });
    Box::new(source)
}

/// Composes the `simulated-test` read path (TEST/simulation-ONLY — a loud,
/// deterministic lab fixture, never a real EMS; the id travels on every
/// sample's `source_id`).
fn compose_simulated(
    config: &EdgeConfig,
    registry: &AdapterRegistry,
    clock: Arc<dyn Clock + Send + Sync>,
) -> Box<dyn flexcore_edge_agent::ports::EmsSource> {
    tracing::warn!(
        "FLEXCORE_EDGE_ADAPTER_ID=simulated-test is TEST/simulation-ONLY: the composed \
         source is the deterministic scenario fixture (id 'simulated-test'); every sample \
         carries the id in its source_id and must NEVER be presented as real site data \
         (P6.25 will surface simulated values visually distinct)"
    );
    let fixture = SimulatedFixture {
        points: vec![
            SimulatedPointFixture {
                point: PointKey::new("ess0", "ActivePower"),
                native_unit: NativeUnit::Kw,
                values_per_tick: vec![1_500.0, 1_600.0],
                quality_per_tick: vec!["GOOD".to_string()],
            },
            SimulatedPointFixture {
                point: PointKey::new("ess0", "Soc"),
                native_unit: NativeUnit::SocPercent,
                values_per_tick: vec![87.0, 86.0],
                quality_per_tick: vec!["GOOD".to_string(), "OUT_OF_RANGE".to_string()],
            },
        ],
        start_millis: 1_700_000_000_000,
        step_millis: 1_000,
        reactive_source_convention: ReactiveSignConvention::InjectionPositive,
        cadence_millis: 5_000,
    };
    let mappings = vec![
        PointMapping {
            point: PointKey::new("ess0", "ActivePower"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            native_unit: NativeUnit::Kw,
            native_sign: Some(NativeSign::ExportPositive),
            reactive_source_convention: None,
        },
        PointMapping {
            point: PointKey::new("ess0", "Soc"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::Soc,
            native_unit: NativeUnit::SocPercent,
            native_sign: None,
            reactive_source_convention: None,
        },
    ];
    let sim_for_factory = SimulatedEmsSource::new(fixture);
    let adapter = registry
        .construct(SIMULATED_ADAPTER_ID, &|| {
            Ok(Box::new(sim_for_factory.clone()) as Box<dyn EmsAdapter>)
        })
        .unwrap_or_else(|error| {
            tracing::error!(
                error = %error,
                "the simulated-test adapter could not be constructed through the registry; \
                 aborting startup"
            );
            std::process::exit(1);
        });
    let source_prefix = format!("edge:{}:{}", config.site_id, config.adapter_id);
    let acl = AntiCorruptionLayer::new(mappings.clone(), source_prefix, Vec::new())
        .unwrap_or_else(|error| {
            tracing::error!(
                error = %error,
                "the anti-corruption mapping table was refused; aborting startup"
            );
            std::process::exit(1);
        });
    let source = AdapterIngestSource::new(adapter, acl, mappings, clock, config.dedup_window)
        .unwrap_or_else(|error| {
            tracing::error!(
                error = %error,
                "the ingest read path could not be composed; aborting startup"
            );
            std::process::exit(2);
        });
    Box::new(source)
}

/// The composed edge lifetime. Returns the process exit code.
async fn run(config: EdgeConfig) -> i32 {
    // --- Read path (P6.10): the config-selected adapter composition -------

    let source = compose_ems_source(&config);

    // --- Buffer (P6.09): the DURABLE store-and-forward buffer -------------

    let buffer_clock: Arc<dyn Clock + Send + Sync> = Arc::new(WallClock);
    let buffer = FileLocalBuffer::open(
        &config.buffer_path,
        config.buffer_capacity,
        Some(config.buffer_max_age),
        buffer_clock,
    )
    .unwrap_or_else(|error| {
        tracing::error!(
            error = %error,
            path = %config.buffer_path.display(),
            "the durable store-and-forward buffer could not be opened; aborting startup"
        );
        std::process::exit(1);
    });
    tracing::info!(
        buffer_path = %config.buffer_path.display(),
        capacity = config.buffer_capacity,
        "buffer is the DURABLE journal-backed store-and-forward buffer (P6.09: CRC-checked \
         records, truncate-and-audit recovery, audited drop-oldest overflow, age retention); \
         encryption at rest (§18.3) is a recorded open item"
    );

    // --- Audit + uplink -----------------------------------------------------

    // Audit: the offline/durable audit buffer is P6.18.
    let audit: Arc<dyn flexcore_edge_agent::ports::AuditSink> = Arc::new(InMemoryAuditSink::new());

    // Uplink: the P6.07 mTLS channel exists, but its wire-payload encoder
    // seam REFUSES encoding (fail-closed by absence) — the composition
    // keeps the fail-closed uplink and states exactly what remains for the
    // wire-format owner. NO wire format is invented here.
    let uplink = Box::new(FailClosedUplink);
    tracing::warn!(
        "uplink stays fail-closed this wave: the P6.07 channel's BatchPayloadEncoder seam \
         refuses encoding, so nothing is forwarded and everything buffers (at-least-once). \
         What remains for the wire-format owner: (1) implement BatchPayloadEncoder owning \
         the telemetry wire format, (2) compose the P6.06 credential store + key source \
         into MtlsUplinkChannel::compose with that encoder — the read path (P6.10) and the \
         durable buffer (P6.09) are real and wait on nothing else"
    );

    // --- Supervision loop -------------------------------------------------

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let runner = EdgeRunner::new(
        RunnerParams::from_config(
            config.poll_interval,
            config.forward_batch_size,
            config.buffer_capacity,
        ),
        source,
        uplink,
        Box::new(buffer),
        audit,
        Arc::new(WallClock),
    );
    tracing::info!(
        poll_interval_ms = config.poll_interval.as_millis() as u64,
        forward_batch_size = config.forward_batch_size,
        "edge supervision loop starting (offline-first: the loop runs degraded when the \
         uplink is down; it never stops on a failed poll or forward)"
    );

    wait_for_signal().await;
    tracing::info!("shutdown signal received; stopping the supervision loop");
    let _ = shutdown_tx.send(true);
    let last = runner.run(shutdown_rx).await;
    tracing::info!(
        polled = last.polled,
        forwarded = last.forwarded,
        buffered = last.buffered_len_after,
        degraded = last.degraded,
        "edge-agent stopped (simulation-first 0.x line; field write authority begins at v1.0.0 \
         with operator approval — nothing here writes)"
    );
    0
}

/// Waits for SIGTERM or SIGINT (the worker-composition shutdown pattern).
async fn wait_for_signal() {
    let sigterm = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            match signal(SignalKind::terminate()) {
                Ok(mut stream) => {
                    stream.recv().await;
                }
                Err(error) => {
                    tracing::error!(error = %error, "SIGTERM handler unavailable");
                    std::future::pending::<()>().await;
                }
            }
        }
        #[cfg(not(unix))]
        {
            std::future::pending::<()>().await;
        }
    };
    let sigint = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {}
            Err(error) => {
                tracing::error!(error = %error, "SIGINT handler unavailable");
                std::future::pending::<()>().await;
            }
        }
    };
    tokio::select! {
        () = sigterm => {}
        () = sigint => {}
    }
}
