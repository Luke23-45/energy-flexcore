// P7.10 — The WASM component host: artifact loading, world selection, and
// binding through the compile-gated bindings seam (report §5.4 Class B,
// §8.1, §8.3, §21.4; ledger `ledger_phase_7.md` §P7.10).
//
// What EXISTS here (honest):
//
// | Concern | Status |
// |---|---|
// | Component artifact LOADING from a local file path | Implemented — existence, size cap, and readability checks; loading, NOT building (the build step is the human's `cargo component` run; see `40_toolchain_pinning.md` §5) |
// | World selection over the P7.09 `wit/` worlds | Implemented — the three recorded worlds by their WIT file path; the host never re-declares the WIT contract in code |
// | Instantiation against a world | SEAM ONLY — the compile-gated bindings seam ([`crate::bindings`]); the default build returns a fail-closed `NoBindings` binding whose every use is [`HostError::BindingsNotGenerated`] |
// | Resource limits | [`crate::limits`] — derived from the manifest's declared bounds |
// | Capability dispatch | [`crate::broker`] — every host function a component can call goes through the broker (P7.11) |
//
// Design constraints this module enforces structurally:
//
// - **No ambient access.** The host exposes NO filesystem, NO network, NO
//   clock, and NO environment access to a component unless a scoped
//   capability for exactly that is granted AND the manifest declares it AND
//   the grant is recorded (P7.11; report §5.4). The P7.09 worlds declare
//   zero `wasi:` imports (`wit/README.md` §5 item 8), and WASI host support
//   is deliberately not enabled in the wasmtime feature set (decision
//   record §3) — there is no ambient surface to leak through.
// - **Sandbox containment.** A component that exceeds a limit is terminated
//   cleanly with a named error and the instance torn down — never a host
//   crash (ledger §P7.10 acceptance; §P7.13 hostile-suite target).
// - **Fail-closed everywhere.** A missing artifact, an unconfigured path,
//   a missing bindings build, or a failed audit write is a named error —
//   nothing degrades silently into a permissive mode.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::bindings::{self, PluginBinding};
use crate::limits::ResourceLimits;

// --- Component artifact bound (fail-closed, DEFAULT decision) -----------------

/// The maximum accepted component artifact size in bytes (DEFAULT decision,
/// recorded in the decision record §3): a `.wasm` artifact is untrusted
/// input, and an unbounded read is itself a host-side denial-of-service
/// vector before any sandbox limit could apply. The real sandbox limits
/// ([`crate::limits`]) govern the INSTANCE; this cap governs the LOADER.
pub const MAX_COMPONENT_BYTES: u64 = 64 * 1024 * 1024;

// --- The recorded worlds (P7.09) ----------------------------------------------

/// The worlds the host instantiates against, as recorded by P7.09. Each
/// variant names its WIT file path under `wit/` — the host references the
/// recorded packages by path and never re-declares the WIT contract in
/// code. Bindings generation for these worlds is a BUILD-TIME step owned by
/// the human's `cargo component` / `wit-bindgen` run (`wit/TOOLCHAIN.md`;
/// `40_toolchain_pinning.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldId {
    /// `flexcore:optimizer@1.0.0` / world `optimizer-plugin` (exports
    /// `optimizer`; mirrors `flexcore-optimization-api/src/port.rs::Optimizer`).
    OptimizerPlugin,
    /// `flexcore:grid@1.0.0` / world `grid-model-provider-plugin` (exports
    /// `grid-model-provider`; mirrors `flexcore-grid-api/src/port.rs::GridModelProvider`).
    GridModelProviderPlugin,
    /// `flexcore:telemetry@1.0.0` / world `telemetry-source-plugin` (exports
    /// `telemetry-source`; mirrors `flexcore-ports/src/telemetry_source.rs::TelemetrySource`).
    TelemetrySourcePlugin,
}

impl WorldId {
    /// Parses the world selector used by the binary root
    /// (`FLEXCORE_PLUGIN_HOST_WORLD`). The spelling is the WIT world name.
    pub fn parse(input: &str) -> Result<WorldId, HostError> {
        match input.trim() {
            "optimizer-plugin" => Ok(WorldId::OptimizerPlugin),
            "grid-model-provider-plugin" => Ok(WorldId::GridModelProviderPlugin),
            "telemetry-source-plugin" => Ok(WorldId::TelemetrySourcePlugin),
            other => Err(HostError::WorldUnknown { value: other.to_string() }),
        }
    }

    /// The WIT world name (the `world` declaration in the package).
    pub fn name(self) -> &'static str {
        match self {
            WorldId::OptimizerPlugin => "optimizer-plugin",
            WorldId::GridModelProviderPlugin => "grid-model-provider-plugin",
            WorldId::TelemetrySourcePlugin => "telemetry-source-plugin",
        }
    }

    /// The WIT package id, as declared inside the file (authoritative for
    /// package identity — directory names use a hyphen rendering on Windows,
    /// `wit/README.md` §1).
    pub fn package_id(self) -> &'static str {
        match self {
            WorldId::OptimizerPlugin => "flexcore:optimizer@1.0.0",
            WorldId::GridModelProviderPlugin => "flexcore:grid@1.0.0",
            WorldId::TelemetrySourcePlugin => "flexcore:telemetry@1.0.0",
        }
    }

    /// The WIT file path (repository-relative) the world is recorded in.
    pub fn wit_path(self) -> &'static str {
        match self {
            WorldId::OptimizerPlugin => "wit/flexcore-optimizer@1.0.0/optimizer.wit",
            WorldId::GridModelProviderPlugin => "wit/flexcore-grid@1.0.0/grid.wit",
            WorldId::TelemetrySourcePlugin => "wit/flexcore-telemetry@1.0.0/telemetry.wit",
        }
    }
}

impl fmt::Display for WorldId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// --- Errors -------------------------------------------------------------------

/// Host errors. Every failure is named; nothing is swallowed, and no error
/// implies a silent pass. The stable machine names ([`HostError::name`]) are
/// recorded in the decision record §5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// No component was configured (the binary root found no component path
    /// in the environment). The honest posture of this loop: the host RUNS
    /// ONLY when a human supplies a signed component — an unconfigured host
    /// refuses to run, it does not idle permissively.
    ComponentUnconfigured { what: String },
    /// The configured component artifact does not exist at the given path.
    ComponentNotFound { path: String },
    /// The artifact exists but could not be read (permissions, IO error).
    ComponentUnreadable { path: String, reason: String },
    /// The artifact exceeds [`MAX_COMPONENT_BYTES`] — refused before any
    /// parsing (the loader cap; the instance limits are separate).
    ComponentOversized { path: String, size_bytes: u64, limit_bytes: u64 },
    /// The build carries no generated bindings for the requested world —
    /// the default (fail-closed) state of this crate. Nothing can be
    /// instantiated until the human's wit-bindgen run materialises the
    /// bindings and the `generated-bindings` feature is compiled in.
    BindingsNotGenerated { world: String, symbol: String },
    /// The requested world selector is not one of the three recorded
    /// P7.09 worlds.
    WorldUnknown { value: String },
    /// A resource limit could not be constructed (a declared bound outside
    /// its closed range, an overflow in the page conversion, or a missing
    /// mandatory budget).
    LimitsInvalid { field: String, reason: String },
    /// The instance exceeded its memory page limit. Enforcement contract:
    /// the instance is torn down; the host process survives.
    MemoryLimitExceeded { limit_pages: u64, requested_pages: u64 },
    /// The instance exceeded its wall-clock deadline (wasmtime epoch-based
    /// interruption, [`crate::limits`] §epoch). Enforcement contract: the
    /// instance is torn down; the host process survives.
    DeadlineExceeded { deadline_ms: u64 },
    /// The instance exhausted its fuel (metered instruction) budget.
    /// Enforcement contract: the instance is torn down; the host process
    /// survives.
    FuelExhausted,
    /// A call's output exceeded the declared output-size limit.
    /// Enforcement contract: the call is refused and the instance torn
    /// down; the host process survives.
    OutputLimitExceeded { limit_bytes: u64 },
    /// The instance exceeded its call-rate limit.
    /// Enforcement contract: further calls are refused and the instance is
    /// torn down; the host process survives.
    CallRateExceeded { limit_per_minute: u64 },
}

impl HostError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            HostError::ComponentUnconfigured { .. } => "COMPONENT_UNCONFIGURED",
            HostError::ComponentNotFound { .. } => "COMPONENT_NOT_FOUND",
            HostError::ComponentUnreadable { .. } => "COMPONENT_UNREADABLE",
            HostError::ComponentOversized { .. } => "COMPONENT_OVERSIZED",
            HostError::BindingsNotGenerated { .. } => "BINDINGS_NOT_GENERATED",
            HostError::WorldUnknown { .. } => "WORLD_UNKNOWN",
            HostError::LimitsInvalid { .. } => "LIMITS_INVALID",
            HostError::MemoryLimitExceeded { .. } => "MEMORY_LIMIT_EXCEEDED",
            HostError::DeadlineExceeded { .. } => "DEADLINE_EXCEEDED",
            HostError::FuelExhausted => "FUEL_EXHAUSTED",
            HostError::OutputLimitExceeded { .. } => "OUTPUT_LIMIT_EXCEEDED",
            HostError::CallRateExceeded { .. } => "CALL_RATE_EXCEEDED",
        }
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostError::ComponentUnconfigured { what } => {
                write!(f, "no component configured: {what} (the host runs only when a human supplies a signed component; an unconfigured host refuses to run)")
            }
            HostError::ComponentNotFound { path } => {
                write!(f, "component artifact not found at '{path}' (loading only — there is no build step in the host; the artifact is produced by the human's cargo-component run)")
            }
            HostError::ComponentUnreadable { path, reason } => {
                write!(f, "component artifact at '{path}' could not be read: {reason}")
            }
            HostError::ComponentOversized { path, size_bytes, limit_bytes } => {
                write!(
                    f,
                    "component artifact at '{path}' is {size_bytes} bytes, above the {limit_bytes}-byte loader cap (refused before any parsing)"
                )
            }
            HostError::BindingsNotGenerated { world, symbol } => {
                write!(
                    f,
                    "no generated bindings exist for world '{world}' (symbol '{symbol}'): this build was compiled without the generated-bindings seam; enable it only together with the wit-bindgen output for the wit/ worlds"
                )
            }
            HostError::WorldUnknown { value } => {
                write!(
                    f,
                    "unknown world '{value}' (expected optimizer-plugin | grid-model-provider-plugin | telemetry-source-plugin)"
                )
            }
            HostError::LimitsInvalid { field, reason } => {
                write!(f, "resource limit '{field}' is invalid: {reason}")
            }
            HostError::MemoryLimitExceeded { limit_pages, requested_pages } => {
                write!(
                    f,
                    "memory limit exceeded: {requested_pages} pages requested against a {limit_pages}-page limit (instance torn down; host survives)"
                }
            )
            HostError::DeadlineExceeded { deadline_ms } => {
                write!(
                    f,
                    "wall-clock deadline of {deadline_ms} ms exceeded (epoch-based interruption; instance torn down, host survives)"
                )
            }
            HostError::FuelExhausted => {
                write!(f, "fuel (metered instruction) budget exhausted (instance torn down; host survives)")
            }
            HostError::OutputLimitExceeded { limit_bytes } => {
                write!(
                    f,
                    "output-size limit of {limit_bytes} bytes exceeded (call refused; instance torn down, host survives)"
                )
            }
            HostError::CallRateExceeded { limit_per_minute } => {
                write!(
                    f,
                    "call-rate limit of {limit_per_minute}/minute exceeded (further calls refused; instance torn down, host survives)"
                )
            }
        }
    }
}

impl std::error::Error for HostError {}

// --- Component artifact (loading, not building) --------------------------------

/// A loaded component artifact: the bytes of a `.wasm` component read from a
/// local file path, with its size recorded. The bytes are UNTRUSTED INPUT —
/// nothing in this struct parses or validates them; compilation happens
/// inside the compile-gated bindings seam only, by wasmtime, under the
/// instance resource limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentArtifact {
    path: PathBuf,
    size_bytes: u64,
}

impl ComponentArtifact {
    /// Loads a component artifact from a local file path. Fail-closed:
    ///
    /// - an empty path is [`HostError::ComponentNotFound`] (there is no
    ///   default component and none is invented);
    /// - a missing file is [`HostError::ComponentNotFound`] — every absent
    ///   path returns this named error (there are no artifacts in this
    ///   loop);
    /// - an artifact above [`MAX_COMPONENT_BYTES`] is
    ///   [`HostError::ComponentOversized`], refused before any read;
    /// - a failed read is [`HostError::ComponentUnreadable`].
    ///
    /// No `unwrap` on anything the filesystem produced.
    pub fn load(path: &Path) -> Result<ComponentArtifact, HostError> {
        if path.as_os_str().is_empty() {
            return Err(HostError::ComponentNotFound { path: String::new() });
        }
        let shown = path.display().to_string();
        let metadata = std::fs::metadata(path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                HostError::ComponentNotFound { path: shown.clone() }
            } else {
                HostError::ComponentUnreadable { path: shown.clone(), reason: err.to_string() }
            }
        })?;
        let size_bytes = metadata.len();
        if size_bytes > MAX_COMPONENT_BYTES {
            return Err(HostError::ComponentOversized {
                path: shown,
                size_bytes,
                limit_bytes: MAX_COMPONENT_BYTES,
            });
        }
        // The bytes are read only to prove readability and hand a length to
        // the seam; the seam re-reads/re-compiles under its own UNVERIFIED
        // wasmtime call sites. No content is parsed here.
        let file = std::fs::File::open(path).map_err(|err| {
            HostError::ComponentUnreadable { path: shown.clone(), reason: err.to_string() }
        })?;
        drop(file);
        Ok(ComponentArtifact { path: path.to_path_buf(), size_bytes })
    }

    /// The path the artifact was loaded from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The artifact size in bytes (as stated by the filesystem metadata).
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

// --- Binding -------------------------------------------------------------------

/// The host-side binding operation: attach a loaded artifact to a recorded
/// world and produce the [`PluginBinding`] for its class-B entry point
/// (the manifest's `symbol`, `crates/flexcore-kernel/src/manifest.rs`).
///
/// In the DEFAULT build this returns a fail-closed [`bindings::NoBindings`]
/// object whose every use is [`HostError::BindingsNotGenerated`]; the real
/// wasmtime instantiation exists only inside the `generated-bindings`
/// feature gate (see `crate::bindings` and the decision record §4). The
/// `limits` are recorded onto the binding request so the gated
/// implementation can apply them at store creation time — the limit
/// CONFIGURATION ([`crate::limits`]) is separate from and prior to any
/// instantiation.
///
// UNVERIFIED — wasmtime 50 API, first human build verifies: the gated
// implementation calls wasmtime's Component Model embedding API
// (`wasmtime::component::Component`, `Linker`, `Store::set_epoch_deadline`)
// at exactly this seam and nowhere else.
//
// P7.12: demoted from `pub` to `pub(crate)` (VISIBILITY-ONLY change; the
// body is untouched). The ONLY public constructor path to a binding is now
// [`crate::enforcement::instantiate`], which requires an enforcement
// verdict — no public API instantiates from raw manifest data.
pub(crate) fn bind_component(
    artifact: &ComponentArtifact,
    world: WorldId,
    symbol: &str,
    limits: &ResourceLimits,
) -> Result<Box<dyn PluginBinding>, HostError> {
    bindings::make_binding(world, symbol.to_string(), &artifact.path, limits)
}
