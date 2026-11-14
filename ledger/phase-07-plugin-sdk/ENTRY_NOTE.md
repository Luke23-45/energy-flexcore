# ENTRY NOTE — Phase 7: Plugin SDK and Modular Composition

Date: 2026-11-13. Coordinator: implementation-coordinator. Loop rules in
force: nothing installed, nothing built, no tests run, no CI run — complete
code + test files written only. Ticks are "implemented, awaiting human
build/test". Decision records per batch in this directory.

## Gate exception (same pattern as Phases 1–6)

Stage 6 exit gate is **NOT HELD** (lab run UNRUN —
`ledger/phase-06-edge-protocol/24_exit_gate_lab_run_record.md`;
`30_retrospective_release_gate.md` records the v0.4.0 position as prepared).
Phase 7 starts on the recorded exception; P7.01 records it honestly.

## Phase-7-specific boundaries

1. **Protected kernel is non-replaceable** (ADR-005, §5.1/§5.7/§9.1):
   authorization, tenant isolation, audit integrity, command lifecycle,
   final dispatch validation. No plugin implements `CommandAuthorizer` or
   touches `DispatchGateway`. The P6.14 command-path gate is protected
   kernel behaviour (P6.29 handover) — never a plugin capability.
2. **ADR-006 hard prohibition**: no arbitrary Rust dylib loading (no
   `.so`/`.dll` loader, including via C ABI) — rejected, not deferred.
   Extensions are WASM/WIT (Class B) or out-of-process gRPC workers
   (Class C) only. P7.35 makes it a CI gate.
3. **ADR-010**: ML/RL outputs are proposals only; the SDK must make a
   dispatch-bypassing plugin UNBUILDABLE, not merely discouraged.
4. **Wasmtime pin floor ≥50.0.0** (RESEARCH.md R7-C2: CVE-2026-115889 /
   CVE-2026-115893 sandbox escapes below 50.0.0); pin 50.0.1; requires
   Rust 1.97 to compile — recorded, NEVER installed/built in this loop.
   WASI 0.2 baseline (0.3 tracked, not blocking — report §5.4).
   `wasm-tools` 1.239.0 ASSUMPTION, re-check at P7.09.
5. **New dependency discipline (D4)**: every new crate (wasmtime,
   wit-bindgen, tonic, prost, wasip2…) pinned from RESEARCH.md or
   labelled UNPINNED-REVERIFY; P7.03 owns the audit; `deny.toml` grows
   additively. Nothing installed.
6. **Simulation/shadow only**: v0.5.0; no field dispatch authority for any
   plugin; no certified conformance claims.
7. Component locations per PLAN.md §3: `crates/flexcore-kernel/` (NEW —
   composition/registry/lifecycle/permissions/dependency resolution),
   `wit/` (versioned worlds), `apps/plugin-host/` (separate process),
   `plugins/{manifests,builtin,wasm-examples}/`, `python/workers/`
   (template), `docs/plugin-sdk/`. Contracts live in `flexcore-ports`.
   New workspace members require an additive `check_dependency_rule.py`
   update (same as edge-agent in Phase 6).

## Wave plan (max 2 subagents; host caps concurrency at 2; disjoint file ownership)

| Wave | Subtasks | Files owned | Notes |
|---|---|---|---|
| 0 (coordinator) | — | this file, HANDOVER.md plan | Gate exception + boundaries |
| 1a | P7.01, P7.02, P7.03, P7.04 | decision records (readiness, scope freeze, license audit, seam inventory) | Records only; input = P6.29 handover |
| 1b | P7.05, P7.06 | NEW `crates/flexcore-kernel/**` (manifest schema + validator, dependency resolution) + workspace member + check_dependency_rule.py additive + tests | First kernel code |
| 2a | P7.07, P7.08 | kernel lifecycle + Class A registry (kernel crate additive) | |
| 2b | P7.09, P7.40 | NEW `wit/**` worlds + toolchain pinning record | |
| 3a | P7.10, P7.11 | NEW `apps/plugin-host/**` (host + capability broker) + workspace member | wasmtime pinned in its Cargo.toml |
| 3b | P7.12, P7.13 | kernel permission enforcement + sandbox negative tests | |
| 4a | P7.14, P7.15, P7.16 | signing/provenance/SBOM, profiles/bundles, profile switching | |
| 4b | P7.17, P7.18 | NEW `python/workers/` gRPC template + worker supervision (control-plane side) | tonic/prost pins |
| 5a | P7.19, P7.20, P7.36 | config schema + secret refs, health/readiness, replay/sequence/expiry on plugin path | |
| 5b | P7.21, P7.22, P7.23 | conformance suite, swap test, untrusted-plugin negative test | |
| 6a | P7.24, P7.25 | `docs/plugin-sdk/` + examples incl. bad example; console Plugins workspace | |
| 6b | P7.26, P7.27, P7.35 | perf templates, CI gates, no-dylib CI gate | |
| 7a | P7.28, P7.29, P7.30 | exit-gate run record (UNRUN), security testing, failure drill | |
| 7b | P7.31, P7.33, P7.34 | Phase 8 handover, audit-integrity proof, trust boundary mTLS | |
| 8a | P7.37, P7.38, P7.39 | safety-invariant test map, revocation/staged rollout, tenant filtering | |
| 8b | P7.41, P7.42, P7.43 | manifest fuzzer, determinism/replay, scenario regression | |
| 9 (coordinator) | P7.32 | retrospective + v0.5.0 release-gate record (UNRUN) | All P7.01–P7.43 |
