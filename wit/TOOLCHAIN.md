# `wit/TOOLCHAIN.md` — WIT/WASM toolchain pins (P7.40)

The pinned toolchain versions the `wit/` packages and the Phase 7 WASM
tooling are built and validated with. Full record — including where each pin
lands, who applies it, and the verbatim human verification steps — is
`ledger/phase-07-plugin-sdk/40_toolchain_pinning.md`. This file keeps the
same pins at hand for the tooling consumer (ledger P7.40 step 2).

**Status: all pins below are RECORDED-UNAPPLIED unless the "Where it lands"
row says otherwise. Nothing in this tree has ever been validated by any of
these tools** (see `README.md` §0).

## 1. Pin table

| Tool | Pinned version | Basis | Where it lands | Who applies |
|---|---|---|---|---|
| Rust toolchain | **1.99.1** (exact pin) | `rust-toolchain.toml` (P1.02; current workspace pin, read 2026-11-13 — NOT edited by P7.40) | `rust-toolchain.toml` (already applied) | Bump only deliberately per its recorded policy |
| Wasmtime | **50.0.1** (the 50.0.0 line; hard floor **≥ 50.0.0** — CVE fixes below) | RESEARCH.md §1 (checked 2026-11-07): 50.0.1 released 29 Oct 2026, carries all three October 2026 fixes | Root `Cargo.toml` `[workspace.dependencies]` at first host build | P7.10 wave owner (root `Cargo.toml` is not touched by P7.40) |
| `wasm-tools` | **1.239.0 — ASSUMPTION** | RESEARCH.md §2: 1.239.0 was the surveyed current binary on 26 Aug 2026; **no later version is claimed — re-check the release page at implementation time (RESEARCH.md §7 task 2)** | P7.27 CI job env (config-only) | P7.27 wave owner, or the human running the prepared validation |
| `wit-bindgen` | **UNPINNED-REVERIFY** | No version verified in RESEARCH.md — never pin an unverified number | P7.27 CI job env; SDK docs at first build | P7.27 wave owner after a release-page check |
| `cargo-component` | **UNPINNED-REVERIFY** | Same — no version verified | P7.27 CI job env; example-component build docs | P7.27 wave owner after a release-page check |
| `wasip2` crate | **2.x** (REVERIFY) | RESEARCH.md §7 task 4 — confirm against `bytecodealliance/wasi` at implementation time | Guest-side bindings for example components | Example-component owner (P7.24) |
| Buf CLI (Protobuf) | Config-only job owned by P7.27 — no pin recorded by P7.40 | Ledger P7.40 is WIT/WASM tooling; Buf is the P7.27/P7.17 seam | P7.27 CI config | P7.27 wave owner |

## 2. The Wasmtime CVE floor (R7-C2)

Any pin below the October 2026 fixes is a known sandbox-escape exposure, not
a stylistic choice (RESEARCH.md §1, §6 R7-C2):

| Advisory | Class | Fixed in |
|---|---|---|
| GHSA-897f-g5gm-fmxm / CVE-2026-115889 (bit-packing sandbox escape, CVSS 9.9) | Cranelift | 36.0.19; main line from 43.0.0 |
| GHSA-mghf-wjfp-p5xm / CVE-2026-115893 (aarch64 branch-table sandbox escape, CVSS 9.9) | Cranelift | 36.0.19 / 48.0.7 / 49.0.4 / 50.0.0 |
| CVE-2026-115384 (async-callback stack overflow, CVSS 9.9) | Wasmtime | 48.0.4 / 49.0.2 |

Re-check the GHSA advisory pages at implementation time and record the
advisory IDs and check date (RESEARCH.md §7 task 1; ledger P7.13 step 4).

## 3. Rust-pin conflict record (resolve at first build)

Wasmtime 50.0.0 requires Rust **1.97.0** to compile. The workspace pin
(1.99.1, `rust-toolchain.toml`) is numerically ABOVE that minimum, so no
bump is required today — recorded honestly as "constraint currently
satisfied". The recorded conflict is POLICY, not number: `rust-toolchain.toml`
pins exactly and bumps only deliberately (release notes + full-workspace
`cargo +beta test` pass), while Wasmtime minor releases may raise the
minimum above the pin. First-build check (P7.10): confirm
`cargo check` of the host crate passes under 1.99.1 with Wasmtime 50.0.1;
if a future Wasmtime demands a newer Rust, the bump follows the
`rust-toolchain.toml` policy — this file records the requirement; it does
NOT edit `rust-toolchain.toml`.

## 4. Human verification steps — PREPARED-UNRUN (verbatim)

These commands are prepared, never run in this loop. Run them from the
repository root with the pinned `wasm-tools` (1.239.0 ASSUMPTION — verify
the exact flags against `wasm-tools --help` for the version actually
installed before treating a failure as a file defect).

1. **WIT package validation** (parse and resolve every package and world):

   ```sh
   wasm-tools component wit wit/
   ```

   Expected: each package resolves and the three worlds print. A parse or
   resolution failure is checked against the checklist in `README.md` §5
   (fix the construct, not the contract).

2. **Example component build** (after `plugins/wasm-examples/` is
   materialised per its README — the example is a DESIGN note only today;
   `cargo-component` and the `wasip2` crate version are UNPINNED-REVERIFY /
   2.x-REVERIFY):

   ```sh
   cargo component build --release
   ```

   Expected: a component targeting `flexcore:optimizer/optimizer-plugin@1.0.0`
   is produced and imports/exports exactly what the world declares.

Record the tool version output (`wasm-tools --version`, `cargo component
--version`) next to the results — an unrecorded toolchain change is exactly
what ledger P7.40 forbids (the CI pin check that enforces this is P7.27's,
config-only).
