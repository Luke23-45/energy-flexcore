// P7.05/P7.06 — `flexcore-kernel`: the FlexCore plugin kernel (report §8.1,
// §8.2; ledger `ledger/phase-07-plugin-sdk/ledger_phase_7.md` P7.05/P7.06;
// PLAN.md §3 component breakdown).
//
// The kernel is the composition-side owner of the plugin SDK's structural
// surface: manifests and dependency resolution. It is a COMPOSITION crate
// (`tools/check_dependency_rule.py`), like `apps/worker` — report §8.2:
// "`flexcore-kernel` manages plugin composition and lifecycle; it must not
// become a second domain layer." It does NOT own domain rules or safety
// invariants; the protected-kernel invariants (§5.1, §5.7, ADR-005) are
// guarded here only structurally — a manifest claiming a protected contract
// is refused before any composition is computed (P7.06; ADR-010/§5.7 rule
// made structural, see [`resolve::PROTECTED_KERNEL_CONTRACT_IDS`]).
//
// Module map:
//
// | Module | Responsibility | Later waves add |
// |---|---|---|
// | [`manifest`] | P7.05: the versioned plugin manifest `flexcore-plugin-manifest-v1`, its strict validator (closed key sets, named errors, fail-closed), the enforced SemVer/range subset, and the secret-reference rule (references only — never values) | signing/provenance fields (P7.14); lifecycle metadata (P7.07) |
// | [`resolve`] | P7.06: deterministic dependency resolution over validated manifests — missing provider, incompatible API version, cycle, conflicting providers, explicit selection, protected-kernel guard — outputting the ordered `CompositionPlan` | consumed by the P7.08 built-in registry and P7.16 re-composition |
// | [`lifecycle`] | P7.07: the §5.6 plugin lifecycle state machine — frozen transition table, named-illegal-transition errors, gate outcomes, drain-before-replace with a deadline-refused fail-closed policy, EXPLICIT-only restart, audit records through the in-module `AuditSink` port on an injected `Clock` | consumed by the P7.08 registry swap and P7.16 profile switching |
// | [`registry`] | P7.08: the Class A built-in provider registry — provider factories per contract id, generic over the contract trait object, registered at the composition root from a `CompositionPlan`, protected-kernel reinforcement, lifecycle-aware `swap_provider` | consumed by P7.15 profiles and exercised by the P7.22 swap test |
// | [`signing`] | P7.14: the plugin supply-chain digest-and-record layer — the `flexcore-plugin-provenance-v1` record (signer key REFERENCE only, SBOM reference REQUIRED), the pluggable `DigestVerifier` port (fail-closed `NoVerifier` default; production SHA-256 wiring is a DEPLOYMENT REQUIREMENT, per the P6.20 precedent), and the `Discovered → Verified` provenance gate producing the P7.07 `GateOutcome` | revocation and staged rollout (P7.38); SBOM generation stays P7.27 CI config |
// | [`profile`] | P7.15: profiles as DATA (`flexcore-profile-v1`) — strict closed-key loading, the §5.8 control-authority mode lock (profile data can only carry `simulation-shadow`), the permission-ceiling model, and profile→plan validation through the P7.06 resolver | profile switching consumes these in P7.16; revocation interplay in P7.38 |
// | [`recomposition`] | P7.16: explicit, audited profile switching — deterministic plan diff, fail-closed `SwitchConfirmation`, the §21.4 control-authority restatement, the composition-level protected-kernel re-check, and drain-before-replace execution with recorded partial-failure semantics and the restart fallback | staged rollout interplay (P7.38) |
// | [`plugin_config`] | P7.19: plugin-instance configuration resolution — values validated against the manifest's declared schema (types, required/optional, closed property set), the layered constraints document (enum/bounds/defaults — the documented P7.19 extension), EXPLICITLY recorded defaults, secret REFERENCES resolved to handles (never values — reading is a host/broker capability, P7.11), secret-value refusals that echo nothing, version-anchored drift refusal, and the sorted deterministic resolved view | consumed by the lifecycle's `Compatible → Configured` gate; P7.41 fuzz targets |
// | [`plugin_path_protection`] | P7.36: the P4.28/P6.15 discipline adapted to the plugin path — payload-bound request identity (correlation id + test-grade FNV-1a digest; production digest is a DEPLOYMENT REQUIREMENT), monotonic per-plugin sequence with distinct duplicate/out-of-sequence refusals surviving restart through a `buffer_file.rs`-framing checkpoint, an injected-clock validity window (expired → refused, never executed), and the ADR-010 PROPOSAL envelope (no dispatch-authorizing variant exists; the protected-contract guard restated at the path) | the P7.42 determinism leg for the plugin path |
// | [`revocation`] | P7.38: the append-only plugin-revocation register (keyed by plugin id+version AND by signer key reference — revoking a key revokes everything it signed), consulted FIRST at `Discovered → Verified` and re-checked before `Configured → Started`, the per-profile staged rollout (explicit promotion order; no percentage canary — the type surface is the proof), and the explicit drain-first removal path for `Failed` plugins (audit trail preserved) | revocation-list distribution stays a deployment seam (the recorded `UNREVOCATION_SEAM`) |
// | [`tenant_scope`] | P7.39: the host-stamped plugin-path tenant scope — `TenantScope` derived ONLY from the authenticated context, the `ScopedRequestEnvelope` wrapper binding the scope into the P7.36 request identity, and the audited data-level enforcement cross-check (`TENANT_SCOPE_MISMATCH`); the RLS relationship is recorded, not re-implemented (§16.2/[R25], ADR-007) | the host-side cross-check mirror in `apps/plugin-host/src/tenant_gate.rs` |
//
// Design-for-later (implemented in later waves, shaped for here):
// - P7.07 lifecycle: every manifest field the state machine checks at
//   `Verified`/`Compatible`/`Configured` already exists (signature fields
//   are P7.14; the validator is the `Discovered → Verified`-side gate).
// - P7.08 registry: consumes [`resolve::CompositionPlan`] as its ordered
//   registration list (registration at the composition root, never a global
//   mutable registry, §6.4).
// - P7.15/P7.16 profiles: a profile is a bundle of manifests + explicit
//   selections; [`resolve::ExplicitSelections`] is the conflict-resolution
//   input (never silent first-wins, §5.5/§5.6).
// - P7.41 fuzz/property tests target [`manifest`]'s validator — it stands
//   between an untrusted plugin author and activation.
//
// Determinism (P7.42 depends on it): every public output of this crate is
// sorted and stable — manifest lists are validated in declared order but
// every aggregate (provider maps, cycles, plans) is keyed by `BTreeMap`/
// `BTreeSet` and emitted sorted; no output depends on `serde_json::Value`
// map iteration order (keys are looked up by name; reported leftovers are
// sorted before display). The resolver is a pure function of
// (manifests, selections, the pinned platform API version constant).
//
// Status (honest, per AGENTS.md rule 7): this crate is WRITTEN, NOT BUILT,
// its tests NOT RUN — the loop bans hold (`ledger/phase-07-plugin-sdk/
// ENTRY_NOTE.md` discipline carried over from Phase 6). Ledger ticks read
// "implemented, awaiting human build/test". No `unwrap`/`expect` exists on
// untrusted input anywhere in `src/`; test files may unwrap.

pub mod lifecycle;
pub mod manifest;
pub mod plugin_config;
pub mod plugin_path_protection;
pub mod profile;
pub mod recomposition;
pub mod registry;
pub mod resolve;
// P7.38 — plugin (not just artifact) revocation completing the P7.14
// seams: the append-only revocation register (plugin id+version and
// signer-key keys), the consulted-FIRST `Discovered → Verified` gate and
// the registry-time start re-check, the per-profile staged rollout (no
// percentage canary), and the explicit drain-first removal path for
// `Failed` plugins (see the module header).
pub mod revocation;
pub mod signing;
// P7.39 — the host-stamped plugin-path tenant scope: the authenticated-
// context-only `TenantScope`, the `ScopedRequestEnvelope` wrapper over
// the P7.36 envelope (scope bound into the request identity), and the
// audited data-level enforcement cross-check (see the module header; the
// host-side mirror lives in `apps/plugin-host/src/tenant_gate.rs`).
pub mod tenant_scope;
