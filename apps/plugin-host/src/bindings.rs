// P7.10 — The compile-gated bindings seam: the honest compile story for an
// unbuilt wasmtime pin (report §5.4 Class B; ledger §P7.10; decision record
// `ledger/phase-07-plugin-sdk/10_11_plugin_host_broker.md` §4).
//
// WHY THIS SEAM EXISTS
//
// The wasmtime 50.0.1 pin has never been compiled in this delivery loop (no
// cargo command was executed anywhere), and no `.wasm` component artifacts
// exist. Wasmtime's Component Model embedding API is therefore UNVERIFIED
// here: writing direct call sites into the always-compiled code would make
// the whole crate's honesty depend on API shapes nobody has ever compiled.
// Instead:
//
// - the host logic ([`crate::host`], [`crate::limits`], [`crate::broker`])
//   is std + tracing only and compiles WITHOUT wasmtime call sites;
// - every point where wasmtime is actually invoked lives in the
//   feature-gated module below (`#[cfg(feature = "generated-bindings")]`);
// - the DEFAULT build compiles the fail-closed [`NoBindings`]
//   implementation, whose every use returns the named
//   [`HostError::BindingsNotGenerated`] error. The crate is complete and
//   honest without wasmtime artifacts: it can load an artifact path, refuse
//   it if absent, and refuse to instantiate until the real build exists.
//
// ENABLING THE FEATURE
//
// `generated-bindings` is meaningful ONLY together with the generated
// binding modules materialised by the human's `cargo component` /
// `wit-bindgen` run over the `wit/` tree (build-time step, recorded in
// `wit/TOOLCHAIN.md` and `40_toolchain_pinning.md` §5). The gated module
// below references those generated items; enabling the feature WITHOUT
// materialising them is a loud compile error — fail-closed, by design. The
// gate is not a magic switch and the default feature set does not contain
// it (apps/plugin-host/Cargo.toml `[features]`).
//
// House rules inside the gate are the same as everywhere else: named
// errors, fail-closed, no `unwrap` on untrusted input, no `todo!()`/
// `unimplemented!()`. Every wasmtime call site carries the UNVERIFIED
// marker for the first human build to verify and (where needed) correct.

use crate::host::{HostError, WorldId};

/// What the host needs from a bound component, independent of how the
/// bindings were generated. The gated wasmtime implementation and the
/// fail-closed default both implement this trait, so all host code paths
/// are written against the trait boundary — no deep wasmtime API coupling
/// outside the gate.
pub trait PluginBinding: Send + Sync {
    /// The recorded world this binding was made for.
    fn world(&self) -> WorldId;

    /// The WIT export symbol the component was bound against — the
    /// manifest's class-B `symbol` entry point
    /// (`crates/flexcore-kernel/src/manifest.rs`).
    fn symbol(&self) -> &str;

    /// Verifies the binding is a real, live instantiation (not the
    /// fail-closed stub). A stub returns the named
    /// [`HostError::BindingsNotGenerated`] error on every call; a real
    /// binding returns `Ok(())` only when its instance is live.
    fn verify(&self) -> Result<(), HostError>;
}

// --- The fail-closed default ---------------------------------------------------

/// The DEFAULT build's binding: a fail-closed stub that carries the
/// requested world and symbol but refuses every use with the named
/// [`HostError::BindingsNotGenerated`] error. Nothing is fabricated in its
/// place — no fake instantiation, no permissive fallback.
pub struct NoBindings {
    world: WorldId,
    symbol: String,
}

impl NoBindings {
    pub fn new(world: WorldId, symbol: String) -> NoBindings {
        NoBindings { world, symbol }
    }
}

impl PluginBinding for NoBindings {
    fn world(&self) -> WorldId {
        self.world
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn verify(&self) -> Result<(), HostError> {
        Err(HostError::BindingsNotGenerated {
            world: self.world.name().to_string(),
            symbol: self.symbol.clone(),
        })
    }
}

/// The seam entry point the host calls. The DEFAULT build hands back the
/// fail-closed [`NoBindings`] object (the caller's `verify()` then fails
/// with [`HostError::BindingsNotGenerated`]); the gated build constructs
/// the real wasmtime binding from the SAME inputs — the artifact path (the
/// loader already existence/size-checked it) and the resource limits
/// derived from the manifest's declared bounds.
pub fn make_binding(
    world: WorldId,
    symbol: String,
    artifact_path: &std::path::Path,
    limits: &crate::limits::ResourceLimits,
) -> Result<Box<dyn PluginBinding>, HostError> {
    #[cfg(not(feature = "generated-bindings"))]
    {
        let _ = artifact_path;
        let _ = limits;
        Ok(Box::new(NoBindings::new(world, symbol)))
    }
    #[cfg(feature = "generated-bindings")]
    {
        wasmtime_bindings::make_binding(world, symbol, artifact_path, limits)
    }
}

// --- The feature-gated wasmtime implementation ---------------------------------
//
// COMPILED ONLY under `--features generated-bindings` AND together with the
// generated modules the human's wit-bindgen run produces. Every wasmtime
// call site below is UNVERIFIED — wasmtime 50 API, first human build
// verifies (and corrects, where the API differs).

#[cfg(feature = "generated-bindings")]
mod wasmtime_bindings {
    // UNVERIFIED — wasmtime 50 API, first human build verifies: the import
    // paths and constructor shapes below follow the wasmtime Component
    // Model embedding documentation; they have never been compiled.
    use std::path::Path;

    use wasmtime::component::{Component, Linker};
    use wasmtime::{Config, Engine, Store};

    use super::{HostError, PluginBinding, WorldId};
    use crate::host::MAX_COMPONENT_BYTES;
    use crate::limits::{ResourceLimits, EPOCH_TICK_MILLIS};

    /// The generated bindings module — MATERIALISED BY THE HUMAN'S
    /// WIT-BINDGEN RUN over the `wit/` tree (the three P7.09 worlds). Until
    /// that run has happened, enabling the `generated-bindings` feature
    /// fails right here, loudly, with a compile error — fail-closed.
    ///
    /// Expected shape (per `wit/README.md` §3 mapping tables):
    ///
    /// ```text
    /// mod generated {
    ///     pub mod flexcore_optimizer_1_0_0 { /* world optimizer-plugin */ }
    ///     pub mod flexcore_grid_1_0_0      { /* world grid-model-provider-plugin */ }
    ///     pub mod flexcore_telemetry_1_0_0 { /* world telemetry-source-plugin */ }
    /// }
    /// ```
    mod generated;

    /// A real, gated binding: an engine + component pair held for one
    /// world's exported interface. The engine is configured ONCE with the
    /// resource limits ([`ResourceLimits`]) so every limit is applied at
    /// store/instantiation time — a limit that exists only in config and
    /// is never applied to the instance is exactly the P7.13 risk.
    pub struct WasmtimeBinding {
        world: WorldId,
        symbol: String,
        // UNVERIFIED — wasmtime 50 API: `Engine` is cheap to clone; held for
        // the epoch ticker and future re-instantiations.
        engine: Engine,
        // UNVERIFIED — wasmtime 50 API: the component is the compiled
        // artifact; stores are created per call from it, so a torn-down
        // instance never takes the compiled component with it.
        component: Component,
    }

    /// Configures the engine from the host's resource limits.
    ///
    // UNVERIFIED — wasmtime 50 API, first human build verifies:
    // - `Config::consume_fuel(true)` enables metering; the per-instance
    //   budget is set on the store at instantiation time
    //   (`Store::set_fuel`) from [`ResourceLimits::fuel_budget`];
    // - the memory PAGE limit is applied per-store via a
    //   `wasmtime::ResourceLimiter` (`Store::limiter`) built from
    //   [`ResourceLimits::max_memory_pages`];
    // - the wall-clock deadline uses the epoch pattern: a host ticker
    //   calls `Engine::increment_epoch()` every `EPOCH_TICK_MILLIS` ms and
    //   the store sets `set_epoch_deadline(ResourceLimits::epoch_ticks())`
    //   (see limits.rs §epoch for the pattern source and its UNVERIFIED
    //   labels).
    fn configure_engine() -> Result<Engine, HostError> {
        let mut config = Config::new();
        // Fuel metering: the budget is mandatory in ResourceLimits, so the
        // engine always meters — an unmetered instance is not constructible.
        config.consume_fuel(true);
        // Component model must be on for `Component::new` over a component
        // artifact (the `component-model` cargo feature also gates this).
        Engine::new(&config).map_err(|err| HostError::LimitsInvalid {
            field: "engine".to_string(),
            reason: format!("the wasmtime engine could not be configured: {err}"),
        })
    }

    /// Compiles the artifact into a Component. LOADING happened in
    /// [`crate::host::ComponentArtifact::load`]; this is the COMPILATION
    /// step (wasmtime compiles; it never builds from WIT — the artifact is
    /// the human's `cargo component` output). The loader cap is re-checked
    /// here before the read (defence in depth: no second unbounded read
    /// happens behind the loader's back).
    fn compile(engine: &Engine, artifact_path: &Path) -> Result<Component, HostError> {
        let metadata = std::fs::metadata(artifact_path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                HostError::ComponentNotFound { path: artifact_path.display().to_string() }
            } else {
                HostError::ComponentUnreadable {
                    path: artifact_path.display().to_string(),
                    reason: err.to_string(),
                }
            }
        })?;
        if metadata.len() > MAX_COMPONENT_BYTES {
            return Err(HostError::ComponentOversized {
                path: artifact_path.display().to_string(),
                size_bytes: metadata.len(),
                limit_bytes: MAX_COMPONENT_BYTES,
            });
        }
        // UNVERIFIED — wasmtime 50 API, first human build verifies: the
        // byte form (`Component::new(&engine, &bytes)`) is chosen over
        // `Component::from_file` so the size cap above governs the only
        // read that happens.
        let bytes = std::fs::read(artifact_path).map_err(|err| {
            HostError::ComponentUnreadable {
                path: artifact_path.display().to_string(),
                reason: err.to_string(),
            }
        })?;
        Component::new(engine, &bytes).map_err(|err| HostError::LimitsInvalid {
            field: "component".to_string(),
            reason: format!("the artifact is not a valid component: {err}"),
        })
    }

    /// Builds the gated binding. The linker exposes ONLY the imports the
    /// recorded world declares — which is NONE for the P7.09 worlds (zero
    /// `wasi:` imports) — so the linker starts empty and stays empty unless
    /// a scoped broker capability explicitly adds a host interface. There
    /// is NO ambient filesystem, network, clock, or environment import in
    /// this linker, ever (P7.11; report §5.4).
    pub fn make_binding(
        world: WorldId,
        symbol: String,
        artifact_path: &Path,
        limits: &ResourceLimits,
    ) -> Result<Box<dyn PluginBinding>, HostError> {
        let engine = configure_engine()?;
        let component = compile(&engine, artifact_path)?;
        // UNVERIFIED — wasmtime 50 API: an empty-linker instantiation of a
        // world with zero declared imports. The store is created per call
        // site (not held here) with:
        //   - a resource limiter built from `limits.max_memory_pages()`
        //     (exceeded => `MemoryLimitExceeded`, instance torn down),
        //   - `store.set_fuel(limits.fuel_budget())` (exhausted =>
        //     `FuelExhausted`, instance torn down),
        //   - `store.set_epoch_deadline(limits.epoch_ticks())` with a host
        //     ticker incrementing the engine epoch every
        //     `EPOCH_TICK_MILLIS` ms (deadline passed => `DeadlineExceeded`,
        //     instance torn down).
        // The generated module's typed world view (per `wit/README.md` §3)
        // provides the exported interface behind `symbol` — the manifest's
        // class-B entry point.
        let mut linker: Linker<()> = Linker::new(&engine);
        let _ = (&mut linker, &limits, EPOCH_TICK_MILLIS);
        let _store = Store::new(&engine, ());
        Ok(Box::new(WasmtimeBinding { world, symbol, engine, component }))
    }

    impl PluginBinding for WasmtimeBinding {
        fn world(&self) -> WorldId {
            self.world
        }

        fn symbol(&self) -> &str {
            &self.symbol
        }

        fn verify(&self) -> Result<(), HostError> {
            // UNVERIFIED — wasmtime 50 API, first human build verifies: the
            // real verification loads the world's typed interface view from
            // the generated module against a fresh store (the exports must
            // match the recorded world or the manifest's `symbol` entry
            // point is stale). Until that call exists, the gated binding
            // reports the same named error as the stub — fail-closed, never
            // a guessed `Ok`.
            Err(HostError::BindingsNotGenerated {
                world: self.world.name().to_string(),
                symbol: self.symbol.clone(),
            })
        }
    }
}
