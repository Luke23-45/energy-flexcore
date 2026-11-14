// P8.05/P8.06 — flexcore-research: the Phase 8 research crate (report §13,
// §19, §22.1 ADR-010; ledger `ledger/phase-08-advanced-coordination/
// ledger_phase_8.md` §P8.05/§P8.06; decision record:
// `ledger/phase-08-advanced-coordination/05_06_harness_baselines.md`).
//
// Two modules, deliberately small and deliberately separate:
//
// - `validation` — the INDEPENDENT VALIDATION HARNESS (P8.05): the
//   validator that does not trust the researcher. Every research module's
//   output (a [`validation::ResearchProposal`]) passes through a FIXED
//   stage order — shape/vocabulary, provenance completeness, validity
//   window (injected clock), snapshot freshness, and PHYSICAL
//   RE-VALIDATION through the platform's existing feasibility gate,
//   injected as a port. The harness NEVER re-implements physics, NEVER
//   accepts a researcher-supplied verdict (there is no field for one), and
//   the only path to a [`validation::ValidatedProposal`] runs through the
//   gate (the constructor is crate-private; the sealed-field pattern makes
//   external construction a compile error).
// - `baselines` — the STRONG DETERMINISTIC BASELINE REGISTRY (P8.06): the
//   version-stamped, frozen comparison set every research module must beat,
//   built from the EXISTING platform providers by citation + composition
//   seams (Tier 1 HiGHS LP/QP/MIP — not SOCP; Tier 2 Clarabel convex conic
//   — not MISOCP; a fully-specified in-crate priority-list dispatch
//   heuristic), plus the frozen-version discipline (a baseline change is a
//   NEW baseline version, never a silent edit).
//
// ADR-010 (the governing boundary): research outputs are PROPOSALS only.
// Nothing in this crate authorizes, issues, approves, or dispatches
// anything; every type surface here is data-only, mirroring the kernel's
// `PluginProposalEnvelope` shape by citation (the alignment seam is
// recorded in `validation.rs` — this crate does not depend on
// flexcore-kernel; see the dependency note in `Cargo.toml`).
//
// Control status: research only. Simulation / shadow. This crate performs
// no field control, makes no safety claim, and grants no dispatch
// authority. Canonical units: MW / MVAr / MVA / MWh; `P > 0` = export at
// the point of connection; SOC is a fraction in [0, 1] (§10.2).
//
// Honest status: WRITTEN-UNCOMPILED. No cargo command was executed in this
// loop (the subtask's loop bans forbid build/test); every type reference
// was read from source and the tests are written, not run. The human
// build/test gate (AGENTS.md rule 7) owns the compile/test evidence.

pub mod baselines;
pub mod validation;

// P8.07 — the ADMM/DMPC formulation with explicit convexity assumptions
// (`admm`), and P8.08 — the operational contract (residuals, termination,
// infeasibility, staleness, centralized fallback) plus the DMPC
// rolling-horizon driver (`dmpc`); decision records:
// `ledger/phase-08-advanced-coordination/07_admm_dmpc.md` and
// `ledger/phase-08-advanced-coordination/08_admm_operations.md`.
pub mod admm;
pub mod dmpc;

// P8.09 — the ADMM/DMPC evaluation harness: per-case statistics assembled
// over INJECTED data (convergence statistics against the recorded Boyd
// tolerances, objective gaps against the frozen P8.06 baselines where
// meaningful, fallback/staleness aggregates, the P8.05 validation outcome,
// the human-recorded runtime). Every measured cell stays EMPTY until the
// human/lab run fills it (ENTRY_NOTE boundary 4); decision record:
// `ledger/phase-08-advanced-coordination/09_admm_evaluation.md`.
pub mod admm_evaluation;

// P8.31 — research-worker tenant isolation and scoped data access: the
// research-side mirror (by citation, no flexcore-kernel dependency) of the
// kernel's host-stamped tenant scope — host-stamped worker scope stamps,
// echo-mismatch refusal, scoped reads over the research catalog (every
// cross-tenant attempt denied AND audited), and the append-only
// model-serving register that refuses unapproved cross-tenant serving.
// Decision record:
// `ledger/phase-08-advanced-coordination/31_tenant_isolation.md`.
pub mod tenant_isolation;

// P8.34 — proposal validity windows and stale-proposal rejection: the
// completion of the P8.05 validity-window seam (ADDITIVE — validation.rs
// and its fixed stage list are unchanged). The host-side window grantor
// (injected clock, capped duration, never the proposal's own clock), the
// wire-contract window extraction (missing window refused, never
// defaulted), the restart-surviving snapshot-supersession checkpoint
// (append-only, buffer_file framing discipline), and the gate-at-time-of-
// use re-check that returns the harness's own refusal vocabulary BEFORE
// any downstream consumption. Decision record:
// `ledger/phase-08-advanced-coordination/34_proposal_windows.md`.
pub mod proposal_windows;

// P8.32 — model-artifact signing, provenance, and verification at load:
// the research-side mirror (by citation, no flexcore-kernel dependency) of
// the kernel's P7.14 provenance-v1 pattern — the versioned
// model-artifact provenance record with LINKED training provenance
// references, the fail-closed digest-verifier port (NoArtifactVerifier
// default refuses everything; production SHA-256 wiring is a recorded
// DEPLOYMENT REQUIREMENT / decision point, no crypto crate added), the
// append-only revocation register, and the composed load gate that runs
// the P8.31 serving register FIRST and refuses unsigned/mismatched/
// revoked artifacts with named errors, audited. The Python worker load
// sites (security-review F-1/F-2/F-3) are NOT wired here — the
// end-to-end load path is recorded as NOT live (see
// `artifact_signing::WORKER_LOAD_SITE_SEAM`). Decision record:
// `ledger/phase-08-advanced-coordination/32_model_artifact_signing.md`.
pub mod artifact_signing;

// P8.33 — research-worker identity, mTLS, and secret handling: the
// research-side mirror (by citation) of the plugin-host's P7.34
// transport-trust posture — deployment-bound service identities over the
// closed P8.31 worker-family vocabulary, the config-shaped mTLS channel
// validation (plaintext refused, pinned server name + CA reference,
// short-lived credentials, recorded rotation owner, revocation checking
// configured), the append-only identity-revocation register with the
// fail-closed admission door, and secret handling as REFERENCES ONLY
// (env:/file:/secret-store:, the kernel manifest rule) with the
// whole-tree worker-config forbidden-key scan. The gRPC binding of the
// research workers is the recorded Phase 7 follow-up (see
// `worker_identity::PHASE7_GRPC_BINDING_FOLLOWUP`). Decision record:
// `ledger/phase-08-advanced-coordination/33_identity_mtls_secrets.md`.
pub mod worker_identity;

/// The crate version stamp recorded on every validation result and baseline
/// record (P8.05 step 4: "Record the harness version with every validation
/// result" — the report that does not name its harness version is not a
/// validation result).
pub const CRATE_VERSION: &str = "flexcore-research.v1";
