// P6.12 — The adapter registry: the code-level allowlist (report §9.1,
// §5.3; ledger P6.12).
//
// Allowlist rule: only a REGISTERED adapter id can be composed or
// constructed. An unregistered id is a named error — never a fallback, and
// never a dynamically discovered adapter (no arbitrary plugin loading; the
// extensibility boundary is WASM/WIT or out-of-process gRPC per ADR-006,
// not dylibs and not runtime reflection).
//
// The registry here is the MECHANISM. The enforcement points that feed it:
//
// - the edge deployment configuration (`FLEXCORE_EDGE_ADAPTER_ID` must be
//   allowlisted before the runner starts — `crates/edge-agent/src/main.rs`),
// - P6.16's per-site allowlist config (which adapters may RECEIVE
//   commands, when the gated path exists at all),
// - the deployment profile (P6.20): the shipped profile carries the
//   allowlist, so an unregistered adapter id cannot be introduced by a
//   config edit alone.
//
// Recorded adapter ids (D5 OPEN — ENTRY_NOTE boundary 1; decision doc §3):
//
// | id | status | meaning |
// |---|---|---|
// | `openems-rest` | **AVAILABLE (P6.10)** in the DEFAULT table | the RECOMMENDED default (OpenEMS first, report §2.3 priority). P6.10 landed the concrete EdgeConfig-discovery/read adapter (`adapters::openems`) against the UNVERIFIED OpenEMS REST API shape (RESEARCH.md §1 — never executed; the P6.11 lab run must verify it), so the id constructs through the registry. Coordinator follow-up: the DEFAULT table now carries `Implemented` directly and the pre-existing integration test `tests/adapter_acl_and_registry.rs` was updated in the same reviewed pass (decision record: `ledger/phase-06-edge-protocol/10_11_openems.md`). P6.13 (fault-injection tests against the real protocol) remains open. |
// | `generic-rest` | RESERVED | a recorded placeholder SLOT for a second adapter. It is deliberately NOT implemented and NOT constructible — recording a slot does not imply an API exists (AGENTS.md rule 1: no hallucinated APIs). |
// | `simulated-test` | TEST-ONLY (P6.08) | the deterministic in-process scenario fixture (`adapters::simulated`). Constructible, but NEVER a real EMS and never presentable as one: its id names itself, its samples carry the id in their `source_id`, and a production composition root must not wire it for a real site. It exists so the read path (P6.08), the store-and-forward drills (P6.09), and the later failure-injection waves (P6.13/P6.28) have an executable test source without OpenEMS. |

use std::collections::BTreeMap;

/// The registered OpenEMS REST adapter id (the concrete implementation is
/// `adapters::openems::OpenEmsRestAdapter`; P6.10).
const OPENEMS_REST_ID: &str = "openems-rest";

/// The build/composition status of a registered adapter id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterStatus {
    /// A concrete implementation exists and can be constructed.
    Implemented,
    /// The id is registered ahead of its implementation (so composition
    /// checks and profiles can reference it); construction is refused until
    /// the implementing wave lands.
    Planned {
        /// The ledger waves that implement it.
        implementing_wave: &'static str,
    },
    /// A recorded placeholder slot — deliberately never implemented in this
    /// phase; construction is always refused.
    Reserved,
    /// (P6.08) A TEST/simulation-only adapter: constructible, but NEVER a
    /// real EMS and never presentable as one. Its id names itself
    /// (`simulated-test`), its samples carry the id in their `source_id`,
    /// and a production composition root must not wire it for a real site
    /// (P6.25 must surface simulated values visually distinct, report
    /// §20.2). It exists so the read path, store-and-forward drills, and
    /// later failure-injection waves have an executable, deterministic,
    /// in-process source — no OpenEMS required.
    TestOnly,
}

impl AdapterStatus {
    /// Human-readable status (honest logging).
    pub fn describe(self) -> &'static str {
        match self {
            AdapterStatus::Implemented => "IMPLEMENTED",
            AdapterStatus::Planned { .. } => "PLANNED",
            AdapterStatus::Reserved => "RESERVED (not implemented; placeholder slot)",
            AdapterStatus::TestOnly => {
                "TEST-ONLY (simulation fixture; NEVER a real EMS — simulation/test-lab use only)"
            }
        }
    }
}

/// Registry errors. Both are named and both refuse — nothing about an
/// unknown or not-yet-implemented adapter id passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The id is not in the allowlist. Only registered ids can be composed;
    /// there is no discovery fallback.
    NotAllowlisted { id: String },
    /// The id IS allowlisted but has no implementation yet (PLANNED or
    /// RESERVED). The error names the recorded status so the log explains
    /// itself.
    NotYetImplemented { id: String, status: &'static str },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::NotAllowlisted { id } => {
                write!(f, "adapter id '{id}' is not in the allowlist; only registered adapter ids can be composed")
            }
            RegistryError::NotYetImplemented { id, status } => {
                write!(f, "adapter id '{id}' is allowlisted but not implemented ({status}); construction is refused until its implementing wave lands")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// The adapter registry (the allowlist).
#[derive(Debug, Clone)]
pub struct AdapterRegistry {
    entries: BTreeMap<String, AdapterStatus>,
}

impl AdapterRegistry {
    /// The default Phase 6 allowlist: `openems-rest` (**IMPLEMENTED** —
    /// P6.10 landed the concrete EdgeConfig-discovery/read adapter
    /// `adapters::openems`; P6.13's fault-injection pass against the real
    /// protocol remains open and is NOT implied by this status),
    /// `generic-rest` (RESERVED), and `simulated-test` (TEST-ONLY, P6.08).
    /// Extending the allowlist is an explicit reviewed change (a new id
    /// needs a mapping-table plan and, per P6.02, a dependency-audit note
    /// for its new dependencies).
    pub fn with_default_allowlist() -> Self {
        let mut entries = BTreeMap::new();
        entries.insert("openems-rest".to_string(), AdapterStatus::Implemented);
        entries.insert("generic-rest".to_string(), AdapterStatus::Reserved);
        // P6.08: the deterministic in-process scenario fixture. TEST-ONLY:
        // never a real EMS, never presentable as one (see AdapterStatus::
        // TestOnly and the simulated module header). No production
        // composition root may wire this id for a real site.
        entries.insert("simulated-test".to_string(), AdapterStatus::TestOnly);
        Self { entries }
    }

    /// Whether the id is allowlisted.
    pub fn is_allowlisted(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// P6.10 — flips `openems-rest` to [`AdapterStatus::Implemented`]:
    /// the concrete EdgeConfig-discovery/read adapter
    /// (`adapters::openems::OpenEmsRestAdapter`) landed, so the id
    /// constructs through the registry. The composition root opts in:
    ///
    /// ```text
    /// AdapterRegistry::with_default_allowlist().with_openems_available()
    /// ```
    ///
    /// Coordinator follow-up note: the DEFAULT table itself now carries
    /// `openems-rest` as [`AdapterStatus::Implemented`] (the original
    /// PLANNED pin lived in `tests/adapter_acl_and_registry.rs`, updated
    /// in the same reviewed pass). This builder remains because the
    /// recorded P6.10 composition roots call it; it is now an idempotent
    /// re-registration. P6.13 (fault-injection tests against the real
    /// protocol) remains open and is NOT implied by this status.
    pub fn with_openems_available(mut self) -> Self {
        self.register(OPENEMS_REST_ID, AdapterStatus::Implemented);
        self
    }

    /// Registers an id with its status. Composition/testing seam: the
    /// SHIPPED profile freezes the default allowlist (P6.20); this method
    /// exists so a wave that lands a concrete adapter (P6.10/P6.13) flips
    /// its id to [`AdapterStatus::Implemented`] in one reviewed edit, and
    /// so tests can prove the Implemented arm without a real adapter.
    pub fn register(&mut self, id: &str, status: AdapterStatus) {
        self.entries.insert(id.to_string(), status);
    }

    /// The recorded status of an allowlisted id, or
    /// [`RegistryError::NotAllowlisted`].
    pub fn status_of(&self, id: &str) -> Result<AdapterStatus, RegistryError> {
        self.entries
            .get(id)
            .copied()
            .ok_or_else(|| RegistryError::NotAllowlisted {
                id: id.to_string(),
            })
    }

    /// Constructs the concrete adapter for an id. Since P6.10 the
    /// default allowlist carries one `Implemented` entry (`openems-rest`)
    /// and one constructible TEST-ONLY entry (`simulated-test`); PLANNED
    /// and RESERVED ids refuse with [`RegistryError::NotYetImplemented`]
    /// — the honest, fail-closed answer — and the factory never runs for
    /// a refused id.
    pub fn construct(
        &self,
        id: &str,
        factory: &dyn Fn() -> Result<Box<dyn crate::adapters::port::EmsAdapter>, String>,
    ) -> Result<Box<dyn crate::adapters::port::EmsAdapter>, RegistryError> {
        let status = self.status_of(id)?;
        match status {
            AdapterStatus::Implemented | AdapterStatus::TestOnly => {
                factory().map_err(|reason| {
                    // A failed constructor is a composition failure, not a
                    // silent skip; the registry re-raises it as a
                    // not-implemented refusal carrying the reason.
                    tracing::error!(adapter_id = %id, %reason, "adapter constructor failed");
                    RegistryError::NotYetImplemented {
                        id: id.to_string(),
                        status: "constructor failed (see log)",
                    }
                })
            }
            AdapterStatus::Planned { implementing_wave } => {
                Err(RegistryError::NotYetImplemented {
                    id: id.to_string(),
                    status: implementing_wave,
                })
            }
            AdapterStatus::Reserved => Err(RegistryError::NotYetImplemented {
                id: id.to_string(),
                status: AdapterStatus::Reserved.describe(),
            }),
        }
    }

    /// The allowlist snapshot, ordered by id (diagnostics/health).
    pub fn allowlist(&self) -> Vec<(&str, AdapterStatus)> {
        self.entries
            .iter()
            .map(|(id, status)| (id.as_str(), *status))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_allowlist_records_openems_reserved_and_testonly_slots() {
        let registry = AdapterRegistry::with_default_allowlist();
        assert!(registry.is_allowlisted("openems-rest"));
        assert!(registry.is_allowlisted("generic-rest"));
        // P6.08: the simulated fixture is allowlisted, explicitly TEST-ONLY.
        assert!(registry.is_allowlisted("simulated-test"));
        assert_eq!(registry.status_of("simulated-test"), Ok(AdapterStatus::TestOnly));
        assert_eq!(
            AdapterStatus::TestOnly.describe(),
            "TEST-ONLY (simulation fixture; NEVER a real EMS — simulation/test-lab use only)"
        );
        assert_eq!(
            registry.status_of("openems-rest"),
            Ok(AdapterStatus::Implemented)
        );
        assert_eq!(registry.status_of("generic-rest"), Ok(AdapterStatus::Reserved));
        assert_eq!(registry.allowlist().len(), 3);
    }

    #[test]
    fn testonly_ids_construct_through_the_factory_like_implemented_ones() {
        // The TestOnly arm constructs through the factory (the read-path
        // tests compose the simulated fixture exactly this way); the
        // TEST-ONLY discipline is carried by the id, the describe() text,
        // and the sample source_ids — not by refusing construction.
        struct StandInAdapter;

        impl crate::adapters::port::EmsAdapter for StandInAdapter {
            fn id(&self) -> &str {
                "simulated-test"
            }
            fn capabilities(&self) -> &crate::adapters::port::AdapterCapabilities {
                unreachable!("stand-in adapter is never read in this test")
            }
            fn discover(
                &mut self,
            ) -> Result<Vec<crate::adapters::port::ChannelDescriptor>, crate::adapters::port::AdapterError>
            {
                unreachable!("stand-in adapter is never read in this test")
            }
            fn read(
                &mut self,
                _point: &crate::adapters::port::PointKey,
            ) -> Result<crate::adapters::port::EmsNativeReading, crate::adapters::port::AdapterError>
            {
                unreachable!("stand-in adapter is never read in this test")
            }
            fn health(&self) -> crate::adapters::port::AdapterHealth {
                unreachable!("stand-in adapter is never read in this test")
            }
        }

        let registry = AdapterRegistry::with_default_allowlist();
        let adapter = registry
            .construct(
                "simulated-test",
                &|| Ok(Box::new(StandInAdapter) as Box<dyn crate::adapters::port::EmsAdapter>),
            )
            .expect("the TEST-ONLY id constructs");
        assert_eq!(adapter.id(), "simulated-test");
    }

    #[test]
    fn unknown_id_is_not_allowlisted_named_error() {
        let registry = AdapterRegistry::with_default_allowlist();
        assert!(!registry.is_allowlisted("modbus-tcp"));
        assert_eq!(
            registry.status_of("modbus-tcp"),
            Err(RegistryError::NotAllowlisted {
                id: "modbus-tcp".to_string()
            })
        );
    }

    #[test]
    fn planned_and_reserved_ids_refuse_construction() {
        // Since the P6.10 DEFAULT flip, `openems-rest` is Implemented; the
        // PLANNED refusal arm is exercised through the documented `register`
        // seam (a wave that lands a concrete adapter flips its id in one
        // reviewed edit — and a test proves the refused arm the same way).
        let mut registry = AdapterRegistry::with_default_allowlist();
        registry.register(
            "legacy-planned",
            AdapterStatus::Planned {
                implementing_wave: "P6.10/P6.13",
            },
        );
        let factory = || -> Result<Box<dyn crate::adapters::port::EmsAdapter>, String> {
            panic!("the factory must never be called for a refused id")
        };
        // A PLANNED id: allowlisted → named refusal, factory NOT called.
        let error = registry.construct("legacy-planned", &factory).unwrap_err();
        assert!(error.to_string().contains("P6.10/P6.13"));
        // generic-rest: allowlisted, RESERVED → named refusal.
        let error = registry.construct("generic-rest", &factory).unwrap_err();
        assert!(error.to_string().contains("RESERVED"));
        // An unknown id: named refusal before any factory would run.
        let error = registry.construct("modbus-tcp", &factory).unwrap_err();
        assert!(error.to_string().contains("not in the allowlist"));
    }

    #[test]
    fn implemented_ids_construct_through_the_factory() {
        // The Implemented arm is exercised with a stand-in adapter so the
        // P6.10 change is a table edit, not a redesign (recorded in the
        // decision doc).
        struct StandInAdapter;

        impl crate::adapters::port::EmsAdapter for StandInAdapter {
            fn id(&self) -> &str {
                "stand-in"
            }
            fn capabilities(&self) -> &crate::adapters::port::AdapterCapabilities {
                unreachable!("stand-in adapter is never read in this test")
            }
            fn discover(
                &mut self,
            ) -> Result<Vec<crate::adapters::port::ChannelDescriptor>, crate::adapters::port::AdapterError>
            {
                unreachable!("stand-in adapter is never read in this test")
            }
            fn read(
                &mut self,
                _point: &crate::adapters::port::PointKey,
            ) -> Result<crate::adapters::port::EmsNativeReading, crate::adapters::port::AdapterError>
            {
                unreachable!("stand-in adapter is never read in this test")
            }
            fn health(&self) -> crate::adapters::port::AdapterHealth {
                unreachable!("stand-in adapter is never read in this test")
            }
        }

        let mut registry = AdapterRegistry::with_default_allowlist();
        registry.entries.insert(
            "stand-in".to_string(),
            AdapterStatus::Implemented,
        );
        let adapter = registry
            .construct("stand-in", &|| Ok(Box::new(StandInAdapter) as Box<dyn crate::adapters::port::EmsAdapter>))
            .expect("constructed");
        assert_eq!(adapter.id(), "stand-in");
    }
}
