# P7.03 — License and dependency audit for the SDK stack (D4 follow-through)

Date: 2026-11-13. Subtask: P7.03 (`ledger_phase_7.md` §P7.03).
Owner role: Legal + engineering. Report trace: §25.2 (D4); §9.3; §22.2.
Inputs: `RESEARCH.md` (§1–§2, §7); `.agent/DECISIONS.md` (D4 OPEN);
`deny.toml` (P6.27); `02_scope_freeze.md` (the scope that determines
which dependencies are actually introduced).

## 1. Status of this audit — STATIC, nothing installed or run

This is a **static audit**: no crate was installed, no lockfile was
created, no `cargo deny` run happened (loop bans; ENTRY_NOTE.md). The
workspace has **no committed `Cargo.lock`** (`deny.toml` header), so no
transitive-set verdict can exist yet. Every license below is recorded
from `RESEARCH.md` or from crates.io/repository metadata and is labelled
**UNVERIFIED** until the first human `cargo deny check licenses` run,
which must (per the P6.02 verification method, carried into this audit)
diff the lockfile-derived dependency set against §3's table; any
unlisted dependency fails that check. Zero-unknown-license claims are
never made from this record.

D4 outcome: **D4 remains OPEN** (`.agent/DECISIONS.md`). This record
narrows what the human verification must cover; it does not close D4.
No decision is recorded here — recording, not deciding.

## 2. Scope of the audit (what the frozen scope actually introduces)

From `02_scope_freeze.md`: v0.5.0 introduces (a) the Wasmtime component
host (Class B, `apps/plugin-host/`), (b) the WIT toolchain (P7.09), and
(c) the Python gRPC worker template (Class C, P7.17). The gRPC stack for
the *control plane's* existing REST/NATS surface is unchanged. Existing
carried items (rustls/rustls-pemfile RE-VERIFY, OpenEMS license
UNVERIFIED) remain owned by the P6.02/P6.27 records — listed in §5 for
completeness, not re-audited here.

## 3. SDK-stack dependency audit table (STATIC)

"Pin" column: RESEARCH.md pin, or **UNPINNED-REVERIFY** where no primary
source recorded a version (RESEARCH.md §7 tasks 2 and 4 name exactly
these re-checks). "License" column: as supported by RESEARCH.md or
crates.io/repository metadata at the 7 November 2026 cut-off — each such
entry is UNVERIFIED until the human run.

| Dependency | Pinned version | License (recorded) | Source of the license claim | D4 action |
|---|---|---|---|---|
| `wasmtime` | **50.0.1** (RESEARCH.md §1; released 29 Oct 2026; requires Rust 1.97 to compile — never installed/built in this loop) | Apache-2.0 WITH LLVM-exception | Bytecode Alliance / crates.io metadata, via RESEARCH.md §1 | **RE-VERIFY** at the first lockfile-backed `cargo deny` run |
| `wasmtime-wasi` (WASI 0.2 host support the P7.10 host needs) | follows `wasmtime` 50.0.1 — **UNPINNED-REVERIFY** | Apache-2.0 WITH LLVM-exception (expected — same Bytecode Alliance repo) | crates.io metadata — UNVERIFIED | **RE-VERIFY** version + license at P7.10 implementation |
| `wit-bindgen` (WIT code generation, P7.09) | **UNPINNED-REVERIFY** (RESEARCH.md §7 task 2: re-check at implementation) | MIT OR Apache-2.0 (expected — crates.io metadata) | crates.io metadata — UNVERIFIED | **RE-VERIFY**; both SPDX families are already allow-listed in `deny.toml` |
| `wasm-tools` (component tooling, P7.09 validation) | 1.239.0 — **ASSUMPTION** (single secondary survey, 26 Aug 2026; RESEARCH.md §2) | Apache-2.0 WITH LLVM-exception OR MIT (expected — repository metadata) | Repository metadata — UNVERIFIED | **RE-VERIFY** version and license at P7.09; tool, may not become a cargo dependency at all |
| `cargo-component` (component authoring tool) | **UNPINNED-REVERIFY** | Apache-2.0 WITH LLVM-exception OR MIT (expected — repository metadata) | Repository metadata — UNVERIFIED | **RE-VERIFY**; developer tool — if it never enters the dependency tree it stays out of the SBOM |
| `wasip2` (WASI 0.2 bindings for components) | **UNPINNED-REVERIFY** (wasip2 2.x per secondary reporting — RESEARCH.md §7 task 4) | Apache-2.0 WITH LLVM-exception OR MIT (expected) | crates.io metadata — UNVERIFIED | **RE-VERIFY** version against the `bytecodealliance/wasi` repo at P7.09/P7.10 |
| `tonic` (gRPC server/client for the worker template's control-plane side, P7.17–P7.18) | **UNPINNED-REVERIFY** (report §7.3 names Tonic/Prost; RESEARCH.md records no version) | MIT (expected — crates.io metadata) | crates.io metadata — UNVERIFIED | **RE-VERIFY** at P7.17 |
| `prost` (Protobuf runtime, with `tonic`) | **UNPINNED-REVERIFY** | Apache-2.0 (expected — crates.io metadata) | crates.io metadata — UNVERIFIED | **RE-VERIFY** at P7.17 |
| `buf` CLI (Protobuf `lint`/`breaking` for worker schemas, §10.5) | **UNPINNED-REVERIFY** (tool, not a cargo dependency) | Apache-2.0 (expected — `bufbuild/buf` repository metadata) | Repository metadata — UNVERIFIED | **RE-VERIFY**; outside `cargo-deny` scope — record-only here; the Buf *Schema Registry* (a commercial service, distinct from the CLI) must not be assumed into the toolchain without its own legal review |
| Python worker template deps (e.g. `grpcio`, `protobuf`) | **UNSELECTED** — P7.17 chooses the template's dependency set | `grpcio`: Apache-2.0; `protobuf`: BSD-3-Clause (per PyPI metadata — both UNVERIFIED) | PyPI metadata — UNVERIFIED | **ADD-THEN-VERIFY**: nothing is installed until each selected package is added to this table with a license entry and the human scan runs (extends to Python the ledger's "zero unknown-license items" rule) |

**Copyleft / unknown items flagged:** none identified in the direct set
above (all recorded licenses are permissive families). This is NOT a
clean bill: the transitive set is UNVERIFIED (no lockfile), and any
future item resolving to GPL/AGPL/EPL or "unknown" fails `deny.toml` by
construction (`unlicensed = "deny"`; copyleft families intentionally not
allowed) — the alternative ladder for such a flag is: replace, isolate
out-of-process, or escalate to legal (ledger P7.03 step 2). Ipopt (EPL)
remains the standing example: not a workspace dependency, recorded only
so D4 expects EPL if Stage 8 ever introduces it (`deny.toml` comment).

## 4. CVE posture for the WASM runtime (RESEARCH.md R7-C2 / §7)

| Item | Recorded position | Status |
|---|---|---|
| Floor | Wasmtime **≥50.0.0**; any pin below 48.0.7/49.0.4 is a known sandbox-escape exposure, not a stylistic choice (RESEARCH.md §1 implication) | Frozen as a Phase 7 boundary (ENTRY_NOTE.md item 4) |
| CVE-2026-115889 (GHSA-897f-g5gm-fmxm, CVSS 9.9, Cranelift bit-packing sandbox escape) | Fixed on the main line in 43.0.0 (correction of 7 Nov 2026) and in 36.0.19/48.0.7/49.0.4/50.0.0 | Carried by the pinned 50.0.1 per RESEARCH.md — **RE-VERIFY** |
| CVE-2026-115893 (GHSA-mghf-wjfp-p5xm, CVSS 9.9, aarch64 branch-table sandbox escape) | Fixed in 48.0.7 / 49.0.4 / 50.0.0 | Carried by the pinned 50.0.1 per RESEARCH.md — **RE-VERIFY** |
| CVE-2026-115384 (CVSS 9.9, async-callback stack overflow; fixed 48.0.4 / 49.0.2) | Carried by the pinned 50.0.1 per RESEARCH.md §7 task 1 — **RE-VERIFY at implementation time**: re-check all three GHSA pages, record the advisory IDs and the check date, and confirm the pinned version carries all three fixes (P7.13 step 4 owns the re-check) | RE-VERIFY — this audit does not discharge P7.13's step |

The pinned **50.0.1** therefore sits above the ≥50.0.0 floor on the
recorded evidence. The re-verification against CVE-2026-115384 (and the
other two advisories' pages) at implementation time is a named,
owned action — not a formality this record can wave through.

## 5. Carried items (prior-phase, still open — restated, not re-audited)

| Item | State | Owner |
|---|---|---|
| `rustls` / `rustls-pemfile` licenses + exact versions | UNVERIFIED crates.io metadata; UNPINNED-REVERIFY | Legal + engineering (first `cargo deny` run) — P6.02 §8 / P6.30 O-3 |
| OpenEMS license | UNVERIFIED (Apache-2.0 must NOT be assumed; REST boundary, no vendoring) | Legal + engineering — P6.02 §4 |
| Whole transitive set | UNVERIFIED until the first lockfile | First human `cargo deny check` — P6.27 |

## 6. SBOM input list (§9.3) and the P7.27 license gate (specified here, implemented there)

- §3's table **is the SBOM input list** for the SDK stack: per plugin
  package and per release, the SBOM must reconcile against it — a
  dependency present in the lockfile but absent from the table (or
  vice versa) is a finding (P7.14 consumes this; P7.27 wires the CI
  job).
- The ledger's step 4 ("add a CI check that fails on a new dependency
  without a license entry") is **specified here, implemented in P7.27**:
  the existing `deny.toml` posture (`unlicensed = "deny"`, `yanked =
  "deny"`) already encodes the fail-closed rule; P7.27 adds the CI
  wiring and demonstrates the failure once. No CI file is touched in
  this record.

## 7. `deny.toml` decision — NO EDIT MADE (recording preferred)

Every license family recorded in §3 maps to an entry **already present**
in the `deny.toml` `[licenses]` allow-list: `Apache-2.0 WITH
LLVM-exception` (wasmtime family), `MIT` (tonic, wit-bindgen's MIT leg),
`Apache-2.0` (prost), `BSD-3-Clause` (python `protobuf` if selected).
No new allowlist entry is structurally required, so **this record makes
no additive `deny.toml` edit** — recording over editing, per the P7.03
guidance. If a first lockfile reveals a family not on the list (e.g. an
unexpected transitive license), the additive entry must carry a dated
`# P7.03 — RE-VERIFY` comment and its own justification at that time;
copyleft stays disallowed.

## 8. Verification method (how this record gets falsified/completed)

Re-run the enumeration from the lockfiles once code exists
(`cargo tree` / the first `Cargo.lock`) and diff against §3; the first
human `cargo deny check licenses,advisories` is the verification point
for every UNVERIFIED cell above. Any diff is a finding against this
table, owned by Legal + engineering (D4).
