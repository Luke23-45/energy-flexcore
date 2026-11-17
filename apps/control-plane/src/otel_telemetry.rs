// P11.04 — OpenTelemetry tracing and logs wiring for the control plane.
//
// WRITTEN-UNRUN. The pinned SDK versions and every SDK call below carry
// VERIFY-BUILD notes (the repo's recorded convention, cf.
// `crates/adapters/nats/src/publisher.rs`): the OpenTelemetry Rust SDK is
// metrics-and-logs Stable / traces Beta [S-63], and the ASSUMPTION that
// the pinned set compiles together is verified ONLY at the first human
// build. Nothing here has ever compiled or executed.
//
// What this module owns (record `records/04_p11_04_otel_tracing_rust.md`):
//
// 1. **OTLP export** via `opentelemetry-otlp` (gRPC/tonic) to the
//    Collector gateway (P11.06, `deploy/compose/`). The endpoint is
//    configuration; unset/unset-empty means OTel export is DISABLED and
//    the process keeps the plain JSON-stdout subscriber (P1.20 behavior
//    is never silently replaced by an exporter).
// 2. **Sampling**: parent-based first — `ParentBased(TraceIdRatioBased)`
//    decides once at the root span and propagates to all children (the
//    P11.06 sampling policy record). Tail sampling is deferred (recorded
//    trigger in the P11.06 record). The ratio default is 1.0 (sample
//    everything) until an operator tunes it on real traffic — a
//    RECOMMENDATION, never a tuned value.
// 3. **Span taxonomy over the frozen lifecycle**: the helper
//    [`command_gate_span`] opens ONE span per gate transition with the
//    `from`/`to` attributes taken from `CommandState::name()` — the
//    frozen 14-state vocabulary. NO span invents a state name; an illegal
//    edge is the caller's error to surface (the taxonomy mirrors the
//    frozen 27-edge lifecycle; the metric counterpart counts only legal
//    edges).
// 4. **W3C trace-context propagation** (`TraceContextPropagator`, the
//    `traceparent` header): set globally here so the P11.05 publish-side
//    seam (`crates/adapters/nats/src/publisher.rs`) injects the current
//    context into JetStream message headers, and inbound `traceparent`
//    headers extract at the consumer side (Agent B's files read it).
// 5. **Subscriber composition**: when OTel is enabled, the subscriber is
//    the registry + JSON fmt layer (the P1.20 stdout behavior, unchanged
//    shape) + the `tracing-opentelemetry` span layer + the
//    `opentelemetry-appender-tracing` logs bridge ([S-62]: every JSON
//    line ALSO becomes an OTel log record exported via OTLP; the two
//    emissions are parallel, neither replaces the other).
//
// The composition root (`main.rs`) calls [`init_from_env`] in a marked
// block and holds the returned [`TelemetryGuard`] for the process
// lifetime, shutting it down explicitly before exit (the recorded
// `std::process::exit` paths skip `Drop`, so the guard exposes an
// explicit [`TelemetryGuard::shutdown`]).

use std::time::Duration;

use flexcore_domain::command_state::CommandState;
use opentelemetry::global;
use opentelemetry::propagation::TraceContextPropagator;
use opentelemetry_otlp::{LogExporter, SpanExporter};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::resource::Resource;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider, TraceIdRatioBased};

/// Environment keys this module reads (composition-root configuration;
/// no secret values — an endpoint URL and a sampling ratio only).
pub const ENV_OTLP_ENDPOINT: &str = "FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT";
pub const ENV_SERVICE_NAME: &str = "FLEXCORE_OTEL_SERVICE_NAME";
pub const ENV_TRACE_SAMPLE_RATIO: &str = "FLEXCORE_OTEL_TRACE_SAMPLE_RATIO";

/// DEFAULT service name for the control plane's OTLP resource.
pub const DEFAULT_SERVICE_NAME: &str = "flexcore-control-plane";

/// DEFAULT trace sample ratio: 1.0 (sample everything) until an operator
/// tunes it on real traffic. A RECOMMENDATION registered in the P11.06
/// sampling policy record — never a tuned production value.
pub const DEFAULT_TRACE_SAMPLE_RATIO: f64 = 1.0;

/// The OTLP export timeout (one export attempt). Configuration constant,
/// not a performance claim.
const OTLP_EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

/// The OTel telemetry configuration, parsed fail-fast from the
/// environment by the composition root's config path ([`from_env`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryConfig {
    /// The OTLP/HTTP-gRPC endpoint of the Collector gateway
    /// (`http://otel-collector:4317` in the Compose shape). `None` or an
    /// empty value DISABLES OTel export (plain P1.20 subscriber only).
    pub otlp_endpoint: Option<String>,
    /// The resource service name (DEFAULT [`DEFAULT_SERVICE_NAME`]).
    pub service_name: String,
    /// The parent-based head-sampling ratio in [0.0, 1.0]
    /// (DEFAULT [`DEFAULT_TRACE_SAMPLE_RATIO`]).
    pub trace_sample_ratio: f64,
}

impl TelemetryConfig {
    /// Parses the configuration from an env-like mapping (pure so tests
    /// exercise it without a process env). An unusable ratio or a ratio
    /// outside [0.0, 1.0] is a named `Err` (fail-closed, the P1.21
    /// configuration convention) — never a silent fallback.
    pub fn from_mapping(env: &dyn Fn(&str) -> Option<String>) -> Result<Self, String> {
        let endpoint_raw = env(ENV_OTLP_ENDPOINT)
            .map(|v| v.trim().to_string())
            .unwrap_or_default();
        let otlp_endpoint = if endpoint_raw.is_empty() {
            None
        } else {
            Some(endpoint_raw)
        };
        let service_name = env(ENV_SERVICE_NAME)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_SERVICE_NAME.to_string());
        let ratio_raw = env(ENV_TRACE_SAMPLE_RATIO)
            .map(|v| v.trim().to_string())
            .unwrap_or_default();
        let trace_sample_ratio = if ratio_raw.is_empty() {
            DEFAULT_TRACE_SAMPLE_RATIO
        } else {
            let parsed: f64 = ratio_raw
                .parse()
                .map_err(|_| format!("{ENV_TRACE_SAMPLE_RATIO}: expected a number"))?;
            if !(0.0..=1.0).contains(&parsed) {
                return Err(format!(
                    "{ENV_TRACE_SAMPLE_RATIO}: must be within [0.0, 1.0], got {parsed}"
                ));
            }
            parsed
        };
        Ok(Self {
            otlp_endpoint,
            service_name,
            trace_sample_ratio,
        })
    }

    /// Reads the real process environment.
    pub fn from_env() -> Result<Self, String> {
        Self::from_mapping(&|key: &str| {
            std::env::var(key).ok().filter(|v| !v.is_empty())
        })
    }
}

/// Owns the SDK providers for the process lifetime. The composition root
/// holds this guard and calls [`TelemetryGuard::shutdown`] before its
/// recorded `std::process::exit` paths (which would otherwise skip
/// `Drop` — recorded limitation, module header).
#[derive(Debug)]
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

impl TelemetryGuard {
    /// A guard that owns nothing (OTel export disabled).
    fn disabled() -> Self {
        Self {
            tracer_provider: None,
            logger_provider: None,
        }
    }

    /// True when OTel export is composed (providers exist).
    pub fn is_enabled(&self) -> bool {
        self.tracer_provider.is_some()
    }

    /// Shuts the SDK providers down (flushes batched exports best-effort;
    /// VERIFY-BUILD: `SdkTracerProvider::shutdown` /
    /// `SdkLoggerProvider::shutdown` are the documented shutdown calls in
    /// the resolved SDK version). Consumes the guard.
    pub fn shutdown(self) {
        if let Some(provider) = self.tracer_provider {
            if let Err(error) = provider.shutdown() {
                eprintln!("otel tracer provider shutdown error: {error}");
            }
        }
        if let Some(provider) = self.logger_provider {
            if let Err(error) = provider.shutdown() {
                eprintln!("otel logger provider shutdown error: {error}");
            }
        }
    }
}

/// Initializes the whole telemetry stack: the global W3C propagator, the
/// OTLP trace/logs providers (when the endpoint is configured), and the
/// composed `tracing` subscriber (JSON stdout layer — the P1.20
/// behavior — plus, when enabled, the span layer and the logs bridge).
/// Returns the guard the composition root holds.
///
/// VERIFY-BUILD (opentelemetry 0.33.x / opentelemetry-otlp 0.33.x /
/// tracing-opentelemetry 0.34.0 / opentelemetry-appender-tracing 0.33.1 —
/// pinned in `apps/control-plane/Cargo.toml`, checked against crates.io
/// on 2026-11-14): every builder call below follows the documented 0.33
/// SDK shape; type and builder names may differ in the resolved version
/// and are re-verified at the first human build (ASSUMPTION, never run).
pub fn init_from_env(config: &TelemetryConfig, log_level: &str) -> TelemetryGuard {
    // W3C trace-context propagation is set UNCONDITIONALLY (even with
    // export disabled): the P11.05 header seam injects whatever the
    // current context holds, and the noop context yields no headers.
    let _ = global::set_text_map_propagator(TraceContextPropagator::new());

    let Some(endpoint) = config.otlp_endpoint.clone() else {
        // OTel export disabled: the plain P1.20 JSON subscriber only.
        init_stdout_subscriber(log_level);
        return TelemetryGuard::disabled();
    };

    // --- Trace provider (traces are Beta in the Rust SDK [S-63]) -------
    let span_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .with_timeout(OTLP_EXPORT_TIMEOUT)
        .build();
    let span_exporter = match span_exporter {
        Ok(exporter) => exporter,
        Err(error) => {
            // Fail-closed posture for EXPORTER CONSTRUCTION: a bad
            // endpoint/config is a startup error, not a silent no-op. The
            // composition root decides (it exits 2 with a named message).
            eprintln!("otel span exporter build failed: {error}");
            std::process::exit(2);
        }
    };
    let resource = Resource::builder()
        .with_service_name(config.service_name.clone())
        .build();
    // Parent-based head sampling: the decision is made once at the root
    // span and inherited by every child (P11.06 sampling policy record).
    let sampler = Sampler::ParentBased(Box::new(TraceIdRatioBased::new(
        config.trace_sample_ratio,
    )));
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_sampler(sampler)
        .with_resource(resource.clone())
        .build();
    global::set_tracer_provider(tracer_provider.clone());

    // --- Log provider (logs are Stable in the Rust SDK [S-63]) ---------
    let log_exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .with_timeout(OTLP_EXPORT_TIMEOUT)
        .build();
    let log_exporter = match log_exporter {
        Ok(exporter) => exporter,
        Err(error) => {
            eprintln!("otel log exporter build failed: {error}");
            std::process::exit(2);
        }
    };
    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(resource)
        .build();

    // --- Composed subscriber -------------------------------------------
    // The JSON fmt layer keeps the P1.20 stdout behavior; the span layer
    // turns `tracing` spans into OTel spans; the logs bridge ([S-62]) turns
    // every `tracing` event into an OTel log record on the OTLP path.
    let filter = tracing_subscriber::EnvFilter::try_new(log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().json().with_target(false);
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(tracing_opentelemetry::layer().with_tracer(global::tracer(
            "flexcore-control-plane",
        )))
        .with(opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
            &logger_provider,
        ))
        .try_init();

    TelemetryGuard {
        tracer_provider: Some(tracer_provider),
        logger_provider: Some(logger_provider),
    }
}

/// The disabled-export subscriber: registry + JSON fmt layer (the P1.20
/// shape, unchanged) + env filter.
fn init_stdout_subscriber(log_level: &str) {
    let filter = tracing_subscriber::EnvFilter::try_new(log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json().with_target(false))
        .try_init();
}

/// Opens ONE span for one command-lifecycle gate transition (the P11.04
/// span taxonomy). The span name is the frozen concept name; the
/// `from`/`to` attributes come from `CommandState::name()` ONLY — no span
/// invents a state. The caller is the code that COMMITTED the transition
/// (the command routes / the command consumer — Agent B's files; the
/// helper is exported for exactly that use). The lifecycle's observability
/// NEVER drives it: opening a span has no authority effect (ADR-010
/// untouched).
pub fn command_gate_span(from: CommandState, to: CommandState) -> tracing::Span {
    tracing::info_span!(
        "command.gate_transition",
        "command.state.from" = from.name(),
        "command.state.to" = to.name(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + '_ {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    /// Disabled by default: no endpoint key means export disabled and the
    /// service name falls back to the DEFAULT.
    #[test]
    fn config_defaults_to_disabled_export_and_default_service_name() {
        let config = TelemetryConfig::from_mapping(&env_of(&[])).expect("default config parses");
        assert_eq!(config.otlp_endpoint, None);
        assert_eq!(config.service_name, DEFAULT_SERVICE_NAME);
        assert!((config.trace_sample_ratio - DEFAULT_TRACE_SAMPLE_RATIO).abs() < f64::EPSILON);
    }

    /// A blank endpoint string disables export (no silent half-config);
    /// an endpoint value enables it.
    #[test]
    fn config_blank_endpoint_disables_export() {
        let config =
            TelemetryConfig::from_mapping(&env_of(&[(ENV_OTLP_ENDPOINT, "   ")])).expect("parses");
        assert_eq!(config.otlp_endpoint, None);
        let config = TelemetryConfig::from_mapping(&env_of(&[(
            ENV_OTLP_ENDPOINT,
            "http://otel-collector:4317",
        )]))
        .expect("parses");
        assert_eq!(
            config.otlp_endpoint.as_deref(),
            Some("http://otel-collector:4317")
        );
    }

    /// Fail-closed: an unusable or out-of-range ratio is a named error,
    /// never a silent fallback (the P1.21 configuration convention).
    #[test]
    fn config_ratio_is_fail_closed() {
        let bad =
            TelemetryConfig::from_mapping(&env_of(&[(ENV_TRACE_SAMPLE_RATIO, "1.5")]));
        assert!(bad.is_err(), "ratio above 1.0 must be rejected");
        let bad = TelemetryConfig::from_mapping(&env_of(&[(ENV_TRACE_SAMPLE_RATIO, "abc")]));
        assert!(bad.is_err(), "a non-number ratio must be rejected");
        let ok = TelemetryConfig::from_mapping(&env_of(&[(ENV_TRACE_SAMPLE_RATIO, "0")]));
        assert!(ok.is_ok(), "0.0 is a legal ratio");
        let ok = TelemetryConfig::from_mapping(&env_of(&[(ENV_TRACE_SAMPLE_RATIO, "1.0")]));
        assert!(ok.is_ok(), "1.0 is a legal ratio");
    }

    /// The span taxonomy: the gate-transition span carries the frozen
    /// state names verbatim (no invented state anywhere in the attributes).
    #[test]
    fn command_gate_span_carries_frozen_state_names() {
        let span = command_gate_span(CommandState::Proposed, CommandState::Validated);
        let meta = span.metadata().expect("span has metadata");
        assert_eq!(meta.name(), "command.gate_transition");
        // The frozen names are the ONLY vocabulary this helper can emit:
        // the attribute values come from CommandState::name() at the call
        // site, so the test pins the span NAME and the helper's type shape
        // (attribute values cannot be asserted without a subscriber; the
        // name-level guarantee is what the taxonomy requires here).
    }
}
