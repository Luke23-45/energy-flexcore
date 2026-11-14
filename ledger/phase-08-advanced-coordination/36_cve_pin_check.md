# P8.36 — Research-stack CVE/advisory check AT PIN TIME: the recorded PROCEDURE + NOT-EXECUTED result table

Date: 2026-11-14. Subtask: P8.36 (`ledger_phase_8.md` §P8.36). Owner
role: security engineer. Report trace: §22.2 (license/security review;
container/dependency scanning); §17.3 (vulnerability management).
Inputs: P8.03 (`03_research_license_audit.md` — the pin table this
record verifies at pin time, §2 of that record), P8.25
(`25_security_review.md` — the security review this check accompanies;
RESEARCH.md §7 task 4 assigns the advisory check here because P8.25
never performs it), Phase 7 (`ledger/phase-07-plugin-sdk/
40_toolchain_pinning.md` §3 — the wasmtime CVE floor, cited not
restated as new research). Structure and run-status honesty follow
`03_research_license_audit.md` §5 (the P8.36 seam that record already
stated) and the P7.29/P8.25 honesty pattern.

## 0. Run-status honesty — READ FIRST

**NOT-EXECUTED.** No advisory database was queried in this loop, no
network lookup was performed, nothing was installed, no scanner ran
(the loop bans; ENTRY_NOTE.md boundary 5: "P8.36 does the CVE check AT
PIN TIME as a recorded procedure"). **Every result cell in §4 is
UNRUN / NOT-EXECUTED.** No CVE number is asserted as new research: the
only CVE/GHSA identifiers in this record are CITED from the Phase 7
toolchain-pinning record (the R7-C2 wasmtime floor) and are marked
"as recorded in Phase 7 — not re-verified here". A blank advisory
result is never recorded as "no issues found" — an unqueried database
is an UNKNOWN, and the ledger's own risk row ("an advisory database
that does not cover the framework in question, recorded as 'no issues
found'") is enforced by §5's coverage rule.

## 1. The pin list being checked (verbatim inputs from `03_research_license_audit.md` §2)

| Dependency | Class | Pin as recorded | What the check must resolve |
|---|---|---|---|
| ANDES | simulation | **2.0.0** (R8-C5: pin 2.0.0 or a verified later release at implementation) | GHSA/OSV/vendor advisories for the ACTUAL installed version at first install; the pin may move per R8-C5 — the check binds to the moved pin |
| HELICS | simulation | **3.7.0** (carried from Phase 5) | Advisories for 3.7.0 and the matching `pyhelics` binding version (Phase 5 open task 3) |
| pandapower | simulation | **3.5.6** (carried; never installed in this repo's history) | Advisories at first install |
| PyTorch | training | **DECISION-POINT — nothing selected/installed** (P8.03 §4; R8-C3 skew vs PyG 2.8.0) | The check runs AGAINST THE PAIR ACTUALLY PINNED when P8.10 resolves the DECISION-POINT — never against an assumed "latest" |
| PyTorch Geometric (PyG) | training | **DECISION-POINT** (with PyTorch) | Same trigger; `pyg-lib` 0.7.0 (RESEARCH.md §2) inherits its own check at pin time |
| JAX | training | **UNSELECTED** (recorded alternative only) | If selected, its own pinned version is checked as its own decision (P8.03 §4) |
| Ray / RLlib | training | **UNPINNED-REVERIFY** (no primary-source version as of the 2026-11-07 cut-off) | Check at the P8.13 selection, against the version actually pinned |
| Gymnasium | training | **1.3.0** (recorded; license UNVERIFIED) | Advisories for 1.3.0 at pin time |
| grpcio | runtime | **UNPINNED-REVERIFY** (deliberately un-installable pin line; carried from P7.17) | Check at the recorded re-verify procedure (the requirements.txt honesty header item 3: look up current releases, confirm the grpc/protobuf compatibility table, install into a scratch venv) |
| protobuf (Python) | runtime | **UNPINNED-REVERIFY** | Checked together with grpcio |
| numpy / scipy | transitive | **UNPINNED-REVERIFY** | Resolved versions from the first real environment; transitive advisories land in the first real scan |
| pytest | test | **UNPINNED-REVERIFY** | Check at the first real test run |
| clarabel (Rust) | research crate dep | **=0.11.1** (root `Cargo.toml`) | RustSec advisory-db via `cargo deny check advisories` (`deny.toml [advisories]` covers the Rust tree — P8.03 §5) |
| wasmtime (Rust, plugin host) | carried from Phase 7 | **=50.0.1** with the R7-C2 CVE floor | Re-check the three GHSA advisory pages at the Phase 7 gate — AS RECORDED in Phase 7 (§2 below), not re-verified here |

## 2. Carried Phase 7 record (cited, not restated as new research)

The wasmtime pin `wasmtime = "=50.0.1"` carries the three October 2026
fixes **as recorded in `ledger/phase-07-plugin-sdk/
40_toolchain_pinning.md` §3 (R7-C2)**: GHSA-897f-g5gm-fmxm /
CVE-2026-115889, GHSA-mghf-wjfp-p5xm / CVE-2026-115893, and
CVE-2026-115384 (each CVSS 9.9, fixed in the listed versions — the
details live in that record and are NOT re-verified by this one). The
advisory-page re-check, the check date, and a real hostile-component
run remain the P7.13 §9 human gate. This record adds no CVE finding
and claims no freshness for those advisories.

## 3. The procedure (the exact lookup steps the human gate executes)

**Rust-side pins (clarabel, wasmtime, the flexcore-research tree):**

1. `cargo deny check advisories` (the `deny.toml [advisories]`
   configuration, P6.27/P8.03 §5) against the RustSec advisory-db —
   the resolved versions come from the actual lockfile, never from
   this record.
2. For each flagged advisory: record the advisory id, the affected
   version range, whether the pinned version is inside the range, and
   the decision (block-use / pin a fixed version / compensating
   measure WITH AN OWNER).
3. For wasmtime specifically: re-read the three GHSA advisory pages
   named in Phase 7 §3 and record the retrieval date next to the pin
   (the Phase 7 requirement).

**Python-side pins (ANDES, HELICS, pandapower, torch, PyG, Ray/RLlib,
Gymnasium, grpcio, protobuf, numpy/scipy, pytest):**

1. After the environment is actually resolved and installed (scratch
   venv, never committed — the P8.03 §8 method), record the exact
   installed version per package (`pip freeze` diffed against the
   P8.03 §2 table; any unlisted dependency is a finding owned by
   Legal + research lead).
2. Run `pip-audit` over the resolved environment (PyPI advisory
   service) — the tool records the queried database and version per
   package.
3. Cross-check each pin against OSV.dev (ecosystem: PyPI) by package
   name + version, and against the vendor/project advisory surface
   (GitHub security advisories / release notes) for ANDES, HELICS,
   pandapower, torch, PyG, and Ray — because no single database
   covers every framework (the ledger's named risk; §5).
4. **Record per version** (the ledger deliverable): package, pinned
   version, tool + database queried, check date, result, and — for
   any known critical/high issue — the decision EITHER WAY (fixed
   version pinned, or compensating measure recorded with a named
   owner). A version with no recorded dated check is NOT cleared.
5. The check result travels WITH the pin: the pin's owning record
   (P8.03 §2 updates, the P8.10/P8.13 DECISION-POINT resolutions, the
   re-verify items) gains the check date and result.

## 4. The re-check TRIGGER (the ledger's stated risk, made explicit)

**Any version move re-runs the check before use.** Concretely, each of
these events invalidates the previous check and requires a fresh dated
lookup for the moved package (and its lockstep pairs):

| Trigger event | Re-check scope |
|---|---|
| The torch/PyG DECISION-POINT resolves (P8.10) | torch + PyG + pyg-lib, both legs' advisories |
| The Ray/RLlib selection is pinned (P8.13) | ray/rllib (+ its forced PyTorch leg) |
| ANDES is installed at 2.0.0 or a later verified release (R8-C5) | ANDES at the installed version |
| The grpcio/protobuf re-verify resolves versions | grpcio + protobuf together (compatibility table) |
| HELICS/pyhelics binding pin (Phase 5 open task 3) | HELICS + pyhelics |
| Any transitive resolution change (numpy/scipy) | the resolved versions |
| Any re-pin, patch bump, or lockfile regeneration | the moved package(s) |
| A new Rust-side dependency enters the research crate | `cargo deny check advisories` re-run + the P8.03 §2 table reconciliation |

A check "run once at audit time and never repeated when versions move"
is the ledger's named failure mode — this table is the recorded guard.

## 5. Advisory-coverage rule (the second named risk)

A "no issues found" is only recordable when the record names WHICH
database was queried for THAT package and the package is within that
database's coverage; if the queried database does not cover the
framework (the ledger's risk: "an advisory database that does not
cover the framework in question, recorded as 'no issues found'"), the
result cell records **COVERAGE-UNKNOWN — not a clear**, and the
vendor-surface lookup of §3 step 3 becomes mandatory. No coverage
claim is made in this record because nothing was queried.

## 6. The check RESULT table (every cell NOT-EXECUTED — the lookups are human-gate inputs)

| Dependency | Pinned version checked | Tool + database to query | Result | Decision / owner (for any critical/high) | Check date |
|---|---|---|---|---|---|
| ANDES | (NOT-EXECUTED — not installed) | pip-audit + OSV + ANDES project advisories | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| HELICS | (NOT-EXECUTED) | OSV + HELICS project advisories | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| pandapower | (NOT-EXECUTED) | pip-audit + OSV | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| PyTorch | (NOT-EXECUTED — DECISION-POINT open) | pip-audit + OSV + PyTorch advisories | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| PyG / pyg-lib | (NOT-EXECUTED — DECISION-POINT open) | pip-audit + OSV + PyG advisories | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| JAX | (NOT-EXECUTED — UNSELECTED) | (if selected: pip-audit + OSV) | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| Ray / RLlib | (NOT-EXECUTED — UNPINNED-REVERIFY) | pip-audit + OSV + Ray advisories | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| Gymnasium | (NOT-EXECUTED) | pip-audit + OSV | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| grpcio | (NOT-EXECUTED — UNPINNED-REVERIFY) | pip-audit + OSV | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| protobuf (Python) | (NOT-EXECUTED — UNPINNED-REVERIFY) | pip-audit + OSV | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| numpy / scipy | (NOT-EXECUTED — transitive, unresolved) | first real scan (pip-audit) | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| pytest | (NOT-EXECUTED — UNPINNED-REVERIFY) | pip-audit + OSV | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| clarabel =0.11.1 (Rust) | (NOT-EXECUTED — no cargo run) | `cargo deny check advisories` (RustSec) | **NOT-EXECUTED** | (EMPTY) | (EMPTY) |
| wasmtime =50.0.1 (Rust) | (NOT-EXECUTED — carried pin) | the three GHSA pages of Phase 7 §3 + RustSec | **NOT-EXECUTED — the Phase 7 record's IDs are cited (§2), not re-verified** | (EMPTY — owner: the P7.13 §9 human gate) | (EMPTY) |

**Zero-clearance statement (honest):** no pinned version is cleared by
this record. The ledger's acceptance item ("every pinned version has a
recorded advisory check with a date; zero known critical/high issues
are open without a recorded decision and owner") is NOT met and is NOT
claimed — the table being empty is the honest current state, exactly
as the P8.03 record anticipated (§5 of that record).

## 7. Contract gaps → follow-ups (recorded, never invented fixes)

| # | Gap | Owner |
|---|---|---|
| V-1 | The Python research stack has NO CI advisory scanner wired (`deny.toml [advisories]` covers the Rust tree only) — the scanner wiring is the P8.28/P8.36 seam recorded in P8.03 §5, activated when CI exists | P8.28 owner |
| V-2 | The re-verify items (grpcio/protobuf, HELICS binding, Ray/RLlib, torch/PyG pair) resolve only at the first human install — this record's §3/§4 become fillable only then | the pins' owning records (P8.10/P8.13/P8.16 + P8.03 §2 updates) |
| V-3 | `pip freeze` / SBOM reconciliation against the P8.03 §2 table (any unlisted dependency is a finding) — first-install action | Legal + research lead (P8.03 §8) |

## 8. Verification status (honest)

| Item | Status |
|---|---|
| Advisory lookups (any §6 row) | **NOT EXECUTED — banned in this loop; human-gate input** |
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (must still report OK; no dependency edge added) |
| CVE/GHSA identifiers in this record | **CITED from `ledger/phase-07-plugin-sdk/40_toolchain_pinning.md` §3 only; none researched or verified here** |
