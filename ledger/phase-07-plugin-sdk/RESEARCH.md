# Phase 7 — Plugin SDK and Modular Composition: Research

| Field | Value |
|---|---|
| Phase | 7 — Stage 7 Plugin SDK and modular composition |
| Checked | 7 November 2026 |
| Method | `browser.search` + `browser.open` on primary sources only (official docs, repos, standards bodies). Secondary sources are marked as such. |
| Rule | Do not invent versions. Anything not verified here is labelled **ASSUMPTION** or listed as an open verification task (§7). |

## 1. Wasmtime

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Latest release | **Wasmtime 50.0.1, released 29 October 2026** (50.0.0 released 20 October 2026; 49.0.5 also 29 October 2026; 48.0.8 also 29 October 2026). Wasmtime 50.0.0 requires Rust 1.97.0 to compile. | Bytecode Alliance Wasmtime releases — https://github.com/bytecodealliance/wasmtime/releases |
| WASI 0.3 runtime support | **WASI 0.3 runtime support is available in Wasmtime 43 and later**, implementing the WASI 0.3 ABI (`async func`, `stream<T>`, `future<T>`). Both `wasmtime run` and `wasmtime serve` can run 0.3 components when the 0.3 ABI is enabled; `wasmtime serve` falls back to the WASI 0.2 `wasi:http/proxy` world automatically for components that do not export a 0.3 `service` world. Support for serving 0.3 `wasi:http/service` and `wasi:http/middleware` worlds was still in development at the time of the docs update. | Bytecode Alliance Component Model docs — https://component-model.bytecodealliance.org/ (Wasmtime page, refreshed for the WASI P3 release) |
| Security advisories (October 2026) | **Two critical Cranelift sandbox-escape flaws, both CVSS 9.9, disclosed 19 October 2026** (GHSA-897f-g5gm-fmxm / CVE-2026-115889, bit-packing; GHSA-mghf-wjfp-p5xm / CVE-2026-115893, aarch64 branch-table; RUSTSEC-2026-0380 / RUSTSEC-2026-0379). A crafted module could escape the sandbox. Fixed in **36.0.19, 48.0.7, 49.0.4, and 50.0.0** (50.0.0 carries both fixes). **CORRECTED 7 November 2026 (iteration-09 review):** CVE-2026-115889 was fixed on the main line already in **43.0.0 (March 2026)**, so releases 43.0.0–47.x are affected ONLY by CVE-2026-115893, not by 115889. A third flaw, GHSA-m9f7-9w4p-gv38 (symlink rename, macOS/FreeBSD, no CVE assigned), was fixed in 36.0.18 / 48.0.6 / 49.0.3. The 24.x LTS line reached end of support 20 August 2026 and gets no fix. A separate async-callback stack-overflow issue (CVE-2026-115384, CVSS 9.9) was fixed in 48.0.4 and 49.0.2. | Wasmtime releases (above); Bytecode Alliance security advisories as reported 19–26 October 2026 (secondary reporting of the primary advisories — re-check the GHSA pages at implementation time). |

**Implication:** pin Wasmtime **≥50.0.0** for the Phase 7 host (P7.03,
P7.10, P7.13). Any pin below 48.0.7/49.0.4 is a known sandbox-escape
exposure, not a stylistic choice.

## 2. WIT / Component Model / WASI status

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| WASI 0.3 release | **WASI 0.3 (Preview 3) released 11 June 2026.** It rebases WASI's interfaces onto three new Canonical ABI primitives (`async func`, `stream<T>`, `future<T>`); the `wasi:io` package is removed and its functionality is provided by the Component Model directly. WASI 0.3 runtimes can polyfill 0.2 by mapping 0.2 imports onto native 0.3 primitives at the host boundary. | Bytecode Alliance Component Model FAQ — https://component-model.bytecodealliance.org/reference/faq.html |
| WASI 0.2 status | **WASI 0.2 (Preview 2) is the production/default target** for new tooling (Wasmtime, `cargo component`, `jco`); WASI 0.3 ecosystem support is uneven (Rust furthest along; jco 0.3 shim experimental; C/Kotlin toolchains had not shipped it as of the October 2026 survey). | Same FAQ; October 2026 WASI survey (secondary) — https://blog.redlinesoft.net/posts/wasi-preview-2-wasm-outside-browser/ |
| WASI 0.3 vs 0.2 worlds | `wasi:cli/command` and `wasi:http/service` become the standard worlds in 0.3 (`wasi:http/service` supersedes `wasi:http/proxy`); `wasi:io`/`wasi:http` gain async/futures-style APIs (`@since(version = 0.3.0)`). | Same sources |
| Component Model tooling | `wasm-tools` 1.239.0 was the surveyed current binary (26 August 2026). **ASSUMPTION:** no later `wasm-tools` version is claimed here — re-check at implementation time (P7.09). | Secondary survey (August 2026) — https://sparkles.petar-kirov.dev/research/autological-artifacts/wasm-component-model |

## 3. Rust 1.100 / `wasm32-wasip3`

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Current stable Rust | **Rust 1.99.1, point release 15 October 2026** (1.99.0 released 1 October 2026; 1.99.1 fixes a missing `wcslen` symbol, an FFI bool-return miscompilation on x86_64, and a rust-lld issue on macOS), on the 2024 edition. **CORRECTED 7 November 2026 (final review):** this row previously said 1.99.0 was current — stale; 1.99.1 superseded it on 15 October 2026, three weeks before the cut-off. | Rust blog, Announcing Rust 1.99.1, 15 Oct 2026, https://blog.rust-lang.org/ ; Rust Forge release table, https://forge.rust-lang.org/ (Beta 1.100, stable 12 Nov 2026) |
| Rust 1.100 | **Scheduled for 12 November 2026 — not yet released as of the 7 November 2026 cut-off.** The `wasm32-wasip3` target is Tier 2 and is **first available on stable in Rust 1.100.0**; before that it requires nightly (`rustup target add wasm32-wasip3 --toolchain nightly`). The target requires LLVM 23 and `wasi-sdk-34`-or-later for C builds. Cooperative multithreading was not yet supported on the target as of 3 September 2026 (`std::thread` returns an error); support was expected before end of 2026. Running 0.3 components requires Wasmtime 46 or later. | Bytecode Alliance Rust component docs — https://component-model.bytecodealliance.org/language-support/creating-runnable-components/rust.html ; rustc book, `wasm32-wasip3` — https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip3.html |
| Report's statement | Report §5.4 [R3] states the same: the Rust `wasm32-wasip3` target requires nightly before the planned Rust 1.100 release on 12 November 2026, and WASI 0.3 should not be a release blocker at the 7 November 2026 cut-off. | Report §5.4 |

**Implication:** the report's §5.4 position is confirmed by the primary
sources. Phase 7 builds on **WASI 0.2 / `wasm32-wasip2`** (Tier 2, stable)
and tracks WASI 0.3 for re-evaluation after 1.100 ships and host
compatibility is confirmed (P7.32).

## 4. Wasmtime security model

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Sandboxing caveat | Wasmtime provides Component Model embedding APIs and sandboxing mechanisms, but sandboxing must still be combined with host capability policy, resource limits, signatures, and isolation. | Report §5.4 [R4]; Bytecode Alliance Wasmtime security docs — https://docs.wasmtime.dev/security.html |

**Implication:** P7.11 (capability broker) and P7.13 (sandbox negative
tests) are not optional hardening — they are part of what "sandboxed" means
in this design. The October 2026 CVEs (§1) are the concrete evidence for why.

## 5. Rust ABI (why dylib loading stays rejected)

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| ABI stability | The Rust ABI has no stability guarantee. | Report §5.4 [R5]; Rust Language Reference, External blocks and ABI — https://doc.rust-lang.org/reference/items/external-blocks.html |

**Implication:** ADR-006 stands. No new evidence in this check weakens it.

## 6. Contradictions and traps found (recorded honestly)

| ID | Finding |
|---|---|
| **R7-C1 — WASI 0.3 is real, but not yet usable on stable Rust** | WASI 0.3 was released 11 June 2026 and Wasmtime 43+ runs it — so a reader could conclude Phase 7 should target 0.3. The blocker is the Rust toolchain: `wasm32-wasip3` is nightly-only until Rust 1.100 ships on 12 November 2026, five days after this ledger's cut-off, and cooperative multithreading on the target was still incomplete in September 2026. The report's §5.4 decision (WASI 0.2 baseline; 0.3 not a blocker) is therefore correct and confirmed, not merely cautious. |
| **R7-C2 — Wasmtime versions before October 2026 carry known sandbox escapes** | Any Phase 7 plan that pins "a recent Wasmtime" without a floor is unsafe: CVE-2026-115889 / CVE-2026-115893 (CVSS 9.9, sandbox escape) affect releases before 36.0.19 / 37.0.0–42.x for both flaws; 43.0.0–47.x are affected ONLY by CVE-2026-115893 (115889 was fixed on the main line in 43.0.0), as are 48.0.0–48.0.6 / 49.0.0–49.0.3 for the branch-table flaw. The floor is ≥50.0.0 (or 36.0.19 / 48.0.7 / 49.0.4 on those lines). The report predates these CVEs (cut-off 7 November 2026, advisories 19 October 2026) and does not name a Wasmtime version; this ledger's floor is new information from this check, not a report correction. |
| **R7-C3 — No contradiction found in §5.5's manifest example** | The §5.5 manifest is explicitly illustrative ("not a final schema"), so the fact that it omits fuel/time, concurrency, output-size, and call-rate limits (which §5.4 Class B requires) is not a contradiction — P7.05 is instructed to record the differences when the real schema is written. |
| **R7-C4 — DeepSeek Harness maturity** | Report §5.1 [R2] notes the Harness project is in developer preview with breaking-change warnings. Nothing in this check changes that; the SDK adopts the ideas, not the implementation. No Harness version is pinned or relied on. |

## 7. Open verification tasks (not facts)

1. Re-check the GHSA pages for CVE-2026-115889 / CVE-2026-115893 /
   CVE-2026-115384 at implementation time and confirm the pinned Wasmtime
   version carries all three fixes.
2. Re-check `wasm-tools` / `cargo-component` / `wit-bindgen` versions at
   implementation time (only `wasm-tools` 1.239.0 as of 26 August 2026 was
   found in this check).
3. Confirm Rust 1.100.0 actually shipped on 12 November 2026 (scheduled, not
   yet released at this cut-off) before any WASI 0.3 re-evaluation.
4. Confirm the `wasip2`/`wasip3` crate versions (`wasip2` 2.x, `wasip3` 0.x
   per secondary reporting) against the `bytecodealliance/wasi` repository
   at implementation time.
5. Confirm the current `wit-bindgen` WASI 0.3 tracking issue status
   (in-development `wasi:http/service` support in `wasmtime serve`) before
   relying on 0.3 HTTP worlds.
