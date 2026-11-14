// P7.10/P7.11 — `flexcore-plugin-host`: the constrained WASM component host
// library and the host capability broker (report §5.4 Class B, §5.7, §8.1,
// §8.3, §17.3/§17.4; ledger `ledger/phase-07-plugin-sdk/ledger_phase_7.md`
// §P7.10, §P7.11).
//
// The WASM plugin host is a SEPARATE PROCESS (report §8.3: "the WASM host is
// extracted for security isolation"). This library is the host's logic; the
// binary root (`main.rs`) is a thin fail-fast wrapper. The host is written
// as an UNTRUSTED-HOST design from day one (ledger §P7.34: the host executes
// untrusted code and is not trusted merely for being first-party) — every
// result it will ever produce is untrusted input to the control plane.
//
// Module map:
//
// | Module      | Responsibility |
// |---|---|
// | [`host`]    | The component host: component artifact loading from a local file path (loading, NOT building), world selection over the P7.09 `wit/` worlds, binding through the compile-gated bindings seam, named host errors |
// | [`bindings`] | The compile-gated bindings seam: the [`bindings::PluginBinding`] trait, the default fail-closed `NoBindings` implementation, and the `generated-bindings` feature gate under which the human's wit-bindgen run materialises the real wasmtime implementation |
// | [`limits`]  | Resource limits derived from the manifest's declared bounds (memory pages, fuel/metering budget, wall-clock epoch deadline, output size, call rate) — every limit exceeded is a named error and the instance is torn down |
// | [`broker`]  | The P7.11 capability broker: explicit scoped grants derived from the manifest's closed permission vocabulary, per-call re-checks (no cached ambient authority), default-deny, audit-or-refuse, and the never-granted set |
// | [`enforcement`] | The P7.12 enforcement gate: the ONLY constructor path to a binding — manifest permissions ∩ profile ceiling, broker scope narrowing, vocabulary-drift refusal, frozen capability set, audit-or-refuse |
// | [`health`] | P7.20: per-plugin health/readiness over the closed vocabulary (mirroring the P6.17 edge discipline), the never-ready-before-first-observation rule, liveness via the host's epoch/probe hooks, the limits.rs exhaustion errors mapped to closed DEGRADED_* reasons, audited transitions, a bounded clonable stats snapshot, and the serializable `HealthSnapshot::json` the P7.25 console consumes — the gRPC/REST transport (P7.20 seam) and mTLS on it (P7.34) are RECORDED seams, not implemented |
// | [`audit_chain`] | P7.33: the TEST-GRADE (FNV-1a 64) per-plugin, per-host-session hash chain over the host-issued audit records — the `ChainedAuditSink` writer (audit-or-refuse preserved: the durable inner sink is written first) and the verifier that walks the chains, names the first broken link, and detects truncation against the platform-retained anchor; the tamper-EVIDENT-not-tamper-PROOF boundary statement is [`audit_chain::AUDIT_INTEGRITY_BOUNDARY`] |
// | [`transport_trust`] | P7.34: the host ↔ platform trust boundary, config-shaped — the fail-closed service-identity assembly (the P6.06 enrollment pattern mirrored; references only, no PKI, no key material), the pinned-CA/plaintext-refusal/TLS-minimum policy (the P6.07 `uplink_mtls.rs` rules restated), the untrusted-host posture table and compromised-host bound (the broker's never-granted set restated as the authorization bound), and the RECORDED transport-wiring seam (no rustls in this wave; the dependency list is the composition owner's) |
// | [`tenant_gate`] | P7.39: the host-side tenant-scope cross-check — the mirrored `HostTenantScope` (authenticated-construction-only), the data gate that admits a request only inside BOTH the broker's grant envelope AND the host-stamped scope (`TENANT_SCOPE_MISMATCH` otherwise, audited; envelope FIRST so a misconfigured grant can never widen the stamped scope), and the recorded worker-tenancy and RLS-relationship statements | the kernel-side type lives in `crates/flexcore-kernel/src/tenant_scope.rs` (documented mirror, no flexcore dependency) |
//
// Honesty status (READ FIRST):
//
// - The wasmtime 50.0.1 pin has NEVER been built or run in this loop (the
//   delivery loop is documentation-and-design only; no cargo command was
//   executed). Every wasmtime call site carries an
//   `// UNVERIFIED — wasmtime 50 API, first human build verifies` comment
//   and lives inside the `generated-bindings` feature gate. The default
//   build contains ZERO wasmtime call sites — it compiles against
//   std + tracing only and fails closed at the bindings seam.
// - No component artifacts exist in this loop. The host returns named
//   errors when a component path is unconfigured or absent; it runs only
//   when a human supplies a signed component AND a bindings-enabled build.
// - Bindings generation is a BUILD-TIME step owned by the human's
//   `cargo component` / `wit-bindgen` run over the `wit/` tree (recorded in
//   `wit/TOOLCHAIN.md` and `40_toolchain_pinning.md`); this host instantiates
//   against the recorded worlds by path reference and never re-declares the
//   WIT contract in code.
//
// Dependency direction (§8.2, enforced by `tools/check_dependency_rule.py`,
// which classifies this crate as COMPOSITION): this crate may use
// infrastructure (wasmtime, tracing); it depends on NO other flexcore crate.
// In particular the permission vocabulary in [`broker`] is a documented
// MIRROR of `crates/flexcore-kernel/src/manifest.rs` (the kernel crate is
// owned by another wave; the alignment seam is recorded in the decision
// record §6 — kernel side untouched).
//
// House rules throughout: fail-closed everywhere, named errors with stable
// machine names, no `unwrap`/`expect` on untrusted input, no `todo!()`/
// `unimplemented!()`, simulation-only (the entire 0.x line; field write
// authority begins at v1.0.0 with operator approval — nothing here grants a
// write path).

// P7.33 — audit-chain tamper-evidence over the host-issued audit records
// (see the module header for the TEST-GRADE digest honesty and the
// boundary statement; decision record
// `ledger/phase-07-plugin-sdk/33_34_audit_integrity_trust_boundary.md`).
pub mod audit_chain;
pub mod bindings;
pub mod broker;
pub mod enforcement;
pub mod health;
pub mod host;
pub mod limits;
// P7.39 — the host-side tenant-scope cross-check (the mirrored scope +
// the data gate over the broker's per-call narrowing; see the module
// header for the mirror seam and the RLS relationship).
pub mod tenant_gate;
// P7.34 — the host ↔ platform trust boundary, config-shaped (service
// identity, pinned CA, untrusted-host posture; the mTLS transport itself
// is the RECORDED composition seam, no new dependency in this wave).
pub mod transport_trust;
