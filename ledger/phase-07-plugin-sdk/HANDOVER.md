# HANDOVER — Phase 7: Plugin SDK and Modular Composition

Status: **CLOSED** — 43/43 subtasks implemented (P7.01–P7.43); the §23
Stage 7 exit gate is **NOT HELD** (prepared, not demonstrated — see
`30_retrospective_release_gate.md` and `28_exit_gate_run_record.md`).
Loop bans held throughout: nothing installed, nothing built, no tests run,
no CI run — all suites WRITTEN-UNRUN.

## Blockers carried (human gate inputs)

1. Stages 1–6 gates NOT HELD — all demonstrations/runs/sign-offs pending.
2. Wasmtime 50.0.1 pin requires Rust 1.97 — never installed/built here.
3. Wasmtime CVE floor ≥50.0.0 (RESEARCH.md R7-C2) — re-verify the pinned
   version carries CVE-2026-115384 fixes at implementation time.
4. `wasm-tools` 1.239.0 ASSUMPTION — re-check at P7.09.
5. Secret/dependency scans, cargo-deny, Buf/WIT validation runs = human.

## Wave status

| Wave | Subtasks | Status |
|---|---|---|
| 1a | P7.01, P7.02, P7.03, P7.04 | pending |
| 1b | P7.05, P7.06 | pending |
| 2a | P7.07, P7.08 | pending |
| 2b | P7.09, P7.40 | pending |
| 3a | P7.10, P7.11 | pending |
| 3b | P7.12, P7.13 | pending |
| 4a | P7.14, P7.15, P7.16 | pending |
| 4b | P7.17, P7.18 | pending |
| 5a | P7.19, P7.20, P7.36 | pending |
| 5b | P7.21, P7.22, P7.23 | pending |
| 6a | P7.24, P7.25 | pending |
| 6b | P7.26, P7.27, P7.35 | pending |
| 7a | P7.28, P7.29, P7.30 | pending |
| 7b | P7.31, P7.33, P7.34 | pending |
| 8a | P7.37, P7.38, P7.39 | pending |
| 8b | P7.41, P7.42, P7.43 | pending |
| 9 | P7.32 (coordinator) | pending |

## What was implemented

`crates/flexcore-kernel` (18th member): manifest (24 named errors), resolver
(10 errors, protected-kernel guard, deterministic Kahn plans), lifecycle
(10-state §5.6, drain-before-replace, no-auto-restart), Class A registry
(real provider factories, lifecycle-aware swap), signing (provenance-v1,
NoVerifier fail-closed), profiles (five shipped, mode-locked), recomposition
(confirmed switches, no half-applied states), plugin_config (recorded
defaults, secret-ref-only), plugin_path_protection (P4.28/P6.15 discipline,
restart-surviving checkpoints, ADR-010 proposal envelope), revocation
(append-only, key revocation, per-profile staging), tenant_scope
(host-stamped envelopes). `apps/plugin-host` (19th member): wasmtime
`=50.0.1` behind a compile-gated bindings seam (zero wasmtime call sites in
the default build), limits, default-deny capability broker, enforcement-at-
the-door, health (closed vocabulary), audit hash chain (test-grade digest,
honestly bounded), transport trust (config-shaped, rustls wiring = recorded
seam). `wit/` (4 packages, faithful Rust↔WIT mappings, WASI 0.2 only),
`python/workers/` (Class C template + proto contract), `plugins/{manifests,
profiles,scenarios}/`, `docs/plugin-sdk/**` (6 guides + bad-plugin error
catalogue), console Plugins workspace, CI additive jobs (incl. no-dylib
gate), `apps/worker/src/supervision.rs`. Tests: 30+ integration suites
across kernel/host/worker/control-plane + 24 pytest tests + 1 vitest file —
ALL WRITTEN-UNRUN. 21 decision records in this directory.

## How to build and test it / Blockers / Decisions still needed / Assumptions / Known risks

**Build/test (the human's gate pass — nothing here has been compiled):**
`cargo build --workspace` → `cargo clippy --workspace --all-targets -- -D
warnings` → `cargo test --workspace` (kernel, plugin-host, worker
supervision, control-plane) → `cd python/workers && pip install -r
requirements.txt && pytest tests/` (grpcio UNPINNED-REVERIFY) → buf lint
(`python/workers/contracts/`) → `wasm-tools component wit wit/` (1.239.0
ASSUMPTION) → CI green run → the witnessed swap/untrusted run per
`28_exit_gate_run_record.md` (including the recorded seam exposure: two
path deps in `apps/sim-runner/Cargo.toml` + the integration test file).

**Blockers (human gate inputs):** Stages 1–7 NOT HELD; wasmtime 50.0.1
never compiled (CVE floor ≥50.0.0; Rust ≥1.97 vs workspace 1.99.1 — exact-
pin policy to resolve); bindings generation (`cargo component`); buf
registration; grpcio/protobuf pins; D4 cargo-deny with the LLVM-exception
allowlist; secret/dependency scans; D5/D6 still OPEN.

**Decisions still needed:** exact-pin policy for Rust (1.99.1 recorded);
production digest/key material (signing, audit chain); rustls composition
wiring; plugin.inventory.read authz action (interim recorded-drift gate).

**Assumptions (labelled):** wasm-tools 1.239.0; wasmtime 50 feature names;
grpcio API shape; `wasi:cli` 0.2 imports.

**Known risks:** the entire new code is uncompiled — the first build may
surface integration defects; the plugin host has never executed a component
(P7.10 NOT-DONE row); kernel lifecycle/signing AuditSinks do not chain
(recorded seam); broker grants keyed by plugin id (O-13); one recorded
P7.07 audit-wording inconsistency (O-12). Full register:
`30_retrospective_release_gate.md` §2.
