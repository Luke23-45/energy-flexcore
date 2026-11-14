# P7.40 — Toolchain version pinning for WIT/WASM tooling

Date: 2026-11-13. Subtask: P7.40 (`ledger_phase_7.md` §P7.40).
Owner role: DevOps. Report trace: §9.1; §22.2; §10.5. Inputs: P7.09
(`09_wit_worlds.md`, the `wit/` tree); P7.27 (CI — seam only, not
implemented here); RESEARCH.md §1–§2, §6–§7 (R7-C2 CVE floor; §7 task 1–5
open verification tasks).

## 1. What this record is (and is not)

It records every WIT/WASM toolchain version the SDK is built and validated
with, where each pin lands, and who applies it — so a WIT world that
validates under one tool version and fails under another is reproducible,
not mysterious (ledger objective). It does NOT apply any pin: nothing was
installed, nothing was built, no `Cargo.toml`, CI file, or
`rust-toolchain.toml` was touched (ownership boundaries; P7.40 is a
RECORD). Pin application happens at the named owner's first build / CI
wiring. The mirror table for the tooling consumer lives in `wit/TOOLCHAIN.md`.

## 2. Pin table (ledger step 1)

| Tool | Pin | Basis / honesty status | Where it lands | Who applies |
|---|---|---|---|---|
| Rust toolchain | **1.99.1** (exact) | `rust-toolchain.toml` (P1.02) — read 2026-11-13, NOT edited | `rust-toolchain.toml` (already applied) | Bump only deliberately, per its recorded policy |
| Wasmtime | **50.0.1** (= 50.0.0 line; hard floor **≥ 50.0.0**) | RESEARCH.md §1 (checked 2026-11-07); carries all three October 2026 fixes (§3 below) | Root `Cargo.toml` `[workspace.dependencies]` — at first host build | P7.10 wave owner (root `Cargo.toml` not touched by P7.40) |
| `wasm-tools` | **1.239.0 — ASSUMPTION** | RESEARCH.md §2: surveyed current binary 26 Aug 2026; no later version claimed — **re-check the release page at implementation time** (RESEARCH.md §7 task 2) | P7.27 CI job env (config-only); human validation run | P7.27 wave owner, or the human running the prepared validation |
| `wit-bindgen` | **UNPINNED-REVERIFY** | No version verified in RESEARCH.md — never pin an unverified number (AGENTS.md rule 1) | P7.27 CI job env; SDK docs at first build | P7.27 wave owner after a release-page check |
| `cargo-component` | **UNPINNED-REVERIFY** | Same — no version verified | P7.27 CI job env; example-component build docs | P7.27 wave owner after a release-page check |
| `wasip2` crate | **2.x — REVERIFY** | RESEARCH.md §7 task 4 (secondary reporting; confirm against `bytecodealliance/wasi`) | Guest-side bindings for example components | Example-component owner (P7.24) |
| Buf CLI | Not pinned by P7.40 | Protobuf tooling is the P7.27/P7.17 seam | P7.27 CI config (config-only) | P7.27 wave owner |

Ledger step 1 also asks for the WASI SDK: none is required by any current
deliverable (the `wit/` tree is pure WIT; example components are Rust
`cargo-component` builds using the `wasip2` crate, not the C
`wasi-sdk`), so no WASI SDK pin is recorded — recorded as "not required
today" rather than a placeholder pin.

## 3. The Wasmtime CVE floor (R7-C2 — the three CVE rows)

Any pin below the October 2026 fixes is a known sandbox-escape exposure,
not a stylistic choice (RESEARCH.md §1, §6):

| Advisory | Class | Fixed in |
|---|---|---|
| GHSA-897f-g5gm-fmxm / CVE-2026-115889 (bit-packing sandbox escape, CVSS 9.9) | Cranelift | 36.0.19; main line from 43.0.0 |
| GHSA-mghf-wjfp-p5xm / CVE-2026-115893 (aarch64 branch-table sandbox escape, CVSS 9.9) | Cranelift | 36.0.19 / 48.0.7 / 49.0.4 / 50.0.0 |
| CVE-2026-115384 (async-callback stack overflow, CVSS 9.9) | Wasmtime | 48.0.4 / 49.0.2 |

Pinning ≥ 50.0.0 carries all three. Re-check the GHSA advisory pages at
implementation time and record the advisory IDs and check date (RESEARCH.md
§7 task 1; ledger P7.13 step 4 — P7.13's owner).

## 4. The Rust-pin conflict record (recorded, not resolved here)

Wasmtime 50.0.0 requires Rust **1.97.0** to compile. The workspace pin
(1.99.1) is numerically above that minimum, so no bump is required today.
The recorded conflict is one of POLICY: `rust-toolchain.toml` pins exactly
and bumps only deliberately (release-notes read + full-workspace
`cargo +beta test` pass), while Wasmtime releases may raise the minimum
above the exact pin at any release. Resolution at first build (P7.10
owner): run `cargo check` of the host crate under 1.99.1 with Wasmtime
50.0.1 and record the outcome; if a future Wasmtime demands a newer Rust,
follow the `rust-toolchain.toml` bump policy with a recorded pin change.
**No `rust-toolchain.toml` edit was made by P7.40** (none was needed; even
an additive comment would violate the ownership boundary).

## 5. Human verification steps — PREPARED-UNRUN (ledger step 2, verbatim)

Never run in this loop; prepared for the human gate. Record tool versions
next to every result (`wasm-tools --version`, `cargo component --version`)
— an unrecorded toolchain change is exactly what this subtask forbids.

1. **WIT package validation** (from the repository root, with the pinned
   `wasm-tools`; verify exact flags against `wasm-tools --help` for the
   installed version before treating a failure as a file defect):

   ```sh
   wasm-tools component wit wit/
   ```

   Expected: all four packages resolve and the three worlds
   (`optimizer-plugin`, `grid-model-provider-plugin`,
   `telemetry-source-plugin`) print. Failures are triaged against
   `wit/README.md` §5 (fix the construct, never the contract).

2. **Example component build** (after `plugins/wasm-examples/` is
   materialised per its README — a DESIGN note only today):

   ```sh
   cargo component build --release
   ```

   Expected: a component targeting
   `flexcore:optimizer/optimizer-plugin@1.0.0` whose WIT surface shows the
   export and zero WASI imports.

## 6. SDK-docs and exit-gate recording (ledger step 2)

The pinned versions are recorded in `wit/TOOLCHAIN.md` (the tooling
consumer's copy) and in this file; the P7.28 exit-gate run book (its
ledger step 1: "exact versions (FlexCore, Wasmtime, WIT worlds, worker
template, plugin versions)") must copy the applied pins from this table at
run-book time — P7.28's owner, not P7.40's.

## 7. CI pin check (ledger step 3 — seam, not implemented here)

The CI check that fails if a toolchain version changes without this pin
record being updated is **P7.27's config-only job** (its step 1 gates).
Seam record for P7.27: implement the check as "CI installs exactly the
versions in `wit/TOOLCHAIN.md` §1; a tool upgrade requires a same-PR edit
of `wit/TOOLCHAIN.md` §1 + `40_toolchain_pinning.md` §2 (the pin-change
record), otherwise the job fails on a version diff". Until P7.27 lands,
the pin discipline is manual: any tool version used to validate `wit/` is
recorded next to the validation output (`wit/TOOLCHAIN.md` §4).

## 8. Acceptance check (ledger P7.40 — honest)

- "Every WIT/WASM tool has an exact pinned version with a recorded
  release-page check": PARTIAL BY DESIGN — Wasmtime and the Rust pin are
  exact with dated sources; `wasm-tools` is an ASSUMPTION (1.239.0) and
  `wit-bindgen`/`cargo-component`/`wasip2` are UNPINNED-REVERIFY, because
  RESEARCH.md verified no version for them and inventing numbers is
  forbidden (AGENTS.md rule 1). The re-check tasks are named (RESEARCH.md
  §7 task 2/4) and owned.
- "CI fails on an unrecorded toolchain change": NOT MET — the check is
  P7.27's; the seam is specified in §7. The ledger's verification method
  (clean-machine rebuild from the pins) requires applied pins and is
  therefore also OPEN until the first build.

## 9. Related records

- `09_wit_worlds.md` — the WIT packages the pins exist to validate.
- `wit/TOOLCHAIN.md` — the consumer-side mirror of §2/§5.
- `wit/README.md` — the first-validation checklist the human run uses.
- `plugins/wasm-examples/README.md` — the example component the build
  command targets.
