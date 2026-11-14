// P7.10 — `plugin-host`: the binary root of the WASM component host
// prototype (report §5.4 Class B, §8.1, §8.3 — the host is a SEPARATE
// PROCESS for security isolation; ledger §P7.10 step 1).
//
// Composition of this wave (honest):
//
// ```text
// FLEXCORE_PLUGIN_HOST_COMPONENT (component path; OPTIONAL = unconfigured)
// FLEXCORE_PLUGIN_HOST_WORLD     (optimizer-plugin | grid-model-provider-plugin
//                                 | telemetry-source-plugin; OPTIONAL)
//   → tracing (structured, env-filtered)
//   → unconfigured?  → named error, exit 2 (the host refuses to idle
//                      permissively — it RUNS ONLY when a human supplies a
//                      signed component AND a bindings-enabled build)
//   → ComponentArtifact::load(path)   (loading, NOT building; there are no
//                      artifacts in this loop and every absent path is a
//                      named error)
//   → EnforcementGate::evaluate  (P7.12: manifest permissions ∩ profile
//                      ceiling, broker scope narrowing, vocabulary-drift
//                      refusal — every refusal is exit 2 BEFORE any
//                      component code would run)
//   → enforcement::instantiate(&verdict, &artifact) / binding.verify()
//                      → BindingsNotGenerated in the default build (the
//                        fail-closed bindings seam; exit 1)
// ```
//
// There is no happy path in this loop, and that is the recorded posture:
// the default build cannot instantiate anything (no generated bindings, no
// components exist). What the binary proves today is the fail-closed
// startup chain, the named errors, and the limit construction — the real
// instantiation is the first human build behind the `generated-bindings`
// feature (`src/bindings.rs`; decision record §4).
//
// The host↔control-plane transport is the P7.20 gRPC seam (NOT in-process
// linking — report §8.3: the host is a separate process wired over
// gRPC/Protobuf; ledger §P7.34 gives it an mTLS service identity). No
// transport exists in this wave: the binary composes no listener and no
// client, and states that loudly.
//
// Exit codes (recorded): 2 = configuration error (unconfigured path/world),
// 1 = operational failure (missing artifact, no bindings), 0 = unreachable
// in this loop (a full successful run requires the human build).

use std::process::ExitCode;

use flexcore_plugin_host::broker::{CapabilityBroker, InMemoryBrokerAuditSink};
use flexcore_plugin_host::enforcement::{
    instantiate, EnforcementError, EnforcementGate, EnforcementRequest,
    InMemoryEnforcementAuditSink, InstantiationInput, ProfileCeiling,
};
use flexcore_plugin_host::host::{ComponentArtifact, HostError, WorldId};
use flexcore_plugin_host::limits::DeclaredBounds;

/// The component artifact path (the `.wasm` the human's `cargo component`
/// run produced). Absent/unset = unconfigured: named error, exit 2.
const ENV_COMPONENT_PATH: &str = "FLEXCORE_PLUGIN_HOST_COMPONENT";

/// The world selector (the WIT world name; absent = unconfigured: named
/// error, exit 2 — the host never guesses a world).
const ENV_WORLD: &str = "FLEXCORE_PLUGIN_HOST_WORLD";

/// The class-B entry-point symbol the manifest declares (the WIT export the
/// host binds). Absent = unconfigured: named error, exit 2.
const ENV_SYMBOL: &str = "FLEXCORE_PLUGIN_HOST_SYMBOL";

/// The plugin the instantiation would belong to. Absent = unconfigured:
/// named error, exit 2 (the enforcement gate must know WHO it is deciding
/// for — P7.12).
const ENV_PLUGIN_ID: &str = "FLEXCORE_PLUGIN_HOST_PLUGIN_ID";

/// The manifest's `plugin-id` (the derivation trail recorded on grants).
/// Absent = unconfigured: named error, exit 2.
const ENV_MANIFEST_ID: &str = "FLEXCORE_PLUGIN_HOST_MANIFEST_ID";

/// The manifest's version. Absent = unconfigured: named error, exit 2.
const ENV_MANIFEST_VERSION: &str = "FLEXCORE_PLUGIN_HOST_MANIFEST_VERSION";

/// The permission ids the manifest declares, comma-separated (P7.12).
/// Absent/empty = the manifest declares no permissions (a capability-less
/// instantiation is legal). Values are re-validated by the gate — never
/// trusted because they arrived through the environment.
const ENV_PERMISSIONS: &str = "FLEXCORE_PLUGIN_HOST_PERMISSIONS";

/// Structured tracing at the configured level (the app-crate convention:
/// `FLEXCORE_LOG_LEVEL`, else `info`; the plain level is passed to
/// EnvFilter, no free-form filter expression).
fn init_tracing() {
    let level = std::env::var("FLEXCORE_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter = tracing_subscriber::EnvFilter::builder().parse_lossy(level.as_str());
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Prints a host error with its stable machine name (the named-error
/// discipline: operators grep the NAME, not the prose).
fn report_host_error(stage: &str, error: &HostError) {
    eprintln!("plugin-host: {stage} failed [{}]: {error}", error.name());
    tracing::error!(stage, error_name = error.name(), error = %error, "{stage} failed");
}

/// Prints an enforcement error with its stable machine name (same
/// discipline; the gate's refusals are the door's refusals).
fn report_enforcement_error(stage: &str, error: &EnforcementError) {
    eprintln!("plugin-host: {stage} failed [{}]: {error}", error.name());
    tracing::error!(stage, error_name = error.name(), error = %error, "{stage} failed");
}

/// One required environment value (non-empty). Missing = unconfigured.
fn required_env(name: &str) -> Result<String, HostError> {
    match std::env::var(name) {
        Ok(raw) if !raw.trim().is_empty() => Ok(raw),
        _ => Err(HostError::ComponentUnconfigured {
            what: format!("{name} is not set"),
        }),
    }
}

/// The permission ids the manifest declares (comma-separated; absent means
/// the manifest declares none). The gate re-validates every id.
fn permissions_from_env() -> Vec<String> {
    std::env::var(ENV_PERMISSIONS)
        .unwrap_or_default()
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// The resource-field names the manifest's `resources` object carried, as
/// far as this prototype's configuration expresses them: the two required
/// bounds are always declared (`parse_declared_bounds` refuses their
/// absence); each optional bound counts as declared iff its variable is
/// set. The gate drift-checks these names against the kernel vocabulary
/// (P7.12) — `timeoutMs` or any fuel/concurrency name would be a
/// `PERMISSION_VOCABULARY_DRIFT` refusal, never a default interpretation.
fn declared_resource_fields() -> Vec<String> {
    let mut fields = vec!["max-memory-mb".to_string(), "timeout-ms".to_string()];
    let optional = [
        ("FLEXCORE_PLUGIN_HOST_MAX_OUTPUT_MB", "max-output-mb"),
        ("FLEXCORE_PLUGIN_HOST_MAX_CALLS_PER_MINUTE", "max-calls-per-minute"),
    ];
    for (env, field) in optional {
        let set = std::env::var(env)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        if set {
            fields.push(field.to_string());
        }
    }
    fields
}

fn main() -> ExitCode {
    init_tracing();
    tracing::info!(
        "plugin-host starting (P7.10 prototype; separate process per report §8.3; \
         simulation-only 0.x line — this host executes UNTRUSTED components and is \
         itself an untrusted process to the control plane, ledger §P7.34)"
    );
    tracing::warn!(
        "no host↔control-plane transport is composed in this wave: the gRPC/Protobuf \
         wire is the P7.20 seam and the mTLS service identity is the P7.34 seam — \
         nothing links this host into the control plane in-process (report §8.3)"
    );

    // --- Configuration (fail-fast, exit 2 on any missing piece) ------------

    let component_path = match std::env::var(ENV_COMPONENT_PATH) {
        Ok(path) if !path.trim().is_empty() => path,
        _ => {
            let error = HostError::ComponentUnconfigured {
                what: format!(
                    "the component path is not set ({ENV_COMPONENT_PATH}); the host runs \
                     only when a human supplies a signed component — an unconfigured host \
                     refuses to run rather than idle permissively"
                ),
            };
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };
    let world_selector = match std::env::var(ENV_WORLD) {
        Ok(world) if !world.trim().is_empty() => world,
        _ => {
            let error = HostError::ComponentUnconfigured {
                what: format!("the world selector is not set ({ENV_WORLD})"),
            };
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };
    let symbol = match std::env::var(ENV_SYMBOL) {
        Ok(symbol) if !symbol.trim().is_empty() => symbol,
        _ => {
            let error = HostError::ComponentUnconfigured {
                what: format!("the entry-point symbol is not set ({ENV_SYMBOL})"),
            };
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };
    let world = match WorldId::parse(&world_selector) {
        Ok(world) => world,
        Err(error) => {
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };

    // --- Identity + enforcement inputs (P7.12) -------------------------------
    //
    // The door must know WHO it is deciding for and WHAT the manifest
    // declared. The plugin/manifest identity is required configuration;
    // the declared permission ids and resource-field names are re-validated
    // by the gate (nothing is trusted because it arrived through the
    // environment).

    let plugin_id = match required_env(ENV_PLUGIN_ID) {
        Ok(value) => value,
        Err(error) => {
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };
    let manifest_id = match required_env(ENV_MANIFEST_ID) {
        Ok(value) => value,
        Err(error) => {
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };
    let manifest_version = match required_env(ENV_MANIFEST_VERSION) {
        Ok(value) => value,
        Err(error) => {
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };
    let manifest_permissions = permissions_from_env();
    let declared_fields = declared_resource_fields();

    // --- Declared resource bounds --------------------------------------------
    //
    // The bounds come from the manifest's declared resources. The manifest
    // is read and validated by the kernel (P7.05) and its bounds reach the
    // host over the composition seam (record §6); this prototype reads the
    // bounds from the environment in the same shape the kernel's
    // `ResourceBounds` struct carries them. Fail-closed: a missing or
    // unparseable bound is a named error, never a default — EXCEPT the two
    // fields the v1 schema itself leaves optional (`max-output-mb`,
    // `max-calls-per-minute`), where the DEFAULT host caps apply
    // (`limits.rs`, decision record §3). The fuel budget is mandatory
    // deployment configuration (no default exists). The GATE re-validates
    // every bound against the kernel's closed ranges before any verdict.

    let bounds = match parse_declared_bounds() {
        Ok(bounds) => bounds,
        Err(error) => {
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };
    let fuel_budget = match std::env::var("FLEXCORE_PLUGIN_HOST_FUEL_BUDGET") {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(fuel) if fuel > 0 => fuel,
            _ => {
                let error = HostError::LimitsInvalid {
                    field: "fuel-budget".to_string(),
                    reason: format!(
                        "'{raw}' is not a positive integer (the fuel budget is mandatory \
                         deployment configuration; there is no default budget)"
                    ),
                };
                report_host_error("configuration", &error);
                return ExitCode::from(2);
            }
        },
        Err(_) => {
            let error = HostError::LimitsInvalid {
                field: "fuel-budget".to_string(),
                reason: format!(
                    "not set (FLEXCORE_PLUGIN_HOST_FUEL_BUDGET); metering is mandatory \
                     and an unmetered instance is not constructible"
                ),
            };
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };

    // --- The enforcement gate (P7.12) ----------------------------------------
    //
    // The broker (P7.11) and the enforcement gate are composed BEFORE any
    // instantiation: the gate drives the broker's scope narrowing, and every
    // host function a component can call goes through the broker. The
    // in-memory audit sinks are this wave's composition (the durable path is
    // the P7.20/P7.23 seam).
    //
    // DEFAULT (recorded, decision record §4): the prototype composes the
    // EMPTY profile ceiling — no deployment profile exists in this wave
    // (composition profiles are the P7.15 seam, owned by the kernel wave),
    // so every declared permission is refused at the door
    // (PERMISSION_ABOVE_PROFILE) and only a capability-less instantiation
    // can pass, straight into the fail-closed bindings seam.

    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let gate = EnforcementGate::compose(broker, Box::new(InMemoryEnforcementAuditSink::new()));
    let request = EnforcementRequest {
        plugin_id,
        manifest_id,
        manifest_version,
        world,
        entry_symbol: symbol,
        manifest_permissions,
        declared_resource_fields: declared_fields,
        bounds,
        fuel_budget,
    };
    let ceiling = ProfileCeiling::empty();
    // The gate reads no clock of its own: the HOST supplies the time and
    // with it the responsibility for its correctness (the broker's
    // discipline, carried into the gate). A clock before the Unix epoch is
    // a broken clock — refused, never defaulted.
    let now_unix_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        Err(_) => {
            let error = HostError::LimitsInvalid {
                field: "host-clock".to_string(),
                reason: "the system clock is before the Unix epoch; refusing to decide"
                    .to_string(),
            };
            report_host_error("configuration", &error);
            return ExitCode::from(2);
        }
    };
    let verdict = match gate.evaluate(
        &InstantiationInput { request: &request, ceiling: &ceiling },
        now_unix_ms,
    ) {
        Ok(verdict) => {
            tracing::info!(
                plugin_id = verdict.plugin_id(),
                limits = %verdict.limits(),
                capabilities = ?verdict.granted_capabilities(),
                "enforcement verdict issued (capability set frozen at instantiation, P7.12)"
            );
            verdict
        }
        Err(error) => {
            report_enforcement_error("enforcement", &error);
            return ExitCode::from(2);
        }
    };

    // --- Load + bind (fail-closed; exit 1 on any operational failure) -------

    let artifact = match ComponentArtifact::load(std::path::Path::new(&component_path)) {
        Ok(artifact) => {
            tracing::info!(
                path = %artifact.path().display(),
                size_bytes = artifact.size_bytes(),
                "component artifact loaded (loading only — the build step is the \
                 human's cargo-component run)"
            );
            artifact
        }
        Err(error) => {
            report_host_error("component load", &error);
            return ExitCode::from(1);
        }
    };

    let binding = match instantiate(&verdict, &artifact) {
        Ok(binding) => binding,
        Err(error) => {
            report_enforcement_error("component bind", &error);
            return ExitCode::from(1);
        }
    };
    match binding.verify() {
        Ok(()) => {
            // Unreachable in this loop: the default build's binding verify
            // is BindingsNotGenerated. The arm exists so the posture is
            // explicit — if a future build reaches here, it must state what
            // ran, never claim a success this loop never produced.
            tracing::error!(
                "binding.verify() returned Ok in a build that cannot instantiate; refusing \
                 to proceed (this arm must be replaced by the real run path at the first \
                 human build, never trusted as-is)"
            );
            return ExitCode::from(1);
        }
        Err(error) => {
            report_host_error("component binding", &error);
            tracing::warn!(
                "honest posture restated: no generated bindings exist in this build, and no \
                 signed component was supplied by a human — the host instantiated nothing"
            );
            return ExitCode::from(1);
        }
    }
}

/// Parses the manifest's declared bounds from the environment, in the shape
/// the kernel's `ResourceBounds` carries (`max-memory-mb`, `timeout-ms`,
/// `max-output-mb`, `max-calls-per-minute`). The two required fields are
/// required (no defaults — fail-closed); the two optional fields default to
/// `None`, which [`ResourceLimits::from_declared_bounds`] maps to the
/// DEFAULT host caps. A malformed value is a named error, never a silent
/// fallback.
fn parse_declared_bounds() -> Result<DeclaredBounds, HostError> {
    fn required_u64(env: &str) -> Result<u64, HostError> {
        match std::env::var(env) {
            Ok(raw) if !raw.trim().is_empty() => raw.trim().parse::<u64>().map_err(|_| {
                HostError::LimitsInvalid {
                    field: env.to_string(),
                    reason: format!("'{raw}' is not a non-negative integer"),
                }
            }),
            _ => Err(HostError::LimitsInvalid {
                field: env.to_string(),
                reason: "not set (required: the manifest's declared bound must reach the host)".to_string(),
            }),
        }
    }
    fn optional_u64(env: &str) -> Result<Option<u64>, HostError> {
        match std::env::var(env) {
            Ok(raw) if !raw.trim().is_empty() => {
                raw.trim().parse::<u64>().map(Some).map_err(|_| {
                    HostError::LimitsInvalid {
                        field: env.to_string(),
                        reason: format!("'{raw}' is not a non-negative integer"),
                    }
                })
            }
            _ => Ok(None),
        }
    }
    Ok(DeclaredBounds {
        max_memory_mb: required_u64("FLEXCORE_PLUGIN_HOST_MAX_MEMORY_MB")?,
        timeout_ms: required_u64("FLEXCORE_PLUGIN_HOST_TIMEOUT_MS")?,
        max_output_mb: optional_u64("FLEXCORE_PLUGIN_HOST_MAX_OUTPUT_MB")?,
        max_calls_per_minute: optional_u64("FLEXCORE_PLUGIN_HOST_MAX_CALLS_PER_MINUTE")?,
    })
}
