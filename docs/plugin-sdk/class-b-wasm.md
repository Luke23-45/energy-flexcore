# Class B — sandboxed WASM components

| Field | Value |
|---|---|
| Manifest `class` | `wasm-component` |
| Entry point | `symbol` — the WIT-exported symbol id (e.g. `neryva:optimizer/lp-solve@0.5.0`) |
| Host | Wasmtime (pin 50.0.1, hard floor ≥ 50.0.0 for the October 2026 CVE fixes — `wit/TOOLCHAIN.md` §2) |
| Enforcement location | `apps/plugin-host/src/enforcement.rs` (P7.12) + `apps/plugin-host/src/broker.rs` (P7.11) |

## 1. What a Class B plugin is

A WASM component implementing a FlexCore WIT world, instantiated by the
plugin host with EXACTLY the capability set derived from
(manifest permissions ∩ profile ceiling) filtered through the broker's
scope narrowing — and nothing else. The WIT worlds carry **zero `wasi:`
imports**: no ambient filesystem, network, environment, clock, or
subsystem access exists to reach for (`wit/README.md` §2 design-for-host
note).

## 2. The WIT worlds (v1.0.0 packages)

| Package | World | Exports | Rust mirror |
|---|---|---|---|
| `flexcore:optimizer@1.0.0` | `optimizer-plugin` | `optimizer` | `flexcore-optimization-api/src/port.rs::Optimizer` |
| `flexcore:grid@1.0.0` | `grid-model-provider-plugin` | `grid-model-provider` | `flexcore-grid-api/src/port.rs::GridModelProvider` |
| `flexcore:telemetry@1.0.0` | `telemetry-source-plugin` | `telemetry-source` | `flexcore-ports/src/telemetry_source.rs::TelemetrySource` |
| `flexcore:core@1.0.0` | — (shared data vocabulary) | — | `flexcore-contracts` ids/quality/units/time model |

The full function- and type-level mapping tables are `wit/README.md` §3 —
nothing is invented; every WIT item mirrors a real Rust trait method.
Deliberately EXCLUDED: any authorization world (the protected kernel is
never a plugin contract), WASI 0.3 syntax (`stream<T>`, `future<T>`,
`async`), and worlds for contracts with no Rust counterpart (the recorded
`wit/README.md` §4 exclusions).

## 3. Authoring workflow

1. **Author the world usage, not a new world.** A component TARGETS an
   existing package (`wit/flexcore-optimizer@1.0.0/` etc. plus
   `wit/deps/flexcore-core@1.0.0/`). A new world is a reviewed schema
   change (SemVer for worlds: breaking change ⇒ major bump, new package
   directory; `wit/README.md` §2).
2. **Export exactly the world's interface; import nothing.** The world
   name carries a `-plugin` suffix distinct from the interface name.
3. **Manifest**: `"class": "wasm-component"`, entry-point key `symbol`,
   permissions from the §5.7 vocabulary, resource bounds declared (the
   host enforces the DECLARED bounds and re-validates them at the door).
4. **Provenance**: the component artifact goes through the P7.14
   digest-and-record gate (`flexcore-plugin-provenance-v1`; signer key
   REFERENCE only, SBOM reference REQUIRED; the fail-closed `NoVerifier`
   default refuses everything until a deployment composes a real verifier).
   Refusal names: `UNSIGNED_ARTIFACT`, `DIGEST_MISMATCH`, `MANIFEST_MISMATCH`,
   `DIGEST_ALGORITHM_MISMATCH`, `VERIFIER_REFUSED`.

## 4. Binding generation — the HONEST, UNVERIFIED status

The bindings seam (`apps/plugin-host/src/bindings.rs`) is fail-closed
`NoBindings` in the default build: instantiation refuses with
`BINDINGS_NOT_GENERATED` (`HostError::BindingsNotGenerated`; the health
vocabulary renders it `FAILING_BINDINGS`). The `wit-bindgen`-generated
guest/host bindings have NEVER been generated or validated in this loop —
no binding-generation claim is made anywhere.

The toolchain steps are PREPARED-UNRUN, pinned in `wit/TOOLCHAIN.md`
(`wasm-tools` 1.239.0 ASSUMPTION — re-verify at the release page;
`wit-bindgen` and `cargo-component` UNPINNED-REVERIFY):

```sh
# 1. Validate the WIT packages (parse + resolve; expected: the three worlds print):
wasm-tools component wit wit/

# 2. Build an example component (after plugins/wasm-examples/ is materialised
#    per its README; cargo-component is UNPINNED-REVERIFY):
cargo component build --release
```

Record the tool version output (`wasm-tools --version`,
`cargo component --version`) next to the results. Until a recorded run
exists, every statement about what these tools accept is a design
statement, not a tool-verified fact (`wit/README.md` §0).

## 5. What the host enforces at the door (fail-closed)

| Refusal | Named error (verified against `apps/plugin-host/src/{enforcement,broker}.rs`) |
|---|---|
| Permission id not in the vocabulary | `PERMISSION_UNKNOWN` |
| Never-granted id (re-refused at the door) | `PERMISSION_NEVER_GRANTABLE` |
| Permission above the profile ceiling | `PERMISSION_ABOVE_PROFILE` (refused, never trimmed) |
| Duplicate permission declaration | `PERMISSION_DUPLICATE_DECLARATION` |
| Unalignable resource field (e.g. the ledger's `timeoutMs` spelling vs the kernel's `timeout-ms`; any fuel/concurrency field the kernel schema does not carry) | `PERMISSION_VOCABULARY_DRIFT` — never a default interpretation |
| Use of a capability not granted | `CAPABILITY_NOT_GRANTED` |
| Use outside the grant's scope envelope | `SCOPE_OUTSIDE_GRANT` |
| Use after expiry | `GRANT_EXPIRED` |
| Limit exhaustion (memory / fuel / deadline / output / call rate) | `MEMORY_LIMIT_EXCEEDED` / `FUEL_EXHAUSTED` / `DEADLINE_EXCEEDED` / `OUTPUT_LIMIT_EXCEEDED` / `CALL_RATE_EXCEEDED` (the host errors the health vocabulary maps to `DEGRADED_*`/`FAILING_*`) |
| Bindings not generated | `BINDINGS_NOT_GENERATED` |

The enforced capability set is FROZEN at instantiation; a later manifest or
ceiling change can only be picked up through re-instantiation via
drain-before-replace. Every allow and every refusal is audited
(audit-or-refuse in both directions).

## 6. Candidate-only rule (§13.4, ADR-010)

`schedule.propose` / `dispatch.propose` grants make the component's output
a CANDIDATE: it enters the protected validation → feasibility →
authorization path and gains no authority from having been proposed. There
is no authorization id in any vocabulary, and the broker's validation
forward returns `()` — no verdict can flow back through a signature that
has no verdict in it.

## 7. Honest status

The host crate is WRITTEN, NOT BUILT and NOT RUN. The sandbox negative
suite (`apps/plugin-host/tests/sandbox_p7_13.rs`) and enforcement tests
(`enforcement_p7_12.rs`) are written-unrun; the first `cargo test -p
flexcore-plugin-host` is the human verification gate. No sandbox-escape
coverage is claimed beyond what the seams provide.
