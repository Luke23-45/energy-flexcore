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
// Recorded adapter ids (D5 OPEN — ENTRY_NOTE boundary 1; decision doc §3;
// P14.18 decision record 82 updated the table):
//
// | id | status | meaning |
// |---|---|---|
// | `openems-rest` | **AVAILABLE (P6.10)** in the DEFAULT table | the RECOMMENDED default (OpenEMS first, report §2.3 priority). P6.10 landed the concrete EdgeConfig-discovery/read adapter (`adapters::openems`) against the UNVERIFIED OpenEMS REST API shape (RESEARCH.md §1 — never executed; the P6.11 lab run must verify it), so the id constructs through the registry. Coordinator follow-up: the DEFAULT table now carries `Implemented` directly and the pre-existing integration test `tests/adapter_acl_and_registry.rs` was updated in the same reviewed pass (decision record: `ledger/phase-06-edge-protocol/10_11_openems.md`). P6.13 (fault-injection tests against the real protocol) remains open. |
// | `generic-rest` | **REMOVED (P14.18, record 82)** | the Phase 6 RESERVED placeholder slot is GONE: the protocol-adapter tier (P14.01) now names concrete protocol ids, and the extension rule is "an id is registered when its implementation lands, never before". A permanent never-constructible placeholder was scaffolding without an owner (AGENTS.md §1: no crates/slots to be ready). History: it was RESERVED and refused construction through Phase 6–13. |
// | `simulated-test` | TEST-ONLY (P6.08) | the deterministic in-process scenario fixture (`adapters::simulated`). Constructible, but NEVER a real EMS and never presentable as one: its id names itself, its samples carry the id in their `source_id`, and a production composition root must not wire it for a real site. It exists so the read path (P6.08), the store-and-forward drills (P6.09), and the later failure-injection waves (P6.13/P6.28) have an executable test source without OpenEMS. |
// | `ieee-2030.5`, `openadr-vtn`, `modbus-client`, `dnp3-outstation`, `mqtt-sparkplug-uplink`, `cim-cgmes-import` | PLANNED (P14.18, record 82) | the Phase 14 protocol adapter ids, registered as PLANNED exactly per the P14.02 contract ("registered as PLANNED in the edge-agent registry only when the edge seam wave (P14.18) lands" — `protocol-20305` lib.rs). Each id's canonical spelling is its own crate's `ADAPTER_ID` constant; the concrete edge-side implementations are NOT built (construction is refused with the named wave reference, the honest fail-closed answer), and a PLANNED id never becomes Implemented by config edit — only by a reviewed code landing. |

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

impl std::fmt::Display for AdapterStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.describe())
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
    /// The default allowlist (P14.18 record 82 edition):
    /// `openems-rest` (**IMPLEMENTED** — P6.10 landed the concrete
    /// EdgeConfig-discovery/read adapter `adapters::openems`;
    /// P6.13's fault-injection pass against the real protocol remains
    /// open and is NOT implied by this status), `simulated-test`
    /// (TEST-ONLY, P6.08), and the six Phase 14 protocol adapter ids as
    /// PLANNED (their canonical spellings are the crates' own
    /// `ADAPTER_ID` constants; the concrete edge-side implementations
    /// are not built, so construction is refused with the named wave
    /// reference). The Phase 6 `generic-rest` RESERVED placeholder is
    /// REMOVED (record 82): ids are registered when their
    /// implementation lands, never before. Extending the allowlist is
    /// an explicit reviewed change (a new id needs a mapping-table plan
    /// and, per P6.02, a dependency-audit note for its new
    /// dependencies).
    pub fn with_default_allowlist() -> Self {
        let mut entries = BTreeMap::new();
        entries.insert("openems-rest".to_string(), AdapterStatus::Implemented);
        // P6.08: the deterministic in-process scenario fixture. TEST-ONLY:
        // never a real EMS, never presentable as one (see AdapterStatus::
        // TestOnly and the simulated module header). No production
        // composition root may wire this id for a real site.
        entries.insert("simulated-test".to_string(), AdapterStatus::TestOnly);
        // P14.18 (record 82): the Phase 14 protocol adapter ids,
        // registered PLANNED per the P14.02 contract. Canonical
        // spellings: each crate's `ADAPTER_ID` constant
        // (`crates/adapters/protocol-*/src/lib.rs`). Construction is
        // refused until the concrete edge-side implementation lands in a
        // reviewed change.
        entries.insert(
            "ieee-2030.5".to_string(),
            AdapterStatus::Planned { implementing_wave: "P14 (record 02; edge implementation unlanded)" },
        );
        entries.insert(
            "openadr-vtn".to_string(),
            AdapterStatus::Planned { implementing_wave: "P14 (record 21; edge implementation unlanded)" },
        );
        entries.insert(
            "modbus-client".to_string(),
            AdapterStatus::Planned { implementing_wave: "P14 (record 23; edge implementation unlanded)" },
        );
        entries.insert(
            "dnp3-outstation".to_string(),
            AdapterStatus::Planned { implementing_wave: "P14 (record 24; edge implementation unlanded)" },
        );
        entries.insert(
            "mqtt-sparkplug-uplink".to_string(),
            AdapterStatus::Planned { implementing_wave: "P14 (record 43; edge implementation unlanded)" },
        );
        entries.insert(
            "cim-cgmes-import".to_string(),
            AdapterStatus::Planned { implementing_wave: "P14 (record 44; edge implementation unlanded)" },
        );
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
    fn default_allowlist_records_openems_testonly_and_the_planned_protocol_ids() {
        let registry = AdapterRegistry::with_default_allowlist();
        assert!(registry.is_allowlisted("openems-rest"));
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
        // P14.18 (record 82): the six Phase 14 protocol ids are PLANNED
        // (registered, never constructible until their edge-side
        // implementations land).
        for id in [
            "ieee-2030.5",
            "openadr-vtn",
            "modbus-client",
            "dnp3-outstation",
            "mqtt-sparkplug-uplink",
            "cim-cgmes-import",
        ] {
            assert!(registry.is_allowlisted(id), "{id} is allowlisted");
            assert!(
                matches!(registry.status_of(id), Ok(AdapterStatus::Planned { .. })),
                "{id} is PLANNED"
            );
        }
        // P14.18 (record 82): the `generic-rest` RESERVED placeholder is
        // REMOVED — it is no longer allowlisted at all (an id is
        // registered when its implementation lands, never before).
        assert!(!registry.is_allowlisted("generic-rest"));
        assert_eq!(
            registry.status_of("generic-rest"),
            Err(RegistryError::NotAllowlisted { id: "generic-rest".to_string() })
        );
        assert_eq!(registry.allowlist().len(), 8);
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
        // `modbus-tcp` is NOT the registered Modbus id (the registered
        // one is `modbus-client`, PLANNED since P14.18).
        assert!(!registry.is_allowlisted("modbus-tcp"));
        assert_eq!(
            registry.status_of("modbus-tcp"),
            Err(RegistryError::NotAllowlisted {
                id: "modbus-tcp".to_string()
            })
        );
        // The removed `generic-rest` placeholder is not allowlisted either.
        assert!(!registry.is_allowlisted("generic-rest"));
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
        let error = match registry.construct("legacy-planned", &factory) {
            Ok(_) => panic!("a planned id must never construct"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("P6.10/P6.13"));
        // The DEFAULT table's own PLANNED ids refuse the same way (P14.18:
        // the six Phase 14 protocol ids are registered but unlanded).
        let registry_default = AdapterRegistry::with_default_allowlist();
        let error = match registry_default.construct("ieee-2030.5", &factory) {
            Ok(_) => panic!("a planned id must never construct"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("P14"), "the refusal names the wave");
        // The REMOVED `generic-rest` id is not allowlisted at all.
        let error = match registry_default.construct("generic-rest", &factory) {
            Ok(_) => panic!("a removed id must never construct"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("not in the allowlist"));
        // An unknown id: named refusal before any factory would run.
        let error = match registry.construct("modbus-tcp", &factory) {
            Ok(_) => panic!("an unknown id must never construct"),
            Err(error) => error,
        };
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
