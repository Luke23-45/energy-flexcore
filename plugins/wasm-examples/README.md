# `plugins/wasm-examples/` — planned example component (DESIGN note)

**Status: DESIGN ONLY.** No component source, no `Cargo.toml`, no build has
happened — nothing may be fetched or built in the delivery loop that
produced this note (ledger P7.09 step 5 anticipates an example; this README
records its design so the human/P7.24 wave can materialise it without
re-deriving the shape). The build commands below are PREPARED-UNRUN, listed
verbatim for the human who runs them; the toolchain pins they depend on are
in `wit/TOOLCHAIN.md`.

## 1. What the example is

A tiny **optimizer echo component** ("optimizer-echo"): the smallest
possible Class B component that implements the
`flexcore:optimizer@1.0.0` world (`wit/flexcore-optimizer@1.0.0/`,
world `optimizer-plugin`). It is the first thing the P7.10 Wasmtime host
prototype runs end to end (ledger P7.10 step 4: "one example component end
to end"), before any real solver plugin exists.

Deliberately NOT an example of a real optimizer:

- It declares minimal capabilities (LP only, no warm start, no duals) so
  the capability-declaration path is exercised honestly.
- Its `solve` echo behaviour: after validating the incoming problem shape,
  it returns a §12.6-conformant result whose primal diagnostics echo the
  problem's variables with a trivial deterministic value (e.g. the lower
  bound when present, else `0.0`), status `feasible`, with full provenance
  (provider/solver identity, input snapshot from the problem provenance,
  determinism record with the config digest) — so the P7.21 conformance
  suite can use it as the minimal passing fixture.
- It never returns `optimal` (that would claim proven optimality it cannot
  provide — AGENTS.md rule 1 applies to examples too).

## 2. Contract conformance requirements

| Requirement | Source |
|---|---|
| Exports exactly the `optimizer` interface of `flexcore:optimizer/optimizer-plugin@1.0.0`; imports nothing (no `wasi:`) | `wit/flexcore-optimizer@1.0.0/optimizer.wit`; report §5.4 |
| Result carries full §9.2 provenance and diagnostics; a result without them must fail the conformance suite | Report §9.2; ledger P7.21 |
| Status vocabulary exactly the six §12.6 strings | `wit/README.md` §3.1 mapping table |
| Units travel with values (declared-units on the objective echo; variable units from the incoming problem) | Report §10.2; `flexcore:core` types |
| Plugin outputs are proposals only — the echo result is a candidate, never authorization | ADR-010; §13.4 |

## 3. Planned layout (when materialised)

```text
plugins/wasm-examples/optimizer-echo/
  Cargo.toml          # crate-type cdylib; wasip2 bindings; component metadata
  src/lib.rs          # the echo implementation
  wit/                # symlink/copy strategy for the flexcore:* packages — decided at build time
  README.md           # build + run against the P7.10 host, recorded outputs
```

## 4. Build commands (PREPARED-UNRUN — for the human)

```sh
# 1. Pin check: record versions actually installed (wit/TOOLCHAIN.md pins;
#    cargo-component is UNPINNED-REVERIFY — check its release page first).
cargo component --version
wasm-tools --version

# 2. Create the component crate (name and path per §3).
cargo component new plugins/wasm-examples/optimizer-echo --lib

# 3. Target world: point the crate's wit/ at wit/flexcore-optimizer@1.0.0/
#    plus wit/deps/flexcore-core@1.0.0/ (binding strategy recorded in the
#    crate README when built).

# 4. Build.
cargo component build --release
```

Expected outcome: a component binary whose WIT surface resolves against
`flexcore:optimizer/optimizer-plugin@1.0.0` with zero WASI imports
(`wasm-tools component wit` on the produced artifact must show the export
and no ambient imports).

## 5. Ownership

- Materialising, building, and testing this example is owned by the
  **P7.10 host-prototype wave** (which must run one example component end
  to end) and documented as a published SDK example by **P7.24** (which
  also ships the deliberately-bad example).
- This README is the P7.09 design note only. It creates no code and makes
  no build claim.
