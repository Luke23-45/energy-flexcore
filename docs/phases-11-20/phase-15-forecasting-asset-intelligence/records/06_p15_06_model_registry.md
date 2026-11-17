# P15.06 — Model registry and forecast-artifact versioning: append-only, digest-chained, lineage-validated, replay-deterministic, UNTRAINED as a first-class state

| Field | Value |
|---|---|
| Subtask | P15.06 (artifacts/models versioned + digest-chained; model→artifact lineage; replay determinism mirroring the flexcore-research patterns; UNTRAINED is a first-class registered state; a registry change is a new version, never a silent edit) |
| Wave / agent | Phase 15 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the registry is written, never compiled or executed; NO model is trained (every model registers UNTRAINED — no runtime data exists); no skill, accuracy, or training claim is made for any entry (conventions §1) |
| Sources | Seams: `crates/flexcore-research/src/artifact_signing.rs` (P8.32 provenance-record shape — the linked-provenance rule), `crates/flexcore-research/src/baselines.rs` (P8.06 version-stamped frozen-registry discipline, mirrored), `benchmarks/research-set-v1.json` (the anti-silent-growth rule), the kernel UNREVOCATION_SEAM posture (append-only safe direction) |
| Files touched | `crates/flexcore-application/src/model_registry.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive) |

## 1. What landed

| Item | Content |
|---|---|
| Entry model | `RegistryEntry`: ref, kind (`ForecastArtifact` / `ForecastModel` — closed), version, lineage refs, artifact kinds, training state, registration timestamp (CALLER-injected clock millis — no `SystemTime` anywhere), content digest, previous-head digest |
| Training states | `TrainingState` (closed): `Untrained` (the state of EVERY model in this delivery — a first-class registered state, not an absence), `Trained { training_data_ref, trained_at_millis }` (the training-data reference REQUIRED — provenance linked to the state, the P8.32 rule; a Trained state with an empty ref is unconstructable), `Retired { reason, retired_at_millis }` (append-only safe direction; no un-retire — the kernel UNREVOCATION_SEAM posture; an empty reason refused) |
| Digest chain | FNV-1a 64 content digest over the canonical description (stable field order, `|` separators; test-grade — NOT a security primitive; the production digest is a DEPLOYMENT REQUIREMENT, the P8.32 posture) + a ROLLING head digest (`digest(prev_head | content_digest)`); the empty-chain head is the FNV offset basis `cbf29ce484222325` (definitional); `verify_chain` replays every link and names a tamper (`MODEL_REGISTRY_DIGEST_INVALID`) instead of absorbing it |
| Append-only discipline | No update, no remove, no re-registration: a duplicate ref is a named refusal (`MODEL_REGISTRY_ALREADY_REGISTERED`); the legal change is a NEW ref + NEW appended record (the test registers a v2 record and pins the v1 record untouched); a registry SHAPE change is a `MODEL_REGISTRY_VERSION` bump |
| Chain-head proof | Every registration must name the CURRENT head (`previous_head_digest`); a stale head is a named refusal (`MODEL_REGISTRY_CHAIN_HEAD_MISMATCH`) — the caller proves it saw the chain it extends, so an out-of-order replay cannot silently fork the history |
| Lineage validation | Every lineage ref must name an ALREADY-registered entry (lineage cannot dangle — the first registration therefore cannot reference nothing: the caller registers input-data/eval-set records FIRST, then models, then artifacts; the test exercises exactly this order); a model entry must declare at least one artifact kind it produces |
| Replay determinism | Entries in a `BTreeMap` keyed by ref (deterministic iteration) + `append_order` (the replay view); timestamps are explicit parameters; no randomness, no wall clock — the test replays the same registration sequence twice and asserts identical entries and identical head digests |

## 2. Placement (recorded)

The registry lives in `flexcore-application` (the plan's wave-1 Agent A ownership: "the model registry modules"), mirroring the flexcore-research patterns BY CITATION (no flexcore-research dependency — the crate manifest policy, the same discipline as the P15.01 signing mirror). The registry is the lineage backbone the P15.01 provenance records' `model-ref` and `eval-set-ref` fields resolve against, and the state vocabulary the P15.02 worker envelope's `model_registry_state: UNTRAINED` field names.

## 3. Honest boundaries

1. NO model is trained and NO training run is claimed: the platform has never run, so every model registers `Untrained`; `Trained` exists in the vocabulary so the future transition is a REGISTERED, versioned event — never a silent relabel.
2. The registry ships EMPTY; the test-built chains are constructed fixtures, not registered state.
3. The digest chain is test-grade (FNV-1a 64): an integrity/replay device, explicitly NOT a security primitive; the production digest is a DEPLOYMENT REQUIREMENT (decision point, recorded here).
4. Every test is WRITTEN-UNRUN; every expected value is hand-derived (published FNV-1a test vectors, the offset-basis definition, and the replay-equality property).
