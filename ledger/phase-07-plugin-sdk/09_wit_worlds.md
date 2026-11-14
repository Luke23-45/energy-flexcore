# P7.09 — WIT interface packages: versioned worlds for each contract

Date: 2026-11-13. Subtask: P7.09 (`ledger_phase_7.md` §P7.09).
Owner role: Rust engineer (SDK) + architect. Report trace: §5.4 Class B;
§8.1 (`wit/`); §9.1; §10.5; §22.2. Inputs: P7.02 (`02_scope_freeze.md`);
P7.04 (`04_seam_inventory.md`); RESEARCH.md (WASI 0.2 baseline; R7-C1);
the real Rust trait definitions in `crates/flexcore-optimization-api/`,
`crates/flexcore-grid-api/`, `crates/flexcore-ports/`, and
`crates/flexcore-contracts/` (grep- and read-verified 2026-11-13).

## 1. What this record is (and is not)

This record delivers ledger P7.09 step 1–4: the WIT packages under `wit/`,
their mapping tables, the compatibility policy, and the WASI-baseline
record. It does NOT claim tool validation: **the WIT files are
WRITTEN-UNVALIDATED — `wasm-tools` has never been run in this loop** (no
install, no build, no execution permitted or attempted). The first
`wasm-tools` validation is a human gate input, prepared verbatim in
`wit/TOOLCHAIN.md` §4 and `40_toolchain_pinning.md` §4. The ledger's
verification method ("`wit-bindgen`/component tooling validates the
packages") is therefore OPEN until that run is recorded or until P7.27
wires it into CI and a green run exists (§6 below).

No contract is invented: every WIT function maps to a real Rust trait
method and every WIT type to a named Rust type, recorded in the mapping
tables (`wit/README.md` §3). Types were checked against the actual trait
and struct definitions, not against the report's §9.1 sketches — the two
known §9.1-vs-Rust differences (`load_model` has no Rust counterpart;
`supported_problem_classes()` maps to `capabilities()`) are carried into
the world designs, not reconciled by inventing methods.

## 2. Package inventory (ledger step 1)

| Package | File | World | Contract mirrored (verified 2026-11-13) |
|---|---|---|---|
| `flexcore:core@1.0.0` | `wit/deps/flexcore-core@1.0.0/types.wit` | — (shared data vocabulary) | `flexcore-contracts/src/{ids,quality,units,time_model}.rs`; `telemetry_source.rs` quantity/value/record types |
| `flexcore:optimizer@1.0.0` | `wit/flexcore-optimizer@1.0.0/optimizer.wit` | `optimizer-plugin` | `flexcore-optimization-api/src/port.rs::Optimizer` (+ `problem.rs`, `config.rs`, `result.rs`) |
| `flexcore:grid@1.0.0` | `wit/flexcore-grid@1.0.0/grid.wit` | `grid-model-provider-plugin` | `flexcore-grid-api/src/port.rs::GridModelProvider` (+ `scope.rs`) |
| `flexcore:telemetry@1.0.0` | `wit/flexcore-telemetry@1.0.0/telemetry.wit` | `telemetry-source-plugin` | `flexcore-ports/src/telemetry_source.rs::TelemetrySource` (+ `backend_error.rs`) |

Directory names use a hyphen rendering because `:` is illegal on NTFS
(Windows development host); the `package` declaration inside each `.wit`
file is authoritative (`wit/README.md` §1 note).

Scope freeze alignment (`02_scope_freeze.md` §2): the three IN contracts
with real, built Rust traits get worlds; `AssetAdapter` / `ForecastProvider`
/ `MarketAdapter` stay world-less (no Rust contract — the fiction risk);
the `EmsAdapter` world is deliberately deferred to a follow-up wave (the
trait is "written, UNBUILT" per `04_seam_inventory.md` §3.10) and recorded
as such in `wit/README.md` §4. Protected-kernel names
(`CommandAuthorizer`, `DispatchGateway`) never become WIT worlds (§2 NEVER
rows).

WASI usage (ledger step 1's guard): no world imports any `wasi:` interface
— the report's §5.4 no-ambient-access rule makes `wasi:cli/command`-style
worlds unnecessary for these contracts, and unused imports are exactly the
attack surface P7.11/P7.13 exist to close. Every file header states this
and names the `wasi:*@0.2.x` pin convention for any future import.

## 3. Data vocabulary (ledger step 2)

`flexcore:core@1.0.0` carries, with exact-value enums:

- **Quality** — `telemetry-quality`: exactly `good / suspect / bad /
  missing / estimated / stale` (report §10.4; P0.08; the six, no seventh).
- **Units** — `canonical-unit`: `active-power-mw / reactive-power-mvar /
  apparent-power-mva / energy-mwh / per-unit / soc-fraction / dimensionless`
  (§10.2; `VariableUnit::canonical_unit()` strings). No bare float without
  a unit field or a unit-bearing field name anywhere in the tree; the
  reactive sign is carried only together with its declared
  `reactive-sign-convention` (`injection-positive` / `consumption-positive`
  — no default, §10.2).
- **Time** — `utc-timestamp` (Unix-epoch millis, UTC-only) with distinct
  `event-time` / `ingest-time` fields on the record (§10.3; P0.09).
- **Identifiers** — `flex-uuid` (16 bytes as two u64 halves;
  `flexcore-contracts::ids::FlexUuid`).
- **Quantities/values** — `telemetry-quantity` (the frozen P2.04 §4
  vocabulary) and `quantity-value` (unit travels with the value, P0.07).
- **§9.2 provenance** — `reproducibility-metadata` (seed/config/artifact
  ref, per PLAN.md §4's §9.2 row); the contract-specific provenance fields
  (plugin/model/solver version, input snapshot ID, horizon, problem class,
  termination status, diagnostics) live on the result records of the
  worlds that require them — `optimization-result` and `power-flow-result`
  carry the full §9.2/§12.6/§12.5 provenance, completeness, and
  diagnostics contracts translated from the Rust validators.

## 4. Compatibility policy (ledger step 3)

Recorded in `wit/README.md` §2: breaking WIT change ⇒ new major package
version (new version directory, released versions never edited); additive
changes bump minor and are RECORDED in `wit/README.md` §6 (changelog); the
manifest's `flexcoreApi` SemVer range (P7.05) resolves against world
versions; `wit-bindgen`/`wasm-tools` compatibility validation runs in CI
as a **P7.27 config-only job** — a breaking change without a version bump
must fail CI (P7.27 acceptance), which is the enforcement of this policy.

## 5. WASI baseline record (ledger step 4)

**WASI 0.2 / Component Model baseline for v0.5.0** (report §5.4;
`02_scope_freeze.md` P-3). WASI 0.3 is tracked, not required (RESEARCH.md
R7-C1: released 11 June 2026, but `wasm32-wasip3` is stable only from Rust
1.100, scheduled 12 November 2026 — after the 7 November 2026 cut-off —
and cooperative multithreading on the target was still incomplete).
**Re-evaluation trigger:** Rust 1.100 stable confirmed shipped AND host
compatibility confirmed — owned by P7.32. No WASI 0.3 syntax exists in the
tree; the telemetry world explicitly avoids `stream<T>`/`future<T>` (WASI
0.3 primitives) and mirrors the sync Rust port shape.

## 6. Verification status and seams (honest)

| Ledger acceptance criterion | Status |
|---|---|
| Every in-scope contract has a WIT world | Met for the three built IN contracts; `EmsAdapter` recorded as a deliberate deferral with an owner (`wit/README.md` §4); deferred contracts stay world-less by scope freeze |
| WIT types carry units, quality, §9.2 provenance (no bare floats) | Met by design — `wit/README.md` §3 mapping tables; reviewer field-by-field check against §9.2 is the ledger's second half of the verification method and is OPEN for the human |
| WASI 0.2 baseline recorded with re-evaluation trigger | Met (§5 above) |
| Verification method: component tooling validates the packages | **OPEN — WRITTEN-UNVALIDATED.** Prepared command in `wit/TOOLCHAIN.md` §4; human gate input. P7.27 owns the CI wiring as a config-only job (its step 1 names "WIT compatibility validation"); that seam is recorded here so P7.27 finds it |

Design-for-P7.10 notes (not implemented here — P7.10 owns the host): the
worlds export exactly one interface each and import nothing, so the host's
"bind only what the world declares" rule is trivially checkable; the
telemetry world's tenant field is host-set (P7.11 broker / P7.39
data-level filtering enforce scope); result records are shaped so the host
can re-run the Rust completeness validators on component results.

First-validation checklist for the human run: `wit/README.md` §5 (eight
flagged constructs, including the exact `use`-path version form and `///`
doc comments).

## 7. Related records

- `40_toolchain_pinning.md` — the pins and the verbatim PREPARED-UNRUN
  verification commands.
- `wit/README.md` / `wit/TOOLCHAIN.md` — package-level records.
- `04_seam_inventory.md` — the seam map the worlds mirror (§3.1, §3.2,
  §3.4; the protected non-seams of §6.1 are absent from `wit/` by design).
- `plugins/wasm-examples/README.md` — the P7.09 design note for the example
  component the P7.10 host will run.
