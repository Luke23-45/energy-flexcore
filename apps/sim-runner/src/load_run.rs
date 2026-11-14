// P5.24 — The load-measurement harness scaffolding (report §19.2 layer 8,
//         §19.4 platform metrics; ledger
//         `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`
//         P5.24; the P3.27/P4.35 measurement discipline).
//
// WHAT THIS MODULE IS:
//
// - A sim-runner subcommand harness (`load-run`, wired in `main.rs`) that
//   executes a DECLARED load — N telemetry-ingest iterations through the
//   real deterministic replay pipeline and/or M sequential query round
//   trips against a configured HTTP endpoint — and emits one
//   `LoadMeasurementRecord` (JSON) with the measured figures, the hardware
//   description, and the load conditions. The ledger template
//   (`ledger/phase-05-operator-console-simulator/24_load_measurement.md`)
//   carries the EMPTY result tables the human's run fills.
//
// WHAT THIS HARNESS HONESTLY IS NOT (the scope notes travel ON THE RECORD):
//
// - It does NOT measure console browser-side performance (initial bundle
//   load, workspace render): no browser automation exists in this loop.
//   That is a pre-registered MANUAL procedure in the template, recorded
//   per §21.4 browser — never claimed from here.
// - It does NOT model concurrent users: the load is ONE sequential client.
//   A figure from this harness is a single-client figure, never a
//   deployment capacity claim.
// - Command fan-out time and recovery time after a service restart have NO
//   harness in 0.3.0: they are recorded `not_measured` with the reason
//   (fail-loud, never a default zero) and pre-registered as MANUAL
//   procedures in the template.
//
// Measurement discipline (reused from P3.27, not re-invented):
// - `MeasuredField` (the P3.27 type) for every figure: a measured value
//   with its unit and method, or `not_measured` with its mandatory reason.
// - Percentiles use the documented nearest-rank method on the MEASURED
//   samples only — never fitted, never extrapolated.
// - Hardware/container capture is the P3.27 `capture_hardware` — the same
//   block, so conditions stay comparable.
// - No threshold anywhere gates on a duration (report §19.4: no arbitrary
//   universal targets). The figures are the v0.3.0 load BASELINE inputs
//   for later comparison, never targets.
//
// Everything this harness produces is synthetic, simulation-only (the
// entire 0.x line, §21.4); the query leg talks only to endpoints the
// operator configures.

use serde::Serialize;

use crate::benchmark::{capture_hardware, HardwareDescription, MeasuredField};
use crate::replay::{run_scenario, ReplayError};

/// The record schema tag (bumped only with a migration note, §10.5).
pub const LOAD_RECORD_SCHEMA: &str = "flexcore-load-measurement-record/v1";

/// The subtask tag.
pub const LOAD_RECORD_SUBTASK: &str = "P5.24";

/// The baseline label every record carries. These figures are the v0.3.0
/// load baseline for LATER COMPARISON — never a target (report §19.4).
pub const LOAD_BASELINE_LABEL: &str =
    "v0.3.0-load-baseline (measured baseline for later comparison, not a target — report §19.4)";

/// The scope note about the console (rendered on every record; the honest
/// boundary of this harness).
pub const CONSOLE_SCOPE_NOTE: &str = "\
    Console browser-side performance (initial bundle load, workspace render \
    time) is NOT measured by this harness — no browser automation exists in \
    this phase. It is a pre-registered MANUAL procedure in \
    ledger/phase-05-operator-console-simulator/24_load_measurement.md, \
    recorded per §21.4 browser; no figure is claimed for a browser not \
    tested.";

/// The scope note about the load shape.
pub const LOAD_SHAPE_SCOPE_NOTE: &str = "\
    The load is ONE sequential client: the ingest leg runs N sequential \
    replay-pipeline iterations; the query leg runs M sequential HTTP GET \
    round trips. This does NOT model concurrent users, backpressure, or \
    broker/database contention — the conditions travel with every figure \
    and no capacity or concurrency claim is made from them.";

// --- The load spec -----------------------------------------------------------------

/// The declared load the harness executes. At least one leg must be
/// configured (a record with nothing measured is a usage error — a
/// measurement run that measures nothing is not a measurement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoadSpec {
    /// The scenario file the ingest leg runs (`--ingest-scenario`); absent
    /// when the operator configured only the query leg.
    pub ingest_scenario: Option<String>,
    /// The number of ingest iterations (`--ingest-iterations`, >= 1).
    pub ingest_iterations: u32,
    /// The HTTP endpoint the query leg probes (`--query-url`); absent when
    /// the operator configured only the ingest leg.
    pub query_url: Option<String>,
    /// The number of query round trips (`--query-count`, >= 1).
    pub query_count: u32,
}

impl LoadSpec {
    /// Validates the spec: at least one leg configured, iteration counts
    /// >= 1 when their leg is configured. A violation is a named error
    /// (exit 2 at the CLI), never a silently empty run.
    pub fn validate(&self) -> Result<(), LoadRunError> {
        let has_ingest = self.ingest_scenario.is_some();
        let has_query = self.query_url.is_some();
        if !has_ingest && !has_query {
            return Err(LoadRunError::Config(
                "the load spec measures nothing: configure --ingest-scenario (the \
                 telemetry-ingest leg) and/or --query-url (the query leg)"
                    .to_string(),
            ));
        }
        if has_ingest && self.ingest_iterations == 0 {
            return Err(LoadRunError::Config(
                "--ingest-iterations must be >= 1 when the ingest leg is configured"
                    .to_string(),
            ));
        }
        if has_query && self.query_count == 0 {
            return Err(LoadRunError::Config(
                "--query-count must be >= 1 when the query leg is configured".to_string(),
            ));
        }
        if has_query && !self.query_url.as_deref().unwrap_or("").starts_with("http://") {
            return Err(LoadRunError::Config(format!(
                "--query-url must be an http:// URL (this harness speaks plain HTTP \
                 only; an https endpoint is a named error, not a silent fallback): {}",
                self.query_url.as_deref().unwrap_or("")
            )));
        }
        Ok(())
    }
}

// --- Errors ------------------------------------------------------------------------

/// A load-run error (named; the CLI maps Config to exit 2, the rest to
/// exit 2 as well — a failed measurement is never recorded as success).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadRunError {
    /// Usage/configuration error.
    Config(String),
    /// I/O or protocol failure during the run.
    Io(String),
    /// A measurement could not be produced honestly.
    Measure(String),
}

impl std::fmt::Display for LoadRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadRunError::Config(detail) => write!(f, "load-run config error: {detail}"),
            LoadRunError::Io(detail) => write!(f, "load-run I/O error: {detail}"),
            LoadRunError::Measure(detail) => write!(f, "load-run measurement error: {detail}"),
        }
    }
}

impl std::error::Error for LoadRunError {}

// --- The measured results -----------------------------------------------------------

/// The measured (or explicitly not-measured) figures of one load run. The
/// not-measured fields carry their reason — fail-loud, never a default
/// zero (the P3.27 `MeasuredField` discipline).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LoadResults {
    /// Canonical telemetry records produced per second of ingest-leg wall
    /// clock, across ALL iterations (total records / total seconds).
    pub telemetry_ingest_rate: MeasuredField,
    /// The p95 (nearest-rank) of the per-iteration ingest wall clock, ms.
    pub telemetry_ingest_iteration_p95_ms: MeasuredField,
    /// The slowest single ingest iteration, ms (a raw observation).
    pub telemetry_ingest_iteration_max_ms: MeasuredField,
    /// The p50 (nearest-rank) of the query round-trip wall clock, ms.
    pub query_latency_p50_ms: MeasuredField,
    /// The p95 (nearest-rank) of the query round-trip wall clock, ms.
    pub query_latency_p95_ms: MeasuredField,
    /// The p99 (nearest-rank) of the query round-trip wall clock, ms.
    pub query_latency_p99_ms: MeasuredField,
    /// Command fan-out time — NOT harness-measurable in 0.3.0 (see the
    /// module header); `not_measured` with the reason.
    pub command_fan_out_time: MeasuredField,
    /// Recovery time after a service restart — NOT harness-measurable in
    /// 0.3.0; `not_measured` with the reason.
    pub recovery_time_after_restart: MeasuredField,
}

// --- The record ----------------------------------------------------------------------

/// The load conditions block (what was actually loaded — conditions travel
/// with every figure, ledger P5.24 step 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoadConditions {
    /// The ingest scenario id, verbatim from the artefact (`null` when the
    /// ingest leg was not configured).
    pub ingest_scenario_id: Option<String>,
    /// Ingest iterations executed.
    pub ingest_iterations: u32,
    /// Canonical records produced by the ingest leg in total.
    pub ingest_records_total: usize,
    /// The probed endpoint URL, verbatim (`null` when the query leg was
    /// not configured).
    pub query_url: Option<String>,
    /// Query round trips executed.
    pub query_count: u32,
    /// The concurrency model, verbatim (always the single-client statement
    /// — the harness never claims more).
    pub concurrency: String,
}

/// The run context (who ran it and when; the only volatile field).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoadRunContext {
    /// The code version the run names (the recorded sim-runner convention).
    pub code_version: String,
    /// Wall-clock generation instant, Unix milliseconds. Volatile metadata.
    pub generated_at_unix_ms: i64,
}

/// One load-measurement record: the declared load, the conditions, the
/// hardware block, and the measured (or honestly not-measured) figures.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LoadMeasurementRecord {
    /// The schema tag ([`LOAD_RECORD_SCHEMA`]).
    pub record_schema: String,
    /// The subtask tag ([`LOAD_RECORD_SUBTASK`]).
    pub subtask: String,
    /// The baseline label ([`LOAD_BASELINE_LABEL`]) — baseline inputs,
    /// never targets.
    pub baseline_label: String,
    /// The declared load spec, verbatim.
    pub load_spec: LoadSpec,
    /// The load conditions (what the figures describe).
    pub conditions: LoadConditions,
    /// The hardware/container description (the P3.27 capture — the same
    /// block, so conditions stay comparable).
    pub hardware: HardwareDescription,
    /// The measured figures (or `not_measured` with reasons).
    pub results: LoadResults,
    /// The scope notes (the honest boundary of this harness).
    pub scope_notes: Vec<String>,
    /// The pre-registered MANUAL procedures this record does NOT cover
    /// (console per-browser figures, command fan-out, recovery).
    pub manual_procedures: Vec<String>,
    /// The run context.
    pub run_context: LoadRunContext,
}

impl LoadMeasurementRecord {
    /// The JSON rendering (pretty; stable field order).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

// --- The percentile method -----------------------------------------------------------

/// The nearest-rank percentile of an ALREADY-SORTED ascending sample.
/// Documented method (recorded once): rank = ceil(fraction * n), clamped to
/// [1, n]; the value at that 1-based rank. No interpolation, no fitting —
/// the method is reproducible from the samples. An empty sample or a
/// fraction outside (0, 1] is a named error.
pub fn percentile_nearest_rank(sorted_ascending: &[f64], fraction: f64) -> Result<f64, LoadRunError> {
    if sorted_ascending.is_empty() {
        return Err(LoadRunError::Measure(
            "the percentile of an empty sample is undefined — no default zero is \
             ever substituted"
                .to_string(),
        ));
    }
    if !(fraction > 0.0 && fraction <= 1.0) {
        return Err(LoadRunError::Measure(format!(
            "the percentile fraction must be in (0, 1], got {fraction}"
        )));
    }
    let n = sorted_ascending.len();
    let rank = ((fraction * n as f64).ceil() as usize).clamp(1, n);
    Ok(sorted_ascending[rank - 1])
}

// --- The ingest leg ------------------------------------------------------------------

/// Runs the telemetry-ingest leg: `iterations` sequential runs of the real
/// deterministic replay pipeline over the pinned scenario, timed per
/// iteration with `std::time::Instant`. Returns the per-iteration
/// durations (ms, in execution order) and the total canonical records.
fn run_ingest_leg(
    scenario_json: &str,
    iterations: u32,
) -> Result<(Vec<f64>, usize, String), LoadRunError> {
    let mut durations_ms = Vec::with_capacity(iterations as usize);
    let mut records_total = 0usize;
    let mut scenario_id = String::new();
    for iteration in 0..iterations {
        let timer = std::time::Instant::now();
        let run = run_scenario(scenario_json).map_err(|error: ReplayError| {
            LoadRunError::Io(format!("ingest iteration {iteration} failed: {error}"))
        })?;
        durations_ms.push(timer.elapsed().as_secs_f64() * 1000.0);
        records_total += run.records.len();
        if iteration == 0 {
            scenario_id = run.scenario_id.clone();
        }
    }
    Ok((durations_ms, records_total, scenario_id))
}

// --- The query leg -------------------------------------------------------------------

/// The parsed http:// URL (host, port, path). Plain HTTP only — an https
/// URL is a named error at spec validation, never a silent fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHttpUrl {
    host: String,
    port: u16,
    path: String,
}

/// Parses an `http://host[:port]/path` URL (std-only; no TLS, no URL crate).
fn parse_http_url(url: &str) -> Result<ParsedHttpUrl, LoadRunError> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| LoadRunError::Config(format!("the query URL must start with http://: {url}")))?;
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err(LoadRunError::Config(format!("the query URL has no host: {url}")));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port_text)) => {
            let port = port_text.parse::<u16>().map_err(|_| {
                LoadRunError::Config(format!(
                    "the query URL port '{port_text}' is not a valid port: {url}"
                ))
            })?;
            (host.to_string(), port)
        }
        None => (authority.to_string(), 80u16),
    };
    Ok(ParsedHttpUrl {
        host,
        port,
        path: path.to_string(),
    })
}

/// One HTTP GET round trip over a fresh `std::net::TcpStream`
/// (Connection: close). Returns the wall-clock round-trip duration in ms.
/// The response body is read to EOF (bounded) and discarded — the harness
/// measures the ROUND TRIP, it does not interpret the payload.
fn http_get_round_trip_ms(parsed: &ParsedHttpUrl) -> Result<f64, LoadRunError> {
    use std::io::{Read, Write};
    let timer = std::time::Instant::now();
    let mut stream = std::net::TcpStream::connect((parsed.host.as_str(), parsed.port))
        .map_err(|error| {
            LoadRunError::Io(format!(
                "the query leg could not connect to {}:{}: {error}",
                parsed.host, parsed.port
            ))
        })?;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nAccept: application/json\r\nConnection: \
         close\r\n\r\n",
        parsed.path, parsed.host, parsed.port
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| LoadRunError::Io(format!("the query leg could not write: {error}")))?;
    let mut response = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| LoadRunError::Io(format!("the query leg could not read: {error}")))?;
        if read == 0 {
            break;
        }
        response.extend_from_slice(&chunk[..read]);
        if response.len() > 16 * 1024 * 1024 {
            return Err(LoadRunError::Io(
                "the query leg response exceeded the 16 MiB read bound — refusing to \
                 buffer an unbounded response"
                    .to_string(),
            ));
        }
    }
    if response.is_empty() {
        return Err(LoadRunError::Io(
            "the query leg received an empty response (the endpoint closed without \
             answering)"
                .to_string(),
        ));
    }
    Ok(timer.elapsed().as_secs_f64() * 1000.0)
}

/// Runs the query leg: `count` sequential round trips against the
/// configured endpoint, each timed. Returns the per-trip durations (ms, in
/// execution order).
fn run_query_leg(parsed: &ParsedHttpUrl, count: u32) -> Result<Vec<f64>, LoadRunError> {
    let mut durations_ms = Vec::with_capacity(count as usize);
    for trip in 0..count {
        let duration = http_get_round_trip_ms(parsed)
            .map_err(|error| LoadRunError::Io(format!("query trip {trip} failed: {error}")))?;
        durations_ms.push(duration);
    }
    Ok(durations_ms)
}

// --- The harness entry point ----------------------------------------------------------

/// Executes the declared load and builds the record. Every figure comes
/// from THIS run's measurements; a leg that was not configured (or cannot
/// be measured in 0.3.0) is `not_measured` with its reason — never zero.
pub fn run_load(spec: &LoadSpec, code_version: &str) -> Result<LoadMeasurementRecord, LoadRunError> {
    spec.validate()?;
    let generated_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);

    // The ingest leg (when configured): N sequential replay-pipeline
    // iterations, timed per iteration.
    let (ingest_rate, ingest_p95, ingest_max, conditions_ingest) = match &spec.ingest_scenario {
        Some(path) => {
            let scenario_json = std::fs::read_to_string(path).map_err(|error| {
                LoadRunError::Io(format!("the ingest scenario '{path}' could not be read: {error}"))
            })?;
            let (durations, records_total, scenario_id) =
                run_ingest_leg(&scenario_json, spec.ingest_iterations)?;
            let total_seconds: f64 = durations.iter().sum::<f64>() / 1000.0;
            if total_seconds <= 0.0 {
                return Err(LoadRunError::Measure(
                    "the ingest leg measured zero total elapsed time — refusing to \
                     divide by it (no fabricated rate)"
                        .to_string(),
                ));
            }
            let mut sorted = durations.clone();
            sorted.sort_by(|a, b| a.total_cmp(b));
            let rate = records_total as f64 / total_seconds;
            let max = sorted.last().copied().unwrap_or(0.0);
            (
                MeasuredField::measured(
                    rate,
                    "records/s",
                    "total canonical records produced by the ingest leg divided by the \
                     leg's TOTAL wall clock across all iterations (one sequential client, \
                     no concurrency); measured, never estimated",
                )
                .map_err(|error| LoadRunError::Measure(error.to_string()))?,
                MeasuredField::measured(
                    percentile_nearest_rank(&sorted, 0.95)?,
                    "ms",
                    "nearest-rank p95 over the per-iteration wall-clock durations \
                     (execution-ordered samples, sorted ascending; no interpolation)",
                )
                .map_err(|error| LoadRunError::Measure(error.to_string()))?,
                MeasuredField::measured(
                    max,
                    "ms",
                    "the slowest single ingest iteration (a raw observation, never a \
                     percentile or a fitted figure)",
                )
                .map_err(|error| LoadRunError::Measure(error.to_string()))?,
                Some((scenario_id, records_total)),
            )
        }
        None => (
            MeasuredField::not_measured(
                "the ingest leg was not configured for this run (--ingest-scenario \
                 absent)",
            ),
            MeasuredField::not_measured(
                "the ingest leg was not configured for this run (--ingest-scenario \
                 absent)",
            ),
            MeasuredField::not_measured(
                "the ingest leg was not configured for this run (--ingest-scenario \
                 absent)",
            ),
            None,
        ),
    };

    // The query leg (when configured): M sequential HTTP GET round trips,
    // each timed.
    let (query_p50, query_p95, query_p99) = match &spec.query_url {
        Some(url) => {
            let parsed = parse_http_url(url)?;
            let durations = run_query_leg(&parsed, spec.query_count)?;
            let mut sorted = durations.clone();
            sorted.sort_by(|a, b| a.total_cmp(b));
            (
                MeasuredField::measured(
                    percentile_nearest_rank(&sorted, 0.50)?,
                    "ms",
                    "nearest-rank p50 over the per-trip round-trip wall-clock durations \
                     (fresh TCP connection per trip, Connection: close; one sequential \
                     client, no concurrency)",
                )
                .map_err(|error| LoadRunError::Measure(error.to_string()))?,
                MeasuredField::measured(
                    percentile_nearest_rank(&sorted, 0.95)?,
                    "ms",
                    "nearest-rank p95 over the per-trip round-trip wall-clock durations \
                     (fresh TCP connection per trip, Connection: close)",
                )
                .map_err(|error| LoadRunError::Measure(error.to_string()))?,
                MeasuredField::measured(
                    percentile_nearest_rank(&sorted, 0.99)?,
                    "ms",
                    "nearest-rank p99 over the per-trip round-trip wall-clock durations \
                     (fresh TCP connection per trip, Connection: close)",
                )
                .map_err(|error| LoadRunError::Measure(error.to_string()))?,
            )
        }
        None => (
            MeasuredField::not_measured(
                "the query leg was not configured for this run (--query-url absent)",
            ),
            MeasuredField::not_measured(
                "the query leg was not configured for this run (--query-url absent)",
            ),
            MeasuredField::not_measured(
                "the query leg was not configured for this run (--query-url absent)",
            ),
        ),
    };

    let (ingest_scenario_id, ingest_records_total) = conditions_ingest
        .map(|(id, records)| (Some(id), records))
        .unwrap_or((None, 0usize));

    Ok(LoadMeasurementRecord {
        record_schema: LOAD_RECORD_SCHEMA.to_string(),
        subtask: LOAD_RECORD_SUBTASK.to_string(),
        baseline_label: LOAD_BASELINE_LABEL.to_string(),
        load_spec: spec.clone(),
        conditions: LoadConditions {
            ingest_scenario_id,
            ingest_iterations: if spec.ingest_scenario.is_some() {
                spec.ingest_iterations
            } else {
                0
            },
            ingest_records_total,
            query_url: spec.query_url.clone(),
            query_count: if spec.query_url.is_some() {
                spec.query_count
            } else {
                0
            },
            concurrency: "single sequential client (no concurrency; the harness does \
                 not model concurrent users — LOAD_SHAPE_SCOPE_NOTE travels on the \
                 record)"
                .to_string(),
        },
        hardware: capture_hardware(),
        results: LoadResults {
            telemetry_ingest_rate: ingest_rate,
            telemetry_ingest_iteration_p95_ms: ingest_p95,
            telemetry_ingest_iteration_max_ms: ingest_max,
            query_latency_p50_ms: query_p50,
            query_latency_p95_ms: query_p95,
            query_latency_p99_ms: query_p99,
            command_fan_out_time: MeasuredField::not_measured(
                "no command fan-out harness exists in 0.3.0 sim-runner (a fan-out \
                 measurement needs the command-issue path under concurrent targets — \
                 pre-registered as a MANUAL procedure in \
                 ledger/phase-05-operator-console-simulator/24_load_measurement.md)",
            ),
            recovery_time_after_restart: MeasuredField::not_measured(
                "no service-restart orchestration exists in this harness (recovery \
                 needs a controlled restart of the Phase 1-4 services — \
                 pre-registered as a MANUAL procedure in \
                 ledger/phase-05-operator-console-simulator/24_load_measurement.md)",
            ),
        },
        scope_notes: vec![
            CONSOLE_SCOPE_NOTE.to_string(),
            LOAD_SHAPE_SCOPE_NOTE.to_string(),
        ],
        manual_procedures: vec![
            "console initial bundle load + workspace render, per §21.4 browser \
             (24_load_measurement.md §4 procedure B)"
                .to_string(),
            "command fan-out time (24_load_measurement.md §4 procedure C)".to_string(),
            "recovery time after a service restart (24_load_measurement.md §4 \
             procedure D)"
                .to_string(),
        ],
        run_context: LoadRunContext {
            code_version: code_version.to_string(),
            generated_at_unix_ms,
        },
    })
}

// --- A small self-check ------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_rank_percentile_is_reproducible_from_the_samples() {
        let mut samples: Vec<f64> = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        assert_eq!(percentile_nearest_rank(&samples, 0.5).unwrap(), 30.0);
        assert_eq!(percentile_nearest_rank(&samples, 0.95).unwrap(), 50.0);
        assert_eq!(percentile_nearest_rank(&samples, 0.99).unwrap(), 50.0);
        samples.sort_by(|a, b| a.total_cmp(b));
        // ceil(0.95 * 2) = 2 → the second of two samples.
        let two = vec![7.0, 9.0];
        assert_eq!(percentile_nearest_rank(&two, 0.95).unwrap(), 9.0);
    }

    #[test]
    fn an_empty_sample_or_bad_fraction_is_a_named_error_never_zero() {
        let empty: Vec<f64> = Vec::new();
        assert!(percentile_nearest_rank(&empty, 0.5).is_err());
        let samples = vec![1.0];
        assert!(percentile_nearest_rank(&samples, 0.0).is_err());
        assert!(percentile_nearest_rank(&samples, 1.5).is_err());
    }

    #[test]
    fn the_spec_validator_fails_loud_on_an_empty_or_misconfigured_load() {
        let nothing = LoadSpec {
            ingest_scenario: None,
            ingest_iterations: 0,
            query_url: None,
            query_count: 0,
        };
        assert!(nothing.validate().is_err(), "measuring nothing is an error");
        let bad_ingest = LoadSpec {
            ingest_scenario: Some("s.json".to_string()),
            ingest_iterations: 0,
            query_url: None,
            query_count: 0,
        };
        assert!(bad_ingest.validate().is_err());
        let https = LoadSpec {
            ingest_scenario: None,
            ingest_iterations: 0,
            query_url: Some("https://control-plane/api/v1/tenants".to_string()),
            query_count: 1,
        };
        let error = format!("{}", https.validate().unwrap_err());
        assert!(error.contains("http://"), "https is a named error");
        let ok = LoadSpec {
            ingest_scenario: Some("s.json".to_string()),
            ingest_iterations: 2,
            query_url: Some("http://127.0.0.1:8080/api/v1/tenants".to_string()),
            query_count: 3,
        };
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn the_url_parser_is_std_only_and_strict() {
        let parsed = parse_http_url("http://127.0.0.1:8080/api/v1/tenants").unwrap();
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 8080);
        assert_eq!(parsed.path, "/api/v1/tenants");
        let default_port = parse_http_url("http://localhost/").unwrap();
        assert_eq!(default_port.port, 80);
        assert_eq!(default_port.path, "/");
        assert!(parse_http_url("https://host/").is_err());
        assert!(parse_http_url("http:///path").is_err());
        assert!(parse_http_url("http://host:notaport/").is_err());
    }
}
