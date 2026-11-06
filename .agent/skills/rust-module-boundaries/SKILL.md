---
name: rust-module-boundaries
description: Applies FlexCore's Rust layering rules when adding a crate, port, adapter, or domain type. Use for any Rust implementation work, new bounded context, dependency-direction review, or when deciding where a piece of logic belongs.
metadata:
  author: neryva
  version: "1.0"
---

# Rust module boundaries

The architecture is a modular monolith with hexagonal boundaries. The value is
in the dependency direction, not in the folder names.

## Target crates

```text
crates/
  flexcore-contracts        shared value types, API-facing types
  flexcore-domain           entities, aggregates, invariants
  flexcore-ports            capability interfaces
  flexcore-application      use cases and orchestration
  flexcore-kernel           composition root, plugin lifecycle
  flexcore-authz            policy decisions
  flexcore-audit            append-oriented evidence
  flexcore-events           event vocabulary, outbox abstraction
  flexcore-storage          persistence abstractions
  flexcore-optimization-api solver/provider contract
  flexcore-grid-api         grid model and feasibility contract
  adapters/*                postgres, nats, openems, openadr, sunspec, market
apps/
  control-plane, worker, edge-agent, plugin-host, sim-runner
```

## Dependency direction

```text
contracts   <- domain <- application -> ports
                            ^
adapters -------------------+
apps -> everything (composition only)
```

Rules:

- `domain` depends on `contracts` and the standard library. Nothing else.
- `application` depends on `domain` and `ports`.
- `ports` define traits; they import no adapter crate.
- `adapters` depend on `ports` and external crates.
- `apps` wire concrete types. No business rules live in `main.rs`.
- `kernel` composes plugins. It must not become a second domain layer.

## Where does this logic go?

| Logic | Home |
|---|---|
| "A command cannot be issued after expiry" | domain (invariant) |
| "Validate then approve then issue" | application (use case) |
| "Call the optimizer worker" | port + adapter |
| "Map SunSpec point to canonical telemetry" | adapter |
| "Which plugins are enabled" | kernel / config |
| "Serialize this to JSON" | contracts or adapter, never domain |

## Adding a bounded context

1. Confirm it owns data and rules that no existing context owns. If it shares
   ownership, it is a module, not a context.
2. Create the domain types and invariants first, with unit tests.
3. Define the ports the application needs. Do not define a port for a dependency
   you do not have.
4. Implement adapters behind those ports.
5. Wire it in the composition root.
6. Add the context to the module table in report §4 if the report describes it.

Do not create the crate until step 2 has real code. Empty crates are noise.

## Anti-patterns

- `use sqlx::...` in the domain.
- A domain type with `serde` derive used as the public API contract.
- An application service that constructs a concrete adapter.
- A port trait with one implementation and no test double.
- A "utils" crate that everything depends on.
- Shared mutable state across contexts instead of an event or an explicit call.
- An adapter returning `serde_json::Value` into the application layer.

## Review checklist

- [ ] Does any import point "upward" against the arrow diagram?
- [ ] Is every external type converted at the boundary?
- [ ] Are invariants enforced by types or constructors, not by callers?
- [ ] Are errors typed, or is there an `unwrap` on external input?
- [ ] Is CPU-heavy work off the async executor?
- [ ] Does the change add a crate that has no code yet?
