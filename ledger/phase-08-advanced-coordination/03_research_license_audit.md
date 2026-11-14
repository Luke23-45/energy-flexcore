# P8.03 — License and dependency audit for the research stack (D4 follow-through)

Date: 2026-11-13. Subtask: P8.03 (`ledger_phase_8.md` §P8.03).
Owner role: Legal + research lead. Report trace: §25.2 (D4); §9.3;
§7.4; §22.2. Inputs: Phase 8 `RESEARCH.md` (ANDES 2.0.0 GPLv3; PyTorch
2.15 / PyG 2.8.0 skew R8-C3; Ray/RLlib; Gymnasium 1.3.0; §7 open
verification tasks); Phase 5 `RESEARCH.md` (HELICS 3.7.0; pandapower
3.5.6); Phase 7 `03_sdk_license_audit.md` (grpcio/protobuf posture);
`python/workers/requirements.txt` honesty header; `apps/worker/python/
requirements.txt`; `deny.toml` (P6.27); `.agent/DECISIONS.md` (D4 OPEN).

## 1. Status of this audit — STATIC, nothing installed or run

This is a **static audit**: no Python package was installed, no venv was
created, no lockfile exists, no `cargo deny` / advisory scan ran (loop
bans; ENTRY_NOTE.md). The whole research stack is UNPINNED-REVERIFY or
RECORDED-NEVER-INSTALLED until the first human build/test pass resolves
and installs it. Every license below is recorded from RESEARCH.md
(primary-source checked at the 7 November 2026 cut-off) or from
project/PyPI metadata as recorded in prior-phase audit records, and is
labelled **UNVERIFIED** until legal confirmation and the first scan.
Zero-unknown-license claims are never made from this record.

D4 outcome: **D4 remains OPEN** (`.agent/DECISIONS.md`). This record
narrows what the human verification must cover and records the copyleft
isolation design; it does not close D4 and decides nothing (recording,
not deciding).

## 2. Research-stack dependency audit table (STATIC)

Class column separates the ledger's step-1 groups: **training** (ML/RL),
**simulation** (grid/dynamics/co-sim), **runtime** (worker transport),
**test/benchmark**. "Pin" is the recorded pin or UNPINNED-REVERIFY; no
version is invented where RESEARCH.md verified none.

| Dependency | Class | Pinned version | License (recorded) | Source of the claim | D4 action |
|---|---|---|---|---|---|
| ANDES | simulation | **2.0.0** (RESEARCH.md §4; R8-C5: pin 2.0.0 or a verified later release at implementation; live docs show a dev version string — do not cite "the docs" generically) | **GPLv3** (PyPI: "ANDES is licensed under the GPL v3 License", RESEARCH.md §4) | PyPI, primary — RESEARCH.md-verified | **ISOLATE (§3 below) + legal confirmation before any dependence.** Version UNVERIFIED until installed; re-check PyPI for a later release (RESEARCH.md §7 task 5) |
| HELICS | simulation | **3.7.0** — carried pin from Phase 5 (`RESEARCH.md` §1 of Phase 5: released 25 Aug 2026; `apps/worker/python/helics_federates/requirements-helics.txt`) | BSD-3-Clause — recorded as **found/reported** in Phase 5 RESEARCH (formal D4 audit open) | Project repository metadata via Phase 5 RESEARCH | **RE-VERIFY label + version at first install**; the matching `pyhelics` binding version is Phase 5's open task 3 (P8.01 G-6) |
| pandapower | simulation | **3.5.6** — carried pin (`apps/worker/python/requirements.txt`; Phase 4/5 records) | BSD-3-Clause (project repository; Phase 4 `01_02_grid_provider.md` D4 note — recorded as audit INPUT, not a formal audit) | Project repository metadata via Phase 4 records | **RE-VERIFY at first install**; never installed in this repo's history |
| PyTorch | training | **DECISION-POINT — nothing selected/installed** (see §4). RESEARCH.md §2 records 2.15 as current (28 Oct 2026) and the skew with PyG 2.8.0 (R8-C3) | BSD-3-Clause family **UNVERIFIED** — RESEARCH.md records no license finding for it | None recorded in this repo yet | **SELECT-THEN-VERIFY**: a verified-compatible torch/PyG pair is pinned per §4 before any GNN dependency exists; license confirmed from the project's primary source at pin time |
| PyTorch Geometric (PyG) | training | **DECISION-POINT** (with PyTorch). RESEARCH.md §2: newest found 2.8.0 (5 Jun 2026), supports PyTorch 2.9–2.12 only | MIT family **UNVERIFIED** — no license finding recorded | None recorded in this repo yet | **SELECT-THEN-VERIFY** per §4; re-check for a newer PyG release (RESEARCH.md §7 task 2) |
| JAX | training | **UNSELECTED** — no version recorded anywhere in this repo's research records (RESEARCH.md does not verify one) | Apache-2.0 family **UNVERIFIED** | None recorded in this repo yet | Recorded alternative only: if the research design selects JAX over the PyTorch default, that selection is its own pinned, licensed, CVE-checked decision (§4) |
| Ray / RLlib | training | **UNPINNED-REVERIFY** — RESEARCH.md §7 task 1: no version current as of 7 Nov 2026 verified from a primary source (2.56.1 / 2.57.0 from July–Aug 2026 secondary snapshots only) | Apache-2.0 family **UNVERIFIED** — no license finding recorded | None recorded in this repo yet | **RE-VERIFY version + license at P8.13**; the RLlib API-stack choice (new vs old, R8-C4) is P8.13 step 4; RLlib is now PyTorch-only (RESEARCH.md §3) — a Ray/RLlib selection therefore forces the PyTorch leg of §4 |
| Gymnasium | training | **1.3.0** (RESEARCH.md §3: uploaded 22 Apr 2026, Python 3.10+) | MIT family **UNVERIFIED** — no license finding recorded | PyPI via RESEARCH.md (version only) | **RE-VERIFY license at pin time**; confirm the version still current (RESEARCH.md §7) |
| grpcio | runtime (worker template) | **UNPINNED-REVERIFY** — deliberately un-installable pin line (`python/workers/requirements.txt`; carried from P7.17) | Apache-2.0 (PyPI metadata — UNVERIFIED, P7.03 §3) | PyPI metadata via P7.03 | **RE-VERIFY** per the recorded procedure (requirements.txt honesty header item 3): look up current releases, confirm the grpc/protobuf compatibility table, record versions, then install into a scratch venv |
| protobuf (Python) | runtime (worker template) | **UNPINNED-REVERIFY** (same source) | BSD-3-Clause (PyPI metadata — UNVERIFIED, P7.03 §3) | PyPI metadata via P7.03 | **RE-VERIFY** together with grpcio (compatibility table) |
| numpy / scipy | simulation/training (transitive) | **UNPINNED-REVERIFY** — transitive under pandapower/ANDES/torch; no direct pin recorded anywhere in this repo | BSD-3-Clause family **UNVERIFIED** | Ecosystem metadata only — UNVERIFIED | **RE-VERIFY at the first install**: resolve exact versions from the environment, add them to this table with sources; transitive licenses land in the first real scan |
| pytest | test/benchmark | **UNPINNED-REVERIFY** (`python/workers/requirements.txt` comment) | MIT family **UNVERIFIED** | PyPI metadata — UNVERIFIED | **RE-VERIFY** at the first real test run |
| Rust-side research dependencies | — | **NONE NEW RECORDED.** The research workers are Python (ADR-004); the Rust side orchestrates through existing workspace crates (`clarabel =0.11.1`, `flexcore-optimization-api`, `flexcore-grid-api`, `apps/sim-runner`, `apps/plugin-host`). No ANDES/HELICS/torch equivalent may EVER enter the cargo tree (§3) | n/a | n/a | If the P8.04–P8.06 owner (concurrent wave) introduces a new workspace member, THAT owner adds its dependency table + license rows to its own record and reconciles against this table; this record audits the recorded research stack, not that crate |

Copyleft items flagged: **ANDES (GPLv3) only.** Everything else in the
table is a recorded permissive family, all UNVERIFIED until confirmed.
GPLv3 is intentionally NOT in the `deny.toml` allow-list — ANDES must
never appear in the cargo tree (§3, §6). Ipopt (EPL) remains the
standing recorded example of an expected-but-absent dependency
(`deny.toml` comment; P7.03 §3).

## 3. Copyleft consequence record — ANDES GPLv3 isolation design

Ledger P8.03 step 2, recorded here as the isolation design; **legal
confirmation is a named gate that this record cannot grant** (P8.01 G-4):

| # | Isolation rule | Basis |
|---|---|---|
| 1 | ANDES lives ONLY inside an isolated Python research worker (Phase 7 Class C, gRPC), per ADR-004 (Python isolated to scientific/numerical/research workers) | §7.2; §5.4; PLAN.md §3 |
| 2 | No core crate, no SDK crate, no app depends on ANDES. The Rust side never links, vendores, or FFI-bridges ANDES code | RESEARCH.md R8-C2 ("an implementer who adds ANDES as a direct dependency of a core crate would create the conflict") |
| 3 | FlexCore consumes ANDES **outputs** (traces, eigenvalues) as data — scenario artifacts over the worker boundary (§8.2: workers communicate via Protobuf/gRPC or scenario artifacts, never internal tables) — never ANDES-derived code | §8.2; P7.31 §8 (dynamic worker row) |
| 4 | Any distribution of ANDES-derived code goes through legal before it happens | RESEARCH.md R8-C2; ledger P8.03 step 2 |
| 5 | The GPLv3 boundary is verified by inspecting the worker boundary at P8.16 (the ledger's verification method); the import-graph check is the evidence, not this table | ledger P8.03 / P8.16 verification methods |
| 6 | Dynamic results cross the trust boundary as UNTRUSTED INPUT (P7.31 §5 transport seam) and are never dispatch inputs (P8.16 boundary statement; §19.1) | P7.31 §5; PLAN.md §5 |

**Consequence statement:** the dynamic-simulation work is isolated as a
research worker, and its outputs (not its code) are what FlexCore
consumes. GPLv3 code is never linked into the Apache-2.0-RECOMMENDED
core (the recommendation is itself not a decision — `.agent/DECISIONS.md`).

## 4. DECISION-POINT — GNN/MARL framework (torch vs jax): recorded DEFAULT, nothing selected

This is the ledger's step-3 slot in the D4 sense: record the choice
space and its consequences; do not select-and-install.

| Item | Recorded content |
|---|---|
| Report default | PyTorch + PyG for GNN work (§7.4); RLlib as the named RL framework (§7.4) — both recorded DEFAULTS/recommendations, not decisions |
| Current blocker | R8-C3 version skew: PyTorch 2.15 (28 Oct 2026) vs PyG 2.8.0 (5 Jun 2026) supporting PyTorch only up to 2.12. Pinning "latest + latest" on assumption is prohibited; a pair that installs but silently changes numerics is worse than one that fails loudly (RESEARCH.md §2 implication) |
| Recorded resolution rule | P8.03/P8.10 must pin a **verified-compatible pair** (e.g. PyTorch 2.12 + PyG 2.8.0 IF that pair is verified at implementation time) or re-check for a newer PyG release and record the result (RESEARCH.md §7 task 2). Owner: Research engineer (ML) + legal |
| MARL constraint | RLlib is now PyTorch-only and its new API stack is default (RESEARCH.md §3, R8-C4); a Ray/RLlib selection forces the PyTorch leg and is pinned in P8.13 with the API stack recorded |
| JAX alternative | If the GNN/MARL design selects JAX instead, that selection is its own recorded decision with version pins, license verification, and CVE check — no version or license is recorded here beyond the row in §2 |
| License implications | Both candidate families are permissive (PyTorch BSD-style / JAX Apache-2.0, UNVERIFIED) — neither is the headline licensing risk of this phase; ANDES GPLv3 is (§3). The PyG/extension packages (`pyg-lib` 0.7.0 per RESEARCH.md §2) inherit their own verification at pin time |
| CVE implications | Whatever is pinned is checked AT PIN TIME by the P8.36 procedure (§5) — not by this record |
| Status | DECISION-POINT OPEN — no framework is selected, installed, or pinned; P8.10/P8.13 resolve it with verified pins and this table is updated then |

## 5. CVE/advisory check — the P8.36 seam (recorded, not run)

The advisory check for every pinned research dependency (PyTorch, PyG,
Ray/RLlib, Gymnasium, ANDES, HELICS, pandapower) is **P8.36's procedure,
run at pin time** (GHSA/OSV/vendor advisories, recorded per version with
a date; known critical/high issues block use or carry a recorded
compensating measure with an owner). This record does not run any
advisory lookup — RESEARCH.md §7 task 4 records the same open task, and
the check only means something once versions are actually pinned and
installed. `deny.toml [advisories]` covers the Rust tree only; the
Python research stack needs the P8.36 procedure (or an equivalent Python
scanner at CI activation — recorded as a P8.28/P8.36 seam, not configured here).

## 6. `deny.toml` decision — NO EDIT MADE (recording preferred)

- **GPLv3 stays disallowed** — the `[licenses]` allow-list contains no
  GPL family, by construction and on purpose; ANDES must never enter the
  cargo tree (§3), so no allow-list change is warranted for it.
- Every permissive family recorded in §2 (BSD-3-Clause, MIT, Apache-2.0)
  is already allow-listed in `deny.toml`; **no additive entry is
  structurally required**, so this record makes no `deny.toml` edit —
  recording over editing, per the P7.03 precedent.
- If the first lockfile (or a new research-crate dependency from the
  P8.04–P8.06 owner) reveals a family not on the list, the additive
  entry carries a dated `# P8.03 — RE-VERIFY` comment and its own
  justification at that time. Copyleft stays disallowed.

## 7. SBOM input and CI license gate (ledger P8.03 step 4 — recorded here, implemented later)

§2's table is the **SBOM input list for the research stack**: when the
research environment is first resolved/installed, the SBOM must
reconcile against it — a dependency present in the environment but
absent from this table (or vice versa) is a finding owned by Legal +
research lead. The CI license-gate wiring for Python dependencies does
not exist yet (P7.27 deliberately left the Python worker gates unwired —
P7.30 O-8); adding it is an activation-wave action that consumes this
table, not a change made in this record.

## 8. Verification method

Re-run the enumeration from the environment once it exists
(`pip freeze` / lockfile in a scratch venv, never committed) and diff
against §2 — any unlisted dependency is a finding. Confirm the ANDES
isolation by inspecting the worker boundary (P8.16 verification method;
import-graph check). Legal confirmation of §3 is the D4 gate for ANDES.
The P8.36 advisory checks and the first `cargo deny check` are the
verification points for every UNVERIFIED cell.

## 9. Verification tools (permitted only; run after this record was written)

Output quoted in the agent completion report and in
`01_readiness_check.md` §8. These check documentation and structure
only; no install, build, scan, or advisory lookup was performed for this
audit.
