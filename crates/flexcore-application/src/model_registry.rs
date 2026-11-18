// P15.06 — The model registry and forecast-artifact versioning: versioned
//         + digest-chained artifacts and models, model→artifact lineage,
//         replay determinism, UNTRAINED as a first-class registered state,
//         and the registry-change-is-a-new-version rule.
//
// Phase 15 plan §2 row P15.06; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 06_p15_06_model_registry.md`.
//
// WHAT THIS IS: the in-application registry that binds forecast-artifact
// and model identity: every registration carries its content digest, the
// digest of the PREVIOUS registry head (a chain — a removal or an edit
// breaks the chain visibly), the model→artifact lineage (which models
// produced which artifacts, from which input data), and the training
// state. It mirrors the flexcore-research patterns BY CITATION (no
// flexcore-research dependency — this crate's manifest policy): the
// P8.32 provenance-record shape (module header of
// `crates/flexcore-application/src/forecast_architecture.rs`) and the
// P8.06 version-stamped frozen-registry discipline
// (`crates/flexcore-research/src/baselines.rs`).
//
// THE RULES (each enforced structurally):
//
// 1. **A registry change is a NEW version, never a silent edit.** The
//    registry is APPEND-ONLY: registering a ref that already exists is a
//    named refusal; there is no update, no remove, no re-registration.
//    The registry's VERSION constant bumps when the registry SHAPE
//    changes (a field added, a vocabulary extended); a CONTENT change is
//    a new APPENDED record — never an in-place edit (the P8.06
//    frozen-version discipline; `benchmarks/research-set-v1.json`
//    anti-silent-growth rule).
// 2. **UNTRAINED is a first-class registered state.** Every model in
//    this delivery registers [`TrainingState::Untrained`] — no runtime
//    data exists (the platform has never run), so no model can be
//    trained, and no training run is claimed. `Trained` exists in the
//    vocabulary so the future state is a REGISTERED, versioned
//    transition (with its training-data reference REQUIRED — provenance
//    linked to the state, the P8.32 rule), never a silent relabel.
//    `Retired` is the append-only safe direction (the kernel
//    UNREVOCATION_SEAM posture: a retirement cannot silently disappear;
//    there is no un-retire).
// 3. **Digest chain.** Every entry carries the FNV-1a 64 (test-grade,
//    NOT a security primitive — the production digest is a DEPLOYMENT
//    REQUIREMENT, the P8.32 posture) content digest of its canonical
//    description and the PREVIOUS entry's digest; the head digest is the
//    chain's integrity check, and replaying the same registrations in
//    the same order reproduces the same head (replay determinism).
// 4. **Replay determinism.** Entries are held in a `BTreeMap` keyed by
//    ref (deterministic iteration), timestamps are EXPLICIT caller-
//    supplied parameters (the injected-clock posture; no `SystemTime`
//    anywhere), and there is no randomness and no wall clock in any
//    registry computation. Replaying the same registration sequence
//    yields a byte-identical registry state and the same head digest.
// 5. **Model→artifact lineage is required, not optional.** A model
//    entry names the artifact kinds it produces; an artifact entry names
//    the model ref that produced it (cross-references are validated at
//    registration: a lineage ref that names a registry entry but nothing
//    registered is refused — lineage cannot silently dangle; an
//    `input://` ref names external input data outside the registry and
//    is exempt from resolution, so a first entry can chain from its
//    input data).
//
// Control status: registry records for the simulation-only 0.x line
// (ADR-010 extension, conventions §6.3). Nothing here authorizes, issues,
// or dispatches anything; a registered model is a RECORD about a
// forecaster, never a capability to command. No skill, accuracy, or
// training claim is made for any registered model.

use std::collections::BTreeMap;
use std::fmt;

/// The registry version: the version of the REGISTRY SHAPE as a whole
/// (adding a field or a vocabulary member is a registry version bump;
/// a content change is a new appended record — never silent).
pub const MODEL_REGISTRY_VERSION: &str = "flexcore-application/model-registry.v1";

/// The recorded registry statement (pinned by tests).
pub const MODEL_REGISTRY_STATEMENT: &str = "the model registry is append-only and \
     digest-chained: a registry change is a NEW version or a NEW appended record, never a \
     silent edit; UNTRAINED is a first-class registered state and every model in this \
     delivery registers UNTRAINED; a Trained transition requires a training-data \
     reference linked to the state; Retired is append-only with no un-retire; lineage \
     references are validated at registration and cannot dangle; replaying the same \
     registrations reproduces the same head digest (determinism). The registry records \
     NO skill, accuracy, or training claim for any model.";

/// The registry entry kinds (closed). Forecast artifacts and models are
/// distinct entry kinds — an artifact is a produced series record; a
/// model is a producing-forecaster record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryEntryKind {
    /// A forecast artifact (a versioned, provenance-signed series; the
    /// P15.01 artifact vocabulary).
    ForecastArtifact,
    /// A forecast model (a statistical baseline or a designed/ML model).
    ForecastModel,
}

impl RegistryEntryKind {
    /// The wire spelling of the kind.
    pub const fn wire_name(self) -> &'static str {
        match self {
            RegistryEntryKind::ForecastArtifact => "forecast_artifact",
            RegistryEntryKind::ForecastModel => "forecast_model",
        }
    }
}

/// The training state of a registered model (closed; UNTRAINED is the
/// state every model in this delivery registers — a first-class state,
/// not an absence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrainingState {
    /// Registered, never trained: no runtime data exists; no training run
    /// is claimed. THE state of every model in this delivery.
    Untrained,
    /// Trained, with the training-data reference REQUIRED (provenance
    /// linked to the state — the P8.32 rule) and the training timestamp
    /// recorded from the caller's injected clock.
    Trained {
        /// The training-data reference (REQUIRED, non-empty — a Trained
        /// state without data provenance is unconstructable).
        training_data_ref: String,
        /// When the training happened (caller-injected clock millis).
        trained_at_millis: i64,
    },
    /// Retired (append-only; no un-retire — the safe direction).
    Retired {
        /// Why the model was retired (REQUIRED — an unattributed
        /// retirement is never recorded).
        reason: String,
        /// When (caller-injected clock millis).
        retired_at_millis: i64,
    },
}

impl TrainingState {
    /// The wire spelling of the state (records; the payload fields of
    /// Trained/Retired are rendered by the canonical description).
    pub fn wire_name(&self) -> &'static str {
        match self {
            TrainingState::Untrained => "untrained",
            TrainingState::Trained { .. } => "trained",
            TrainingState::Retired { .. } => "retired",
        }
    }
}

/// Why a registration was refused. Every variant is a DISTINCT named
/// error; refusals never degrade into an append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The ref was empty, whitespace, or over 256 characters.
    RefInvalid { reason: String },
    /// The ref is already registered (the registry is append-only; there
    /// is no update, no remove, no re-registration).
    AlreadyRegistered { entry_ref: String },
    /// The entry's content digest was not a lowercase hex string of
    /// plausible length.
    DigestInvalid { reason: String },
    /// The lineage reference names nothing registered (an artifact's
    /// producing-model ref, or a model's artifact-kind coverage, must
    /// resolve — lineage cannot dangle).
    LineageRefUnresolved { entry_ref: String, lineage_ref: String },
    /// A lineage reference was empty (a model or artifact entry with no
    /// declared lineage is refused — lineage is required, not optional).
    LineageEmpty { entry_ref: String },
    /// A Trained state carried an empty training-data reference.
    TrainingDataRefMissing,
    /// A Retired state carried an empty reason.
    RetirementReasonMissing,
    /// The previous-head digest carried on a chained registration does
    /// not match the registry's current head (an out-of-order replay is
    /// named, never absorbed).
    ChainHeadMismatch { expected: String, found: String },
    /// A model entry declared no artifact kind it produces.
    NoArtifactKind { entry_ref: String },
}

impl RegistryError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            RegistryError::RefInvalid { .. } => "MODEL_REGISTRY_REF_INVALID",
            RegistryError::AlreadyRegistered { .. } => "MODEL_REGISTRY_ALREADY_REGISTERED",
            RegistryError::DigestInvalid { .. } => "MODEL_REGISTRY_DIGEST_INVALID",
            RegistryError::LineageRefUnresolved { .. } => "MODEL_REGISTRY_LINEAGE_REF_UNRESOLVED",
            RegistryError::LineageEmpty { .. } => "MODEL_REGISTRY_LINEAGE_EMPTY",
            RegistryError::TrainingDataRefMissing => "MODEL_REGISTRY_TRAINING_DATA_REF_MISSING",
            RegistryError::RetirementReasonMissing => "MODEL_REGISTRY_RETIREMENT_REASON_MISSING",
            RegistryError::ChainHeadMismatch { .. } => "MODEL_REGISTRY_CHAIN_HEAD_MISMATCH",
            RegistryError::NoArtifactKind { .. } => "MODEL_REGISTRY_NO_ARTIFACT_KIND",
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::RefInvalid { reason } => {
                write!(f, "the registry ref is invalid: {reason}")
            }
            RegistryError::AlreadyRegistered { entry_ref } => write!(
                f,
                "the ref '{entry_ref}' is already registered; the registry is append-only \
                 and a change is a NEW version or a NEW appended record, never a silent \
                 edit"
            ),
            RegistryError::DigestInvalid { reason } => {
                write!(f, "the content digest is invalid: {reason}")
            }
            RegistryError::LineageRefUnresolved {
                entry_ref,
                lineage_ref,
            } => write!(
                f,
                "the entry '{entry_ref}' names lineage ref '{lineage_ref}', which is not \
                 registered; lineage references must resolve and cannot dangle"
            ),
            RegistryError::LineageEmpty { entry_ref } => write!(
                f,
                "the entry '{entry_ref}' declares no lineage; lineage is required, not \
                 optional (the P8.32 linked-provenance rule)"
            ),
            RegistryError::TrainingDataRefMissing => write!(
                f,
                "a Trained state requires a non-empty training-data reference; provenance \
                 is linked to the state, never stored apart (the P8.32 rule)"
            ),
            RegistryError::RetirementReasonMissing => write!(
                f,
                "a Retired state requires a non-empty reason; an unattributed retirement \
                 is never recorded"
            ),
            RegistryError::ChainHeadMismatch { expected, found } => write!(
                f,
                "the registration names previous-head digest '{found}' but the registry \
                 head is '{expected}'; an out-of-order replay is named, never absorbed"
            ),
            RegistryError::NoArtifactKind { entry_ref } => write!(
                f,
                "the model entry '{entry_ref}' declares no artifact kind it produces; a \
                 model that produces nothing is not registered"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}

/// The digest of a registration's canonical description (FNV-1a 64
/// test-grade — NOT a security primitive; the production digest is a
/// DEPLOYMENT REQUIREMENT, the P8.32 posture).
pub fn registry_content_digest(canonical: &str) -> String {
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

/// One registry entry: the identity, kind, lineage, training state, and
/// chain position of a registered model or artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    /// The stable entry ref (e.g. `model://registry/persistence-v1`).
    pub entry_ref: String,
    /// The kind (closed vocabulary).
    pub kind: RegistryEntryKind,
    /// The entry version (a content change is a NEW ref + NEW entry, so
    /// this is the version of the ENTRY SHAPE's description; starts v1).
    pub version: String,
    /// The lineage references this entry names (registry refs validated
    /// to resolve at registration; `input://` external input-data refs
    /// exempt; REQUIRED non-empty).
    pub lineage_refs: Vec<String>,
    /// The artifact kinds the entry relates to (a model entry: the kinds
    /// it produces — at least one REQUIRED; an artifact entry: its own
    /// kind).
    pub artifact_kinds: Vec<&'static str>,
    /// The training state (models); `Untrained` for artifacts.
    pub training_state: TrainingState,
    /// The registration timestamp (caller-injected clock millis; set by
    /// the register call, never by the entry).
    pub registered_at_millis: i64,
    /// The content digest over this entry's canonical description.
    pub content_digest: String,
    /// The digest of the PREVIOUS registry head when this entry was
    /// appended (the chain link; the FIRST entry chains the offset basis
    /// — the empty-chain digest).
    pub previous_head_digest: String,
}

impl RegistryEntry {
    /// The canonical description (stable field order, `|` separators) the
    /// content digest is computed over. Deterministic: fixed order, no
    /// floats, no wall clock.
    pub fn canonical_string(&self) -> String {
        format!(
            "ref:{}|version:{}|kind:{}|lineage:{}|kinds:{}|state:{}|registered:{}",
            self.entry_ref,
            self.version,
            self.kind.wire_name(),
            self.lineage_refs.join(","),
            self.artifact_kinds.join(","),
            self.state_canonical(),
            self.registered_at_millis,
        )
    }

    /// The training-state's canonical fragment (deterministic rendering
    /// of the state's payload fields).
    fn state_canonical(&self) -> String {
        match &self.training_state {
            TrainingState::Untrained => "untrained".to_string(),
            TrainingState::Trained {
                training_data_ref,
                trained_at_millis,
            } => format!("trained({training_data_ref},{trained_at_millis})"),
            TrainingState::Retired {
                reason,
                retired_at_millis,
            } => format!("retired({reason},{retired_at_millis})"),
        }
    }
}

/// The empty-chain digest: the FNV-1a 64 offset basis rendered as hex —
/// the head digest of an empty registry (DEFINITIONAL: FNV-1a 64 over the
/// empty byte string IS the basis; nothing is fabricated).
pub const EMPTY_CHAIN_HEAD_DIGEST: &str = "cbf29ce484222325";

/// The append-only, digest-chained model registry. Entries live in a
/// `BTreeMap` keyed by ref (deterministic iteration); `append_order`
/// preserves registration order (the replay view). There is no update,
/// no remove, no re-registration.
#[derive(Debug, Default, Clone)]
pub struct ModelRegistry {
    entries: BTreeMap<String, RegistryEntry>,
    append_order: Vec<RegistryEntry>,
    head_digest: String,
}

impl ModelRegistry {
    /// A fresh, empty registry whose head is the empty-chain digest.
    pub fn new() -> ModelRegistry {
        ModelRegistry {
            entries: BTreeMap::new(),
            append_order: Vec::new(),
            head_digest: EMPTY_CHAIN_HEAD_DIGEST.to_string(),
        }
    }

    /// The current head digest (the chain's integrity check).
    pub fn head_digest(&self) -> &str {
        &self.head_digest
    }

    /// The registry version the entries were registered under.
    pub fn registry_version(&self) -> &'static str {
        MODEL_REGISTRY_VERSION
    }

    /// The number of registered entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether the ref is registered.
    pub fn contains(&self, entry_ref: &str) -> bool {
        self.entries.contains_key(entry_ref)
    }

    /// The entry for a ref, if registered.
    pub fn get(&self, entry_ref: &str) -> Option<&RegistryEntry> {
        self.entries.get(entry_ref)
    }

    /// The entries in APPEND order (the replay view; deterministic).
    pub fn entries_in_append_order(&self) -> &[RegistryEntry] {
        &self.append_order
    }

    /// The entries in REF order (BTreeMap order; deterministic).
    pub fn entries_in_ref_order(&self) -> Vec<&RegistryEntry> {
        self.entries.values().collect()
    }

    /// Registers one entry (append-only). Validation order: ref, digest,
    /// training state, lineage, chain head — each a distinct named
    /// refusal; nothing is partially accepted. On success the head
    /// digest advances to the new entry's digest.
    ///
    /// `previous_head_digest` must name the CURRENT head (an out-of-order
    /// replay is a named refusal — the caller proves it saw the chain it
    /// extends).
    pub fn register(
        &mut self,
        entry_ref: String,
        kind: RegistryEntryKind,
        version: String,
        lineage_refs: Vec<String>,
        artifact_kinds: Vec<&'static str>,
        training_state: TrainingState,
        registered_at_millis: i64,
        previous_head_digest: String,
    ) -> Result<&RegistryEntry, RegistryError> {
        if entry_ref.trim().is_empty() || entry_ref.len() > 256 {
            return Err(RegistryError::RefInvalid {
                reason: "the ref must be a non-empty string of at most 256 characters"
                    .to_string(),
            });
        }
        if self.entries.contains_key(&entry_ref) {
            return Err(RegistryError::AlreadyRegistered {
                entry_ref: entry_ref.clone(),
            });
        }
        if version.trim().is_empty() {
            return Err(RegistryError::RefInvalid {
                reason: "the version must be non-empty".to_string(),
            });
        }
        match &training_state {
            TrainingState::Trained {
                training_data_ref,
                ..
            } => {
                if training_data_ref.trim().is_empty() {
                    return Err(RegistryError::TrainingDataRefMissing);
                }
            }
            TrainingState::Retired { reason, .. } => {
                if reason.trim().is_empty() {
                    return Err(RegistryError::RetirementReasonMissing);
                }
            }
            TrainingState::Untrained => {}
        }
        if lineage_refs.is_empty() || lineage_refs.iter().any(|r| r.trim().is_empty()) {
            return Err(RegistryError::LineageEmpty {
                entry_ref: entry_ref.clone(),
            });
        }
        if kind == RegistryEntryKind::ForecastModel && artifact_kinds.is_empty() {
            return Err(RegistryError::NoArtifactKind {
                entry_ref: entry_ref.clone(),
            });
        }
        if previous_head_digest != self.head_digest {
            return Err(RegistryError::ChainHeadMismatch {
                expected: self.head_digest.clone(),
                found: previous_head_digest,
            });
        }
        // Lineage resolution: every lineage ref that names a REGISTRY
        // entry must name an ALREADY registered entry (lineage cannot
        // dangle). A ref with the `input://` scheme names EXTERNAL input
        // data — a raw fixture outside the registry, never a registry
        // entry — so it is exempt from resolution: the first entry of a
        // fresh registry chains from its input data (the eval-set record
        // is registered first, then the model, then the artifact), and
        // requiring an `input://` ref to resolve would make the FIRST
        // registration impossible (nothing is registered yet, and lineage
        // is required non-empty), deadlocking the registry permanently.
        for lineage_ref in &lineage_refs {
            if lineage_ref.starts_with("input://") {
                continue;
            }
            if !self.entries.contains_key(lineage_ref) {
                return Err(RegistryError::LineageRefUnresolved {
                    entry_ref: entry_ref.clone(),
                    lineage_ref: lineage_ref.clone(),
                });
            }
        }

        let entry = RegistryEntry {
            entry_ref: entry_ref.clone(),
            kind,
            version,
            lineage_refs,
            artifact_kinds,
            training_state,
            registered_at_millis,
            content_digest: String::new(),
            previous_head_digest: self.head_digest.clone(),
        };
        let mut entry = entry;
        entry.content_digest = registry_content_digest(&entry.canonical_string());
        // The new head is the digest over the PREVIOUS head and the new
        // content digest (the chain is a rolling digest, not just a list
        // of content digests — an edit anywhere breaks every later link).
        let chained = format!("{}|{}", self.head_digest, entry.content_digest);
        self.head_digest = registry_content_digest(&chained);
        self.entries.insert(entry_ref.clone(), entry.clone());
        self.append_order.push(entry);
        Ok(self
            .entries
            .get(&entry_ref)
            .expect("the entry was just inserted"))
    }

    /// Verifies the chain: replaying every entry's previous-head link and
    /// recomputing the rolling digest must reproduce the current head.
    /// A mismatch is a NAMED deviation, never absorbed (the tamper check).
    pub fn verify_chain(&self) -> Result<(), RegistryError> {
        let mut head = EMPTY_CHAIN_HEAD_DIGEST.to_string();
        for entry in &self.append_order {
            if entry.previous_head_digest != head {
                return Err(RegistryError::ChainHeadMismatch {
                    expected: head,
                    found: entry.previous_head_digest.clone(),
                });
            }
            let recomputed = registry_content_digest(&entry.canonical_string());
            if recomputed != entry.content_digest {
                return Err(RegistryError::DigestInvalid {
                    reason: format!(
                        "entry '{}' content digest does not reproduce (recorded '{}', \
                         recomputed '{recomputed}')",
                        entry.entry_ref, entry.content_digest
                    ),
                });
            }
            head = registry_content_digest(&format!("{head}|{}", entry.content_digest));
        }
        if head != self.head_digest {
            return Err(RegistryError::ChainHeadMismatch {
                expected: head,
                found: self.head_digest.clone(),
            });
        }
        Ok(())
    }
}

/// The FNV-1a 64-bit hash (the repo's non-cryptographic pattern;
/// deterministic, dependency-free, explicitly NOT a security primitive).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-derived from the rules above, a published FNV-1a 64 test vector,
// or a literal copied from this module's own constants. Nothing is
// fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fnv_hex(bytes: &[u8]) -> String {
        format!("{:016x}", fnv1a64(bytes))
    }

    #[test]
    fn fnv1a64_matches_published_test_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn the_empty_chain_head_is_the_fnv_offset_basis() {
        // DEFINITIONAL: an empty registry's head is FNV-1a 64 over the
        // empty byte string = the offset basis.
        assert_eq!(EMPTY_CHAIN_HEAD_DIGEST, "cbf29ce484222325");
        assert_eq!(fnv_hex(b""), EMPTY_CHAIN_HEAD_DIGEST);
        let registry = ModelRegistry::new();
        assert_eq!(registry.head_digest(), EMPTY_CHAIN_HEAD_DIGEST);
        assert!(registry.is_empty());
    }

    #[test]
    fn the_first_registration_refuses_to_dangle_lineage() {
        // Lineage must resolve: the FIRST entry of a fresh registry cannot
        // name an unregistered ref — the caller registers the input-data /
        // eval-set records FIRST, then the model that consumed them.
        let mut registry = ModelRegistry::new();
        let err = registry
            .register(
                "model://registry/persistence-v1".to_string(),
                RegistryEntryKind::ForecastModel,
                "v1".to_string(),
                vec!["eval-set://forecast-eval-set-v1".to_string()],
                vec!["load"],
                TrainingState::Untrained,
                1_000,
                EMPTY_CHAIN_HEAD_DIGEST.to_string(),
            )
            .expect_err("dangling lineage refused");
        assert_eq!(
            err,
            RegistryError::LineageRefUnresolved {
                entry_ref: "model://registry/persistence-v1".to_string(),
                lineage_ref: "eval-set://forecast-eval-set-v1".to_string(),
            }
        );
    }

    #[test]
    fn a_valid_registration_chain_builds_and_verifies() {
        let mut registry = ModelRegistry::new();
        let head0 = registry.head_digest().to_string();

        // 1. The eval-set record (an artifact-kind record with no model).
        let eval_ref = "record://forecast-eval-set-v1".to_string();
        let first = registry
            .register(
                eval_ref.clone(),
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec!["input://synthetic-portfolio-fixture-v1".to_string()],
                vec!["load", "pv_generation"],
                TrainingState::Untrained,
                1_000,
                head0,
            )
            .expect("the first record registers");
        assert_eq!(first.entry_ref, eval_ref);
        assert_eq!(
            first.content_digest,
            registry_content_digest(&first.canonical_string())
        );

        // 2. The persistence model, lineage = the eval-set record.
        let head1 = registry.head_digest().to_string();
        let model_ref = "model://registry/persistence-v1".to_string();
        let model = registry
            .register(
                model_ref.clone(),
                RegistryEntryKind::ForecastModel,
                "v1".to_string(),
                vec![eval_ref.clone()],
                vec!["load", "pv_generation"],
                TrainingState::Untrained,
                2_000,
                head1,
            )
            .expect("the model registers");
        assert_eq!(model.training_state, TrainingState::Untrained);

        // 3. An artifact produced BY the model (lineage = the model).
        let head2 = registry.head_digest().to_string();
        let artifact_ref = "artifact://forecasts/load/site-a/v1".to_string();
        registry
            .register(
                artifact_ref,
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec![model_ref.clone()],
                vec!["load"],
                TrainingState::Untrained,
                3_000,
                head2,
            )
            .expect("the artifact registers");

        // The chain verifies end-to-end.
        registry.verify_chain().expect("the chain verifies");
        assert_eq!(registry.len(), 3);
        // BTreeMap ref order is deterministic and distinct from append order.
        let ref_order: Vec<&str> = registry
            .entries_in_ref_order()
            .iter()
            .map(|e| e.entry_ref.as_str())
            .collect();
        assert_eq!(
            ref_order,
            vec![
                "artifact://forecasts/load/site-a/v1",
                "model://registry/persistence-v1",
                "record://forecast-eval-set-v1",
            ]
        );
        let append_order: Vec<&str> = registry
            .entries_in_append_order()
            .iter()
            .map(|e| e.entry_ref.as_str())
            .collect();
        assert_eq!(append_order.len(), 3);
    }

    #[test]
    fn replaying_the_same_registrations_reproduces_the_same_head() {
        // Replay determinism: two registries built from the SAME
        // registration sequence (same timestamps — caller-injected clocks,
        // never wall clocks) have byte-identical entries and the same
        // head digest.
        let build = || {
            let mut registry = ModelRegistry::new();
            let head0 = registry.head_digest().to_string();
            registry
                .register(
                    "record://forecast-eval-set-v1".to_string(),
                    RegistryEntryKind::ForecastArtifact,
                    "v1".to_string(),
                    vec!["input://synthetic-portfolio-fixture-v1".to_string()],
                    vec!["load"],
                    TrainingState::Untrained,
                    1_000,
                    head0,
                )
                .expect("registers");
            let head1 = registry.head_digest().to_string();
            registry
                .register(
                    "model://registry/persistence-v1".to_string(),
                    RegistryEntryKind::ForecastModel,
                    "v1".to_string(),
                    vec!["record://forecast-eval-set-v1".to_string()],
                    vec!["load"],
                    TrainingState::Untrained,
                    2_000,
                    head1,
                )
                .expect("registers");
            registry
        };
        let a = build();
        let b = build();
        assert_eq!(a.head_digest(), b.head_digest());
        assert_eq!(a.entries_in_append_order(), b.entries_in_append_order());
    }

    #[test]
    fn a_change_is_a_new_appended_record_never_a_silent_edit() {
        let mut registry = ModelRegistry::new();
        registry
            .register(
                "record://forecast-eval-set-v1".to_string(),
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec!["input://fixture-v1".to_string()],
                vec!["load"],
                TrainingState::Untrained,
                1_000,
                EMPTY_CHAIN_HEAD_DIGEST.to_string(),
            )
            .expect("registers");
        let head1 = registry.head_digest().to_string();
        // Re-registering the SAME ref is a named refusal (append-only).
        let err = registry
            .register(
                "record://forecast-eval-set-v1".to_string(),
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec!["input://fixture-v1".to_string()],
                vec!["load"],
                TrainingState::Untrained,
                2_000,
                head1.clone(),
            )
            .expect_err("re-registration refused");
        assert_eq!(
            err,
            RegistryError::AlreadyRegistered {
                entry_ref: "record://forecast-eval-set-v1".to_string(),
            }
        );
        // The legal change: a NEW ref, appended (v2 of the record).
        registry
            .register(
                "record://forecast-eval-set-v2".to_string(),
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec!["input://fixture-v1".to_string()],
                vec!["load"],
                TrainingState::Untrained,
                2_000,
                head1,
            )
            .expect("the v2 record registers");
        // And the ORIGINAL entry is untouched (frozen, not edited).
        assert_eq!(
            registry.get("record://forecast-eval-set-v1").expect("still present").registered_at_millis,
            1_000
        );
    }

    #[test]
    fn an_out_of_order_replay_is_a_named_refusal() {
        let mut registry = ModelRegistry::new();
        // The registration names a STALE head (the empty chain) after the
        // registry already advanced.
        registry
            .register(
                "record://a".to_string(),
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec!["input://x".to_string()],
                vec!["load"],
                TrainingState::Untrained,
                1_000,
                EMPTY_CHAIN_HEAD_DIGEST.to_string(),
            )
            .expect("registers");
        let err = registry
            .register(
                "record://b".to_string(),
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec!["record://a".to_string()],
                vec!["load"],
                TrainingState::Untrained,
                2_000,
                EMPTY_CHAIN_HEAD_DIGEST.to_string(),
            )
            .expect_err("stale head refused");
        assert!(matches!(err, RegistryError::ChainHeadMismatch { .. }));
    }

    #[test]
    fn trained_and_retired_states_carry_required_provenance() {
        let mut registry = ModelRegistry::new();
        registry
            .register(
                "record://forecast-eval-set-v1".to_string(),
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec!["input://fixture-v1".to_string()],
                vec!["load"],
                TrainingState::Untrained,
                1_000,
                EMPTY_CHAIN_HEAD_DIGEST.to_string(),
            )
            .expect("registers");
        let head1 = registry.head_digest().to_string();
        // A Trained state with an EMPTY training-data ref is refused.
        let err = registry
            .register(
                "model://registry/x".to_string(),
                RegistryEntryKind::ForecastModel,
                "v1".to_string(),
                vec!["record://forecast-eval-set-v1".to_string()],
                vec!["load"],
                TrainingState::Trained {
                    training_data_ref: "  ".to_string(),
                    trained_at_millis: 2_000,
                },
                2_000,
                head1.clone(),
            )
            .expect_err("empty training-data ref refused");
        assert_eq!(err.name(), "MODEL_REGISTRY_TRAINING_DATA_REF_MISSING");
        // A Retired state with an empty reason is refused.
        let err = registry
            .register(
                "model://registry/x".to_string(),
                RegistryEntryKind::ForecastModel,
                "v1".to_string(),
                vec!["record://forecast-eval-set-v1".to_string()],
                vec!["load"],
                TrainingState::Retired {
                    reason: "".to_string(),
                    retired_at_millis: 2_000,
                },
                2_000,
                head1.clone(),
            )
            .expect_err("empty retirement reason refused");
        assert_eq!(err.name(), "MODEL_REGISTRY_RETIREMENT_REASON_MISSING");
        // A model entry with NO artifact kind is refused.
        let err = registry
            .register(
                "model://registry/x".to_string(),
                RegistryEntryKind::ForecastModel,
                "v1".to_string(),
                vec!["record://forecast-eval-set-v1".to_string()],
                Vec::new(),
                TrainingState::Untrained,
                2_000,
                head1,
            )
            .expect_err("no artifact kind refused");
        assert_eq!(err.name(), "MODEL_REGISTRY_NO_ARTIFACT_KIND");
    }

    #[test]
    fn a_tampered_entry_breaks_the_chain_verification() {
        let mut registry = ModelRegistry::new();
        registry
            .register(
                "record://forecast-eval-set-v1".to_string(),
                RegistryEntryKind::ForecastArtifact,
                "v1".to_string(),
                vec!["input://fixture-v1".to_string()],
                vec!["load"],
                TrainingState::Untrained,
                1_000,
                EMPTY_CHAIN_HEAD_DIGEST.to_string(),
            )
            .expect("registers");
        // Tamper with the recorded state (simulating an in-place edit —
        // the exact thing the registry forbids).
        registry.append_order[0].registered_at_millis = 99_000;
        let err = registry.verify_chain().expect_err("tamper named");
        assert_eq!(err.name(), "MODEL_REGISTRY_DIGEST_INVALID");
    }

    #[test]
    fn every_registry_error_renders_non_empty_prose() {
        let errors = [
            RegistryError::RefInvalid {
                reason: "fixture".to_string(),
            },
            RegistryError::AlreadyRegistered {
                entry_ref: "x".to_string(),
            },
            RegistryError::DigestInvalid {
                reason: "fixture".to_string(),
            },
            RegistryError::LineageRefUnresolved {
                entry_ref: "x".to_string(),
                lineage_ref: "y".to_string(),
            },
            RegistryError::LineageEmpty {
                entry_ref: "x".to_string(),
            },
            RegistryError::TrainingDataRefMissing,
            RegistryError::RetirementReasonMissing,
            RegistryError::ChainHeadMismatch {
                expected: "a".to_string(),
                found: "b".to_string(),
            },
            RegistryError::NoArtifactKind {
                entry_ref: "x".to_string(),
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn the_registry_statement_carries_the_rules() {
        assert_eq!(MODEL_REGISTRY_VERSION, "flexcore-application/model-registry.v1");
        assert!(MODEL_REGISTRY_STATEMENT.contains("append-only"));
        assert!(MODEL_REGISTRY_STATEMENT.contains("UNTRAINED"));
        assert!(MODEL_REGISTRY_STATEMENT.contains("never a silent edit"));
        assert!(MODEL_REGISTRY_STATEMENT.contains("determinism"));
        assert_eq!(RegistryEntryKind::ForecastArtifact.wire_name(), "forecast_artifact");
        assert_eq!(RegistryEntryKind::ForecastModel.wire_name(), "forecast_model");
        assert_eq!(TrainingState::Untrained.wire_name(), "untrained");
    }
}
