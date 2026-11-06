# Phase 7 — Plugin SDK and Modular Composition: Exit-Gate Checklist

| Field | Value |
|---|---|
| Phase | 7 — Stage 7 Plugin SDK and modular composition |
| Report trace | §23 Stage 7 |
| Release | v0.5.0 — Plugin SDK |

## §23 Stage 7 exit gate (verbatim substance)

> Swap one optimizer/grid provider without changing application/domain code;
> untrusted plugin cannot access ungranted capabilities.

Each item below mirrors that gate (plus the §23 Stage 7 deliverables list).
Tick an item only when the named evidence exists. Evidence is a file, a
test run, or a signed record — not an assertion in chat.

## Deliverables (§23 Stage 7)

- [ ] **Plugin manifests and schema exist (versioned, validated, published)** — Evidence: manifest JSON Schema + validator + example manifests + differences-from-§5.5 note. (P7.05)
- [ ] **Capability seams mapped (contract → provider → consumer)** — Evidence: seam map with solver-class declarations; zero direct consumer→provider imports. (P7.04)
- [ ] **Built-in Rust provider registry works** — Evidence: registry implementation + registered built-ins + composition-swap test. (P7.08)
- [ ] **Plugin profiles/bundles exist and load** — Evidence: profile definitions + loader/validator + per-profile composition record; no profile silently enables a control-capable plugin. (P7.15)
- [ ] **Wasmtime/WIT sandbox prototype runs** — Evidence: `apps/plugin-host/` running an example component behind a WIT world on the WASI 0.2 baseline, with resource limits applied and a recorded Wasmtime version ≥50.0.0 (or a fixed backport). (P7.10)
- [ ] **Python gRPC worker template works** — Evidence: template + example worker + Protobuf schemas (`buf lint` passing) + crash-isolation test. (P7.17)
- [ ] **Plugin conformance suite exists and runs in CI** — Evidence: CI run of the suite against all three plugin classes on the same contract; a result without §9.2 provenance/diagnostics fails. (P7.21)

## Exit gate

- [ ] **One optimizer provider swapped without changing application/domain code** — Evidence: P7.22 swap-test record; `git diff` over application/domain code is empty; solver-class declarations correct for the provider used.
- [ ] **One grid provider swapped without changing application/domain code** — Evidence: P7.22 swap-test record (second swap); same empty-diff condition; any model-fidelity change recorded (D3).
- [ ] **Untrusted plugin cannot access ungranted capabilities (WASM class)** — Evidence: P7.23 negative-test record for a WASM component attempting cross-tenant reads, authorization, issue, secret read, network access, and cross-module writes — every attempt denied and audited.
- [ ] **Untrusted plugin cannot access ungranted capabilities (gRPC class)** — Evidence: P7.23 negative-test record for a gRPC worker attempting the same set — every attempt denied and audited.
- [ ] **No arbitrary Rust dylib loading exists** — Evidence: CI no-dylib check (dependency ban + code search) green on the phase branch, plus the scratch-branch negative demonstration (P7.35).
- [ ] **Protected kernel intact** — Evidence: enforcement matrix (P7.12) showing `CommandAuthorizer`/`DispatchGateway`/tenant isolation/audit/lifecycle/final validation are not plugin-replaceable; escalation attempts fail.
- [ ] **Sandbox negative tests pass** — Evidence: P7.13 hostile-component suite (filesystem, network, environment, secrets, exhaustion, abusive call rates) — host survives every case.
- [ ] **Signing/provenance/SBOM in place** — Evidence: unsigned/tampered packages rejected; SBOM per package; rollback works; no self-upgrade or runtime code fetch. (P7.14)
- [ ] **WASI 0.2 baseline recorded; WASI 0.3 not claimed** — Evidence: baseline record with re-evaluation trigger; no doc or code claims WASI 0.3 support in v0.5.0. (P7.09, P7.32)
- [ ] **CI gates green on the SDK** — Evidence: CI run showing `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, Python Ruff/type/pytest, Buf lint/breaking, manifest + WIT validation, SBOM, scanning. (P7.27)
- [ ] **Security tests pass with zero open high/critical findings** — Evidence: §19.2(7) security-test report for the plugin surface. (P7.29)
- [ ] **Operator console shows plugins, permissions, versions, health** — Evidence: console walkthrough record; no secret values displayed; high-impact actions require confirmation. (P7.25)
- [ ] **SDK documentation and examples complete** — Evidence: docs under `docs/plugin-sdk/` + working examples + the deliberately bad example with its recorded rejection. (P7.24)
- [ ] **Handover to Phase 8 written** — Evidence: handover note listing research profiles, reusable contracts, the proposal-only boundary (ADR-010), and non-deliverables. (P7.31)
- [ ] **Retrospective and v0.5.0 release-gate record written** — Evidence: retrospective + open-items register + §22.3 position statement containing the simulation/shadow limitation in plain language. (P7.32)

- [ ] **Plugin audit records are tamper-evident** — Evidence: tamper/survival test record; a profile switch that drops audit coverage is refused. (P7.33)
- [ ] **Plugin-host trust boundary enforced (mTLS, untrusted host)** — Evidence: host identity/mTLS design + compromised-host test record; revoked identity refused a session. (P7.34)
- [ ] **Replay/sequence/expiry protection on the plugin path** — Evidence: replay/expired/out-of-sequence attempts rejected and audited; duplicate delivery processed once. (P7.36)
- [ ] **§17.5 safety invariants mapped to tests** — Evidence: invariant→test map with all ten invariants and the invariant-9 stale-model rejection test. (P7.37)
- [ ] **Revocation, staged rollout, failed-plugin removal tested** — Evidence: revocation/rollout/removal test record. (P7.38)
- [ ] **Cross-component isolation and data-level tenant filtering tested** — Evidence: isolation + data-filtering test record for WASM and gRPC classes. (P7.39)
- [ ] **WIT/WASM toolchain versions pinned** — Evidence: toolchain pin record + CI pin check. (P7.40)
- [ ] **Manifest validator fuzz/property tested** — Evidence: fuzz/property run record with zero crashes/hangs. (P7.41)
- [ ] **Plugin-path replay reproduces within tolerance** — Evidence: seeded replay record filed with the exit-gate evidence. (P7.42)
- [ ] **Scenario-regression suite defined and green in CI** — Evidence: versioned suite definition + green CI run. (P7.43)

## What this gate does NOT establish

- It does not establish certified conformance of any plugin, safety
  qualification, or field-control approval. v0.5.0 remains simulation/shadow;
  no plugin can bypass dispatch gates; operator-approved control is the
  v1.0.0 default after Stage 9 qualification (§21.4, §22.3).
- It does not establish WASI 0.3 support, GNN/DMPC/MARL capability, or any
  research provider. Those belong to Phase 8 (tracked) or later.
