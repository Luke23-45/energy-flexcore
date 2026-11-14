# The FlexCore plugin SDK — overview

| Field | Value |
|---|---|
| Audience | Plugin authors (Class A built-in providers, Class B WASM component authors, Class C gRPC worker authors) |
| Baseline | v0.5.0 SDK (platform plugin-API version `0.5.0`, `crates/flexcore-kernel/src/resolve.rs::CURRENT_PLATFORM_API_VERSION`) |
| Subtask | P7.24 (`ledger/phase-07-plugin-sdk/ledger_phase_7.md` §P7.24) |
| Status of the built system | The kernel and host code this guide describes is WRITTEN, NOT BUILT and NOT RUN (the loop bans); every never-run step in these pages is labelled PREPARED-UNRUN with its exact command. Nothing here is a tool-verified fact. |
| Control-authority boundary | Simulation and shadow operation ONLY (report §21.4). Field write authority begins at v1.0.0 with operator approval. No plugin, profile, or SDK document changes that. |

## 1. What the SDK is

The FlexCore plugin SDK is the set of contracts and seams by which
third-party code participates in the FlexCore platform WITHOUT ever holding
authority the platform kernel keeps for itself. Three things define it:

1. **Capability seams** (ADR-005, report §5.2): a plugin exists in the
   composition as the PROVIDER of a named contract (`<domain>.<capability>`
   id), consuming other named contracts. It never reaches into the platform;
   the platform calls it through the seam. Every seam has a real Rust trait
   behind it (report §9.1); the SDK never invents a contract without one.
2. **The three plugin classes** (report §5.4): see §2 below.
3. **The protected kernel list** (report §5.1, §5.7, §9.1; ADR-005 made
   structural in `crates/flexcore-kernel/src/resolve.rs::PROTECTED_KERNEL_CONTRACT_IDS`):
   six contract ids NO plugin may ever declare in `provider-of`:

   | Protected contract id | What it owns (report §5.1/§9.1) |
   |---|---|
   | `kernel.command-authorizer` | dispatch authorization |
   | `kernel.dispatch-gateway` | dispatch state transitions |
   | `kernel.dispatch-validation` | final dispatch validation |
   | `kernel.command-lifecycle` | the command lifecycle gates |
   | `kernel.audit-integrity` | append-only audit integrity |
   | `kernel.tenant-isolation` | tenant isolation |

   A claim is refused with the named error `PROTECTED_KERNEL_REPLACE`
   before any composition is computed. This is not a policy preference; it
   is a structural refusal in the resolver.

## 2. The three plugin classes (report §5.4)

| Class | Manifest `class` value | Form | Guide |
|---|---|---|---|
| A | `builtin-rust` | Trusted built-in Rust module, registered at the composition root | [class-a-builtin.md](class-a-builtin.md) |
| B | `wasm-component` | Sandboxed WASM component (Wasmtime host; WIT world) | [class-b-wasm.md](class-b-wasm.md) |
| C | `grpc-worker` | Out-of-process gRPC worker (Python numerical/ML code) | [class-c-worker.md](class-c-worker.md) |

Arbitrary Rust `dylib` loading is REJECTED for the whole 1.x line (report
§5.4 "Explicitly rejected"). There is no Class D.

## 3. What a plugin author CAN build

| You can | Because |
|---|---|
| Provide a forecast, grid-model, telemetry-source, or optimizer contract | The §5.2 provider seams with implemented Rust mirrors (`wit/README.md` §1 inventory) |
| Read scoped telemetry and asset metadata | `telemetry.read` / `asset.read` — §5.7 initial policy, granted tenant/site/asset-scoped by the capability broker (P7.11) |
| Publish forecasts after schema validation | `forecast.publish` |
| PROPOSE schedules and dispatch candidates | `schedule.propose` / `dispatch.propose` — the output is a CANDIDATE (§13.4, ADR-010): it enters the protected validation → feasibility → authorization path with no authority of its own |
| Request validation from the protected validator | `dispatch.validate.request` — a send-only forward to exactly `protected-validator` (P7.11 broker); the forward returns nothing back |
| Declare configuration properties and secret REFERENCES | [configuration-and-secrets.md](configuration-and-secrets.md) |

## 4. What a plugin author CANNOT build (and why "cannot" is literal)

The dispatch-bypassing plugin is not "discouraged" — it is **unbuildable**.
The enforcement story, told plainly (decision record
`ledger/phase-07-plugin-sdk/12_13_enforcement_negatives.md`):

| Attempt | Where it dies | Named error (verified against source) |
|---|---|---|
| Manifest requests `dispatch.authorize`, `dispatch.issue`, `secret.read`, or `network.any` | The manifest validator — the earliest gate | `PERMISSION_NEVER_GRANTED` (`crates/flexcore-kernel/src/manifest.rs`) |
| A permission id that is not on the §5.7 vocabulary at all | The manifest validator | `PERMISSION_ID_UNKNOWN` |
| The same never-granted id arrives through any later path (hand-built grant request, host enforcement) | The capability broker AND the enforcement gate re-refuse it | `PERMISSION_NEVER_GRANTABLE` (`apps/plugin-host/src/broker.rs`, `enforcement.rs`) |
| Manifest declares a permission its profile ceiling does not permit | Profile plan validation, then the host door | `PROFILE_PERMISSION_CEILING_EXCEEDED` (kernel `profile.rs`), `PERMISSION_ABOVE_PROFILE` (host `enforcement.rs`) — refused, never silently trimmed |
| Manifest claims a protected kernel contract | The resolver, before composition | `PROTECTED_KERNEL_REPLACE` |
| A component calls for an ungranted capability at use time | The broker's per-call check | `CAPABILITY_NOT_GRANTED` (defaults deny), `SCOPE_OUTSIDE_GRANT`, `GRANT_EXPIRED` |
| A component tries ambient filesystem/network/environment access | It has no import to do it: WIT worlds carry zero `wasi:` imports, and the broker refuses the ids | `PERMISSION_UNKNOWN` / `PERMISSION_NEVER_GRANTABLE`; the world-level proof is `wit/README.md` §5 item 8 |
| A plugin path request claims a protected contract | The P7.36 path-protection verifier | `PROTECTED_CONTRACT_CLAIM` |
| A plugin output pretends to be an authorization | Type-level: the broker's forward path returns `()` — there is no verdict to carry | (no error name; the signature has no verdict in it) |

The §5.7 capability names `dispatch.authorize` and `dispatch.issue` do not
exist on the platform authorization action vocabulary either
(`crates/flexcore-authz/src/action.rs` — they parse to nothing, so they can
only ever be denied). Plugin outputs are PROPOSALS by construction.

## 5. The v0.5.0 simulation/shadow boundary

- The whole 0.x line is simulation-only (report §21.4). The shipped
  profiles all carry `"control-authority": "simulation-shadow"`, and that
  value is the ONLY one the profile loader accepts — any other §21.4 mode
  name is the named error `CONTROL_AUTHORITY_MODE_FORBIDDEN`
  (`crates/flexcore-kernel/src/profile.rs`). A profile, like a plugin, can
  never select or raise the control-authority mode; the mode is a
  deployment and per-site setting.
- Plugin proposals are candidates even in shadow mode: a proposal gains
  nothing from having been proposed.

## 6. Reading order

1. [manifest-guide.md](manifest-guide.md) — authoring and validating the
   manifest (every plugin class starts here).
2. Your class guide: [class-a-builtin.md](class-a-builtin.md),
   [class-b-wasm.md](class-b-wasm.md), [class-c-worker.md](class-c-worker.md).
3. [lifecycle-and-profiles.md](lifecycle-and-profiles.md) — the states your
   plugin passes through and the profile that composes it.
4. [configuration-and-secrets.md](configuration-and-secrets.md) — operator
   configuration and secret references.
5. [examples/](examples/) — the good minimal worker example and the
   deliberately bad plugin whose every field is a named rejection.

## 7. Honest status of this documentation

These pages describe the BUILT system: every error name, type name, field
name, and constant cited here was verified against the source files named
inline (read 2026-11-13). Every step that requires a tool this loop never
ran (`wasm-tools`, `cargo component`, grpcio, wasmtime execution, the
conformance suite) is labelled PREPARED-UNRUN with its exact command and
its source of pins (`wit/TOOLCHAIN.md`). No certification, safety, or
conformance claim is made anywhere in the SDK documentation; the first
build/test pass is the verification gate for everything.
