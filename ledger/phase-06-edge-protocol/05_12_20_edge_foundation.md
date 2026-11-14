# P6.05 + P6.12 — Edge agent crate, composition-root design, and the protocol-adapter interface/ACL

Date: 2026-11-12. Status: **IMPLEMENTED — awaiting human build/test** (wave 1b
of Phase 6; the loop rules in `ledger/phase-06-edge-protocol/ENTRY_NOTE.md`
forbid building, running tests, and installing: everything below is written
code, never executed). Report trace: §18.1, §8.1–§8.3, §6.2, §6.4, §3.3,
§9.1, §5.3, §10.2–§10.4, §2.3. Ledger: P6.05, P6.12.

Dependency context (P6.04 readiness, supplied by the coordinator): the
Stage 1–5 artifacts exist in-tree but their demonstration gates are **NOT
HELD** (Stage 5 exit record UNRUN/unsigned,
`ledger/phase-05-operator-console-simulator/30_stage5_exit_record.md`);
Phase 6 proceeds on the recorded gate exception (ENTRY_NOTE §"Gate
exception"). P6.05/P6.12 build on that exception rather than asserting
Stage 1–5 completion.

---

## 1. What this record decides

1. **The edge agent exists as a workspace crate** — `crates/edge-agent`
   (package `flexcore-edge-agent`, binary `edge-agent`), with its port
   surface, supervision runner, configuration, and composition root — and
   NOTHING else is scaffolded: no adapter crate, no enrollment code, no
   command receiver, no plugin host (P6.05 step 3; AGENTS.md §1: no crates
   "to be ready"; no empty crates).
2. **The protocol-adapter interface and anti-corruption layer are defined
   inside the edge agent crate** (`crates/edge-agent/src/adapters/`), as
   the port holder. The concrete adapter crate
   (`crates/adapters/openems`, P6.10/P6.13) does not exist yet and will
   depend on THIS crate for the port + ACL types — adapters depend on
   ports, never the reverse (§8.2).
3. **The command path is absent, not disabled-by-flag-only.** No receive,
   verify, or forward code for commands exists in the crate; the config
   flag `command_path_enabled` defaults to FALSE and a `true` value is
   logged as inert (P6.14 owns the real receiver and its written
   enablement gate). Nothing implies a write path is active.
4. **D5 stays OPEN**; the adapter ids recorded in the registry follow the
   RECOMMENDED default (OpenEMS first, report §2.3) while the decision is
   pending: `openems-rest` = PLANNED (P6.10/P6.13), `generic-rest` =
   RESERVED (a recorded slot, never implemented — no hallucinated APIs,
   AGENTS.md rule 1).

## 2. Placement decision (recorded deviation from report §8.1)

The report §8.1 target tree names `apps/edge-agent`; this wave places the
crate at `crates/edge-agent`, classified COMPOSITION in
`tools/check_dependency_rule.py` like `apps/worker`.

| Option | Content | Decision |
|---|---|---|
| `crates/edge-agent` (CHOSEN, DEFAULT) | Composition crate beside the other composition roots; `check_dependency_rule.py` gains one COMPOSITION_CRATES entry + one MEMBER_DIRS entry | minimal additive change to the P1.04 enforcement tool; keeps the wave's diff surgical (AGENTS.md rule 6) |
| `apps/edge-agent` (report §8.1 literal) | Same classification via the existing `apps/*` rule | would rename the directory the wave plan already owns; the coordinator's file-ownership assignment for wave 1b names `crates/edge-agent/**` |

The deviation is recorded here and in the root `Cargo.toml` member comment;
reconciling the §8.1 tree with the actual layout (or moving the directory)
is a candidate P6.30 item — it is a mechanical rename, deliberately not
absorbed silently.

## 3. Composition-root design (the P6.05 design note, in text)

### 3.1 Crate layout

```text
crates/edge-agent/
  Cargo.toml            composition-crate manifest (tokio, time, tracing,
                        tracing-subscriber, flexcore-contracts, flexcore-ports)
  src/lib.rs            module map + the offline-tolerant/outbound-only/read-only
                        structural statement
  src/config.rs         EdgeConfig: site id, tenant id, control-plane endpoint
                        (redacted in Debug), buffer path, adapter id,
                        command-path flag (DEFAULT FALSE), cadence/batch/bounds
  src/ports.rs          the edge-local ports + EdgeTelemetrySample + in-memory
                        implementations + fail-closed stubs
  src/runner.rs         supervision loop: poll → buffer → forward, degraded-mode
                        state machine, audited backpressure, graceful shutdown
  src/clock.rs          WallClock (the P1.07 Clock port on the platform clock;
                        no database dependency to tell the time)
  src/adapters/
    mod.rs              P6.12 module map + adapter-selection state
    port.rs             EmsAdapter port: discover / read / capabilities /
                        health — NO dispatch method (structurally absent)
    acl.rs              the anti-corruption layer: mapping table + mapping rules
    registry.rs         the adapter allowlist (openems-rest PLANNED,
                        generic-rest RESERVED)
  src/main.rs           the binary composition root (fail-fast config →
                        registry check → honest degraded composition →
                        signal-driven shutdown)
  tests/config_and_runner.rs          P6.05 tests (UNRUN)
  tests/adapter_acl_and_registry.rs   P6.12 tests (UNRUN)
```

### 3.2 The composition-root diagram

```text
                     ┌──────────────────────────────────────────────────────┐
   site EMS          │                edge-agent (composition root)         │
 (OpenEMS, later)    │                                                      │
      │              │   EdgeConfig ──► AdapterRegistry (allowlist check)   │
      ▼              │        │                                             │
┌─────────────┐  implements   ▼                                             │
│  P6.10/P6.13 │──EmsAdapter──► adapters::acl ──► EdgeTelemetrySample        │
│ openems-rest │  + ACL       (mapping table)        (canonical)             │
│ (crates/     │                                             │               │
│ adapters/    │                                             ▼               │
│ openems)     │                          ┌──────────────────────────┐       │
└─────────────┘                          │       EdgeRunner         │       │
      ▲                                  │  poll → buffer → forward │       │
      │ read (outbound to EMS API)       └───┬───────────┬──────────┬┘       │
      │                                      │           │          │        │
      │                        EmsSource ────┘   UplinkChannel ────┐  │        │
      │                        (P6.08/P6.10)     (P6.07 outbound   │  │        │
      │                                           mTLS; NO inbound)│  │        │
      │                                    LocalBuffer ────┐       │  │        │
      │                                    (P6.09 durable; │       │  │        │
      │                                     this wave: in- │       │  │        │
      │                                     memory, honest)│       ▼  │        │
      │                                    AuditSink ──────┼──► central platform
      │                                    (P6.18 offline) │    (cloud/customer
      └────────────────────────────────────────────────────┘     private cloud)
```

Dependency direction (§6.2/§8.2, enforced by `tools/check_dependency_rule.py`):

- `flexcore-edge-agent` (composition) → `flexcore-contracts`,
  `flexcore-ports` (the hexagonal inside) + infrastructure (tokio, time,
  tracing, tracing-subscriber). Allowed: composition crates may use
  infrastructure; the eight core crates still may not.
- Future `crates/adapters/openems` → `flexcore-edge-agent` (for the
  `EmsAdapter` port + ACL types) + its own protocol dependencies. Allowed:
  adapters depend on ports.
- Nothing depends ON `flexcore-edge-agent` except future adapter crates and
  the binary itself. No core crate ever reaches the edge agent.
- Deliberately NO `flexcore-domain` dependency: the edge carries its
  site/tenant binding as configuration, and the asset-key →
  registry-`AssetId` mapping exists only at registration control-plane side
  (the P2.25 step-2 pattern, cited not reinvented).

### 3.3 How the edge differs from the control plane (the structural statement)

| Property | Control plane (`apps/control-plane`, `apps/worker`) | Edge agent (this crate) |
|---|---|---|
| Availability assumption | Store and broker present; startup fail-fast on connection failure | **Offline-tolerant**: the loop runs degraded when the uplink is down; buffering is the degraded spine |
| Connection direction | Serves inbound API; publishes outbound to broker | **Outbound-only**: dials the control plane and the EMS API; opens no listening socket (§3.3) |
| Control authority | Command lifecycle + simulation-only dispatch (P4.28 discipline) | **No command path at all this wave**; flag defaults FALSE and is inert; receiver is P6.14 behind a written test-env gate |
| Failure of a dependency | Abort/startup failure | Degrade, buffer, audit, retry — the loop never stops on a failed poll or forward |
| Sample vocabulary | `NormalizedTelemetry` (registry `AssetId`s) | `EdgeTelemetrySample` (edge-local asset KEYs; ID mapping happens control-plane side) |

### 3.4 Planned adapters, per wave (the module list P6.05 step 1 names)

| Wave | Port | Concrete adapter | Note |
|---|---|---|---|
| P6.06 | enrollment/cert lifecycle (new ports) | platform-issued identity | depends on this crate's config |
| P6.07 | `UplinkChannel` | outbound mTLS channel (rustls) | replaces `FailClosedUplink` |
| P6.08/P6.10 | `EmsSource` | read path over the P6.12 adapter; OpenEMS discovery/read | replaces `FailClosedEmsSource` |
| P6.09 | `LocalBuffer` | durable disk store-and-forward | replaces `InMemoryLocalBuffer` (volatile today — logged) |
| P6.13 | `EmsAdapter` impl | openems-rest read/telemetry + fault injection | registry table edit `Planned → Implemented` |
| P6.14–P6.16 | (none — new gated receiver) | command path disabled-by-default; allowlist enforcement config | flag semantics become real; this wave's absence is structural |
| P6.17/P6.18 | health/audit upgrades | local health; offline audit buffer | replaces `InMemoryAuditSink` |

## 4. The anti-corruption layer (P6.12) — the mapping rules

Everything EMS-native becomes canonical ONLY through `adapters::acl`
(§6.4). The rules, each enforced by construction and tested (UNRUN):

1. **Points are mapped or refused.** A reading for a point without a
   declared `PointMapping` is `AclError::PointUnmapped`; the ACL also
   carries a DECLARED-unmapped inventory (`declared_unmapped` +
   `unmapped_points()`), so a known-but-unmapped channel is listed, never
   silently dropped (P6.12 step 2: every unmapped field listed as unmapped).
2. **Units are named or refused.** `NativeUnit::parse` accepts exactly W,
   kW, MW, var, kvar, Mvar, Wh, kWh, MWh, `%`, `fraction`; anything else is
   `UnknownNativeUnit` — no invented unit, no default scale. Quantity/unit
   families are enforced (`UnitQuantityMismatch` otherwise); the table
   itself is validated at construction.
3. **Quality has no default.** The named table: `GOOD`→GOOD, `BAD`→BAD,
   `STALE`→STALE, `SUBSTITUTED`→ESTIMATED, `OUT_OF_RANGE`/`UNSYNCHRONIZED`/
   `UNRELIABLE`→SUSPECT, `COMMUNICATION_LOSS`→STALE (the §10.4 richer
   states via `flexcore_contracts::quality::map_source_state`); absent code
   → `QualityNotDeclared`; unknown code → `UnknownQualityCode`; native
   `MISSING` → refused (`NativeMissingRequiresGapMarker` — gap markers are
   platform-derived, P2.04 D-T6; the edge is a measured source). No path
   yields `GOOD` from a degraded state (P0.08 non-silencing); the original
   code travels as `source_quality_code` metadata. The adapter additionally
   declares its full quality mapping in `AdapterCapabilities`
   (`QualityMappingDeclaration`, the P2.11 provider-rule shape).
4. **Time is UTC or refused.** `NativeTimestamp` is Unix-millis or RFC 3339
   with the zero offset; non-UTC is `NonUtcNativeTimestamp` — never
   converted (P0.09; site timezone travels as metadata). `event_time` and
   `ingest_time` are the distinct contracts types; ingest ≥ event enforced
   by the sample constructor.
5. **Signs are declared, not inferred.** Active power: canonical is
   P > 0 = export (P0.07 §2); an `ImportPositive` native mapping is
   negated EXPLICITLY (the recorded conversion). Reactive power: the value
   passes through in MVAr WITH its declared
   `ReactiveSignConvention` (`QuantityValue::Reactive`) — never flipped,
   never interpreted from sign alone (P0.07 §3; `ReactiveConventionNotDeclared`
   otherwise).
6. **Canonical output only.** The ACL emits `EdgeTelemetrySample` in the
   frozen vocabulary (`TelemetryQuantity`/`QuantityValue` from
   `flexcore-ports`; unit newtypes from `flexcore-contracts`), so no
   downstream consumer needs a second mapping. `flexcore:`-namespaced
   source ids are refused (P2.04 D-T6).

Specification-version note (P6.12 step 4): rules 2–5 encode FlexCore's OWN
frozen contracts (P0.07/P0.08/P0.09; report §10.2–§10.4) — they are
FlexCore norms, not protocol claims. The OpenEMS-specific channel and
quality vocabulary is deliberately NOT encoded here; P6.10 encodes it
against the VERIFIED EdgeConfig API (currently UNVERIFIED, RESEARCH.md §1)
by EXTENDING the adapter's declared mapping, never by widening this table
implicitly. The protocol-side native-unit spellings accepted by
`NativeUnit::parse` are the ACL's own named vocabulary (a FlexCore-local
decision, recorded here) — they are not OpenEMS channel-type claims.

## 5. Adapter registry (P6.12) — the allowlist

| id | Status | Constructible? | Note |
|---|---|---|---|
| `openems-rest` | `Planned { implementing_wave: "P6.10/P6.13" }` | No — `NotYetImplemented`, naming the wave | the RECOMMENDED default under OPEN D5 (ENTRY_NOTE boundary 1); registered now so composition checks and profiles can reference it |
| `generic-rest` | `Reserved` | No — always refused | a recorded placeholder SLOT; recording it does not imply an API exists |

Enforcement points: `main.rs` fails fast on a non-allowlisted
`FLEXCORE_EDGE_ADAPTER_ID`; P6.16 will own the per-site allowlist config
(which adapters may receive commands, if the gated path ever exists); the
P6.20 deployment profile ships the allowlist. Extending the allowlist is a
reviewed change (new mapping table + P6.02 dependency-audit note).

## 6. Toolchain and workspace wiring (P6.05 step 4)

- Toolchain: **Rust 1.99.1 stable, exact-pinned** in `rust-toolchain.toml`
  since P1.02 — unchanged by this wave (the 1.100 bump policy in that
  file's header applies and was NOT triggered). The pinned version is the
  recorded Rust version for this crate.
- Root `Cargo.toml`: additive — one member entry (`crates/edge-agent`, with
  a comment naming this record) and one `[workspace.dependencies]` entry
  (`flexcore-edge-agent`). No other member was touched.
- `tools/check_dependency_rule.py`: additive — `flexcore-edge-agent` added
  to `COMPOSITION_CRATES` and `crates/edge-agent` to `MEMBER_DIRS`.
  Verification output quoted in the wave report and in HANDOVER.md.

## 7. Honest status and open items

| Item | Status |
|---|---|
| Crate code + tests | WRITTEN; **not built** (`cargo build`/`fmt`/`clippy`/`test` banned this wave — AGENTS.md rule 7 honoured by NOT claiming verification) |
| Stage 1–5 gates | NOT HELD (recorded exception, ENTRY_NOTE) — this crate compiles against Phase 1–5 APIs that exist in-tree, unbuilt |
| D5, D6 | OPEN (recorded, not decided); adapter work proceeds against the RECOMMENDED default only |
| OpenEMS REST API | UNVERIFIED (never executed); the adapter port is protocol-agnostic so the concrete adapter can follow the verified API |
| `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` on the new crate | the P6.05 acceptance check — for the human build pass |
