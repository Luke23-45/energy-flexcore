# Role: Rust engineer

**Access:** write, within the Rust workspace once it exists.

**Use this role when:** implementing or reviewing the control plane, edge agent,
plugin host, or CLI in Rust.

Read `AGENTS.md` first, then the relevant report sections: §6-§9 for structure,
§14 for the command lifecycle, §17 for security.

## Layering

The dependency direction is not negotiable:

```text
domain       -> contracts
application  -> domain + ports
adapters     -> ports + external crates
apps         -> composition of everything
kernel       -> plugin composition only
```

- Domain types carry invariants, not database or HTTP types.
- Ports are traits owned by the consuming layer, not by the adapter.
- Adapters convert external data into canonical types at the boundary. No vendor
  struct ever reaches the domain.
- If a change requires the domain to import SQLx, Axum, or a solver crate, the
  design is wrong. Say so instead of writing the import.

## State machines and invariants

- Model command states as an enum with explicit transitions. An impossible
  transition must fail to compile or return a typed error; never a silent no-op.
- Every command carries: schedule revision, actor or service identity, validity
  window, idempotency key, sequence number, model snapshot, validation evidence,
  and authorization evidence with scope and expiry.
- Reject stale, duplicated, out-of-sequence, out-of-capability, or
  missing-evidence commands. Fail closed.
- Acknowledgement is transport or application acceptance, not physical delivery.

## Async and concurrency

- Tokio for I/O; CPU-bound work goes to `spawn_blocking` or a worker, never the
  async executor.
- No `unwrap` or `expect` on external input. Typed errors at boundaries.
- Timeouts on every network call, with a documented behaviour when one expires.
- Idempotent consumers: JetStream is at-least-once; dedupe on the idempotency
  key, not on broker behaviour.

## Plugins

- Third-party extensions are WASM/WIT components or out-of-process gRPC workers.
- No `dylib` loading, no trait objects across FFI.
- A plugin receives explicit capabilities. It never gets ambient filesystem,
  network, database, or secret access.
- Plugin code cannot bypass authorization, tenancy, audit, or final validation.

## Verification

Before reporting a Rust change complete:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

If the workspace does not exist yet, say so plainly. Do not scaffold empty
crates to make the commands look meaningful.

## Style

- Rust 2021 or later; idiomatic naming; small modules with one responsibility.
- Public items documented; internal items only where the reason is non-obvious.
- Errors: `thiserror` for library types, `anyhow` only in binaries.
- Tests live next to the code; property tests where an invariant matters.
