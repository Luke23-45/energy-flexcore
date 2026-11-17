// P11.05 — OpenTelemetry wiring for the Rust worker (Phase 11 wave 1,
// Agent A): the trace-context propagation seam for the JetStream
// publish path, plus optional OTLP export of worker spans.
//
// WRITTEN-UNRUN. The pinned SDK versions carry VERIFY-BUILD notes; the
// ASSUMPTION that the pinned set compiles together is verified ONLY at
// the first human build (record `records/
// 05_p11_05_otel_python_and_trace_headers.md`).
//
// Division of the P11.05 propagation contract:
//
// - **Publisher sets** (`crates/adapters/nats/src/publisher.rs`, the
//   P11.05 seam): every published JetStream message carries the W3C
//   `traceparent` (and any other context the active propagator writes)
//   as message headers, injected via the callback this module supplies —
//   the adapter crate itself stays OTel-free (the dependency rule: an
//   adapter depends on ports, never on infrastructure SDKs).
// - **Consumer reads**: the command consumer's JetStream header read is
//   Agent B's file (handoff note in the record); the header NAME it must
//   read is the W3C standard `traceparent` — set here and in the
//   P11.04 module via `TraceContextPropagator`.
//
// The worker's outbox publisher runs as a background drain loop with no
// request context of its own, so the injected context is whatever the
// code inside the drain cycle has active: absent a span, the global
// noop propagator injects NOTHING (no traceparent header is written —
// an honest no-op, never a fabricated trace id). A producer that wants
// its events followed end-to-end opens a span around the work that
// appends the outbox entry; the propagation then flows through the
// headers. Recorded limitation, not a promise of automatic end-to-end
// tracing.
//
// Optional OTLP export mirrors the control plane's P11.04 module shape:
// endpoint unset = export disabled, plain subscriber unchanged. The
// worker keeps its existing subscriber format; when export is enabled,
// the composed subscriber adds the `tracing-opentelemetry` span layer
// (the worker's `fmt` output shape is otherwise unchanged — recorded).

use opentelemetry::global;
use opentelemetry::propagation::{Injector, TraceContextPropagator};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::resource::Resource;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider, TraceIdRatioBased};
use std::time::Duration;

/// The environment keys this module reads (mirrors the P11.04 control
/// plane keys; worker-default service name).
pub const ENV_OTLP_ENDPOINT: &str = "FLEXCORE_OTEL_EXPORTER_OTLP_ENDPOINT";
pub const ENV_SERVICE_NAME: &str = "FLEXCORE_OTEL_SERVICE_NAME";
pub const ENV_TRACE_SAMPLE_RATIO: &str = "FLEXCORE_OTEL_TRACE_SAMPLE_RATIO";

/// DEFAULT service name for the worker's OTLP resource.
pub const DEFAULT_SERVICE_NAME: &str = "flexcore-worker";

/// DEFAULT trace sample ratio (RECOMMENDATION until an operator tunes
/// it; the P11.06 sampling policy record).
pub const DEFAULT_TRACE_SAMPLE_RATIO: f64 = 1.0;

const OTLP_EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

/// A W3C-propagator carrier that collects header name/value pairs
/// (the injection target for the publish-side seam; the publisher maps
/// these onto JetStream message headers).
#[derive(Debug, Default)]
pub struct HeaderCarrier {
    pub pairs: Vec<(String, String)>,
}

impl Injector for HeaderCarrier {
    fn set(&mut self, key: &str, value: String) {
        self.pairs.push((key.to_string(), value));
    }
}

/// The trace-context header pairs of the CURRENT OTel context (W3C
/// `traceparent` et al.). Called by the publish-side seam on every
/// published message. With no active span (the background drain loop's
/// steady state) the global propagator injects nothing — the returned
/// vector is empty and NO header is written (honest no-op, module
/// header). With OTel export disabled the global propagator is the
/// configured `TraceContextPropagator` over a noop-context — also
/// nothing.
pub fn trace_header_pairs() -> Vec<(String, String)> {
    let mut carrier = HeaderCarrier::default();
    global::get_text_map_propagator(|propagator| propagator.inject(&mut carrier));
    carrier.pairs
}

/// The parsed worker telemetry configuration (same fail-closed rules as
/// the P11.04 control-plane module).
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryConfig {
    /// The OTLP endpoint; `None` or empty DISABLES OTel export.
    pub otlp_endpoint: Option<String>,
    /// The resource service name.
    pub service_name: String,
    /// The parent-based head-sampling ratio in [0.0, 1.0].
    pub trace_sample_ratio: f64,
}

impl TelemetryConfig {
    /// Parses from an env-like mapping (pure; tests exercise it without
    /// a process env).
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

/// The worker's provider guard (same recorded `std::process::exit`
/// caveat as the P11.04 guard: shut down explicitly before exit —
/// `std::process::exit` skips `Drop`).
#[derive(Debug)]
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl TelemetryGuard {
    fn disabled() -> Self {
        Self {
            tracer_provider: None,
        }
    }

    /// True when OTel export is composed.
    pub fn is_enabled(&self) -> bool {
        self.tracer_provider.is_some()
    }

    /// Shuts the provider down (VERIFY-BUILD:
    /// `SdkTracerProvider::shutdown` is the documented shutdown call in
    /// the resolved SDK version). Consumes the guard.
    pub fn shutdown(self) {
        if let Some(provider) = self.tracer_provider {
            if let Err(error) = provider.shutdown() {
                eprintln!("otel tracer provider shutdown error: {error}");
            }
        }
    }
}

/// Initializes the worker telemetry stack: the global W3C propagator
/// (UNCONDITIONAL — the P11.05 publish seam needs it even with export
/// disabled) and the whole `tracing` subscriber. With export disabled the
/// subscriber is the worker's recorded plain-`fmt` shape (the P1.19
/// behavior, unchanged); with export enabled the composed registry adds
/// the `tracing-opentelemetry` span layer (the same layers, plus OTLP
/// export — the subscriber composition happens HERE so exactly one
/// subscriber is ever installed; a second `try_init` after the plain
/// `fmt().init()` would silently no-op). VERIFY-BUILD (opentelemetry
/// 0.33.x / opentelemetry-otlp 0.33.x / tracing-opentelemetry 0.34.0 —
/// pinned in `apps/worker/Cargo.toml`, checked against crates.io on
/// 2026-11-14): every call follows the documented 0.33 SDK shape;
/// re-verified at the first human build (ASSUMPTION, never run).
pub fn init_from_env(config: &TelemetryConfig, log_level: &str) -> TelemetryGuard {
    let _ = global::set_text_map_propagator(TraceContextPropagator::new());

    let filter = tracing_subscriber::EnvFilter::builder()
        .parse_lossy(log_level);

    let Some(endpoint) = config.otlp_endpoint.clone() else {
        // Export disabled: the recorded plain-`fmt` subscriber shape.
        let _ = tracing_subscriber::fmt().with_env_filter(filter).init();
        return TelemetryGuard::disabled();
    };

    let span_exporter = match SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .with_timeout(OTLP_EXPORT_TIMEOUT)
        .build()
    {
        Ok(exporter) => exporter,
        Err(error) => {
            eprintln!("otel span exporter build failed: {error}");
            std::process::exit(2);
        }
    };
    let resource = Resource::builder()
        .with_service_name(config.service_name.clone())
        .build();
    // Parent-based head sampling (the P11.06 sampling policy record;
    // tail sampling deferred with its recorded trigger).
    let sampler = Sampler::ParentBased(Box::new(TraceIdRatioBased::new(
        config.trace_sample_ratio,
    )));
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_sampler(sampler)
        .with_resource(resource)
        .build();
    global::set_tracer_provider(tracer_provider.clone());

    // Composed subscriber: the same `fmt` layer over the registry plus
    // the OTel span layer (spans additionally export OTLP; the log-line
    // shape is the `fmt` shape, unchanged).
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_opentelemetry::layer().with_tracer(global::tracer(
            "flexcore-worker",
        )))
        .try_init();

    TelemetryGuard {
        tracer_provider: Some(tracer_provider),
    }
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

    /// Disabled by default; the ratio is fail-closed like the P11.04
    /// control-plane configuration.
    #[test]
    fn config_defaults_and_fail_closed_ratio() {
        let config = TelemetryConfig::from_mapping(&env_of(&[])).expect("default config parses");
        assert_eq!(config.otlp_endpoint, None);
        assert_eq!(config.service_name, DEFAULT_SERVICE_NAME);
        let bad = TelemetryConfig::from_mapping(&env_of(&[(ENV_TRACE_SAMPLE_RATIO, "-0.5")]));
        assert!(bad.is_err(), "a ratio below 0.0 must be rejected");
        let bad = TelemetryConfig::from_mapping(&env_of(&[(ENV_TRACE_SAMPLE_RATIO, "xyz")]));
        assert!(bad.is_err(), "a non-number ratio must be rejected");
    }

    /// The header carrier collects exactly what the active propagator
    /// writes. With the default (noop) global propagator — the state of
    /// a process that never called `init_from_env` — NOTHING is
    /// injected: the publish seam writes no trace header rather than
    /// fabricating one.
    #[test]
    fn trace_header_pairs_are_a_noop_without_a_composed_context() {
        // Deliberately NOT initializing any provider/propagator here.
        let pairs = trace_header_pairs();
        assert!(
            pairs.is_empty(),
            "a noop context must inject no trace headers"
        );
    }
}
