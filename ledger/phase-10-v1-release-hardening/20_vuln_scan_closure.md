<!-- P10.20 — Vulnerability and dependency scan closure. HONEST RECORD:
     the scan closure is a PROCEDURE + EMPTY result record — no scanner
     has ever run against anything (nothing built, no SBOM exists, no
     advisory database queried in this loop). The result table (§6) is
     EMPTY: ZERO versions are cleared. The carried advisory state is
     cited, not re-researched: the P8.36 NOT-EXECUTED table (zero
     versions cleared there either), the Phase 7 wasmtime R7-C2 floor,
     and the Phase 1 JetStream CVE-2026-33222/33223 surface. The
     re-scan trigger rule (any version move) is carried from P8.36 §4
     and extended to the release candidate. An unqueried database is an
     UNKNOWN — a blank result is never recorded as "no issues found". -->
# P10.20 — Vulnerability and Dependency Scan Closure (procedure + EMPTY result record — zero versions cleared, verdict NOT CLOSED)

Date: 2026-11-14. Subtask: P10.20 (`ledger_phase_10.md` §P10.20).
Owner role: Security (with platform engineering). Report trace: §22.2
(container/dependency scanning); §17.3 (vulnerability management and
dependency scanning). Inputs: P10.18 candidate record
(`ledger/phase-10-v1-release-hardening/18_ci_gate_run_record.md` — the
scan CI steps G12/G18/G19, all UNRUN); P10.19 SBOM record
(`ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md` —
"scan the SBOM, not just the source tree"; no SBOM exists yet); the
P8.36 advisory procedure + NOT-EXECUTED table
(`ledger/phase-08-advanced-coordination/36_cve_pin_check.md`); the
Phase 7 wasmtime floor
(`ledger/phase-07-plugin-sdk/40_toolchain_pinning.md` §3); the Phase 1
NATS/JetStream CVE surface
(`ledger/phase-01-rust-foundation/RESEARCH.md` §5 and the P1.14 record).

## 1. Honest status — the plain statement

**No vulnerability or dependency scan has ever run in this repository's
history.** No container image exists to scan, no SBOM exists (P10.19
§1), no advisory database was queried in this loop (banned; ENTRY_NOTE
boundaries), and the release candidate does not exist (P10.18 §1).
**The result table (§6) is EMPTY — zero versions are cleared by this
record.** An unqueried advisory database is an UNKNOWN, never a "no
issues found" (the P8.36 §0 rule, carried verbatim in substance).

What this record delivers: the scan procedure per stack with exact
commands (§3), the re-scan trigger rule (§4), the carried advisory
state cited by path (§5), the EMPTY triage/result record (§6–§7), the
post-release vulnerability-management process note (§8 — written here
as the process the pilot period adopts, finalised by its human owner),
and the closure verdict (§10): **NOT CLOSED**. Ticking P10.20 in the
ledger means "implemented, awaiting human scan/triage" — the same
honest convention as every prior phase.

## 2. What this subtask consumes — and what does not exist yet

The ledger's inputs are the P10.18 candidate artifacts and the P10.19
SBOMs. Both are EMPTY today (P10.18 §6 non-claim 4: "No SBOM, advisory
result, or scan output exists"). The scans below therefore bind to the
candidate **when it exists**, and the ledger's own dependency order
(P10.18, P10.19 → P10.20) is enforced by the precondition list in §3
step 0. A scan of a source tree without a lockfile, or of a build that
is not the candidate, is the ledger's named failure mode
("scanning the source tree but not the shipped container image") — the
procedure is written to prevent exactly that.

## 3. The scan procedure per stack — exact commands, NEVER RUN (human gate)

Tools: the ledger says "tools per RESEARCH.md — e.g. Grype, Trivy,
OSV-Scanner; record the tool and version actually used". The tool NAMES
below are the ones the repository's own CI and records already name
(cargo-audit, cargo-deny, Syft, pip-audit, OSV, npm audit); where a
tool is an EXAMPLE the ledger offers, it is marked as such and the
actual tool + version is recorded at run time — never assumed.

**Step 0 — Preconditions (all must hold):**

- the candidate build exists with its identifier (P10.18 §4 filled);
- the SBOMs exist (P10.19 §6 procedure executed) — the scans in §3
  steps 3–5 run against the SBOMs and the built artifacts, not the
  source tree;
- the lockfiles (Cargo.lock, console lockfile, resolved Python
  environment) are committed or recorded next to the candidate.

**Step 1 — Rust workspace (advisories):**

```sh
cargo deny check advisories      # deny.toml [advisories]: yanked = "deny",
                                 # ignore = [] — never silenced without a
                                 # dated, owned record (the P6.27 posture)
cargo audit                      # the G18 CI step (RustSec advisory db)
```

The resolved versions come from the actual Cargo.lock, never from any
record (the P8.36 §3 rule for the Rust side, carried).

**Step 2 — Python workers/research stack:**

```sh
# after the environment is actually resolved (scratch venv, P8.03 §8):
pip freeze
pip-audit                        # PyPI advisory service; record tool version
# cross-check each pin against OSV.dev (ecosystem: PyPI) AND the
# vendor/project advisory surface — the P8.36 §3 step-3 rule, carried:
# no single database covers every framework.
```

**Step 3 — SBOM and container scanning (the ledger's step 1):**

```sh
grype sbom:sbom/<artifact>.spdx.json        # Grype is a ledger EXAMPLE;
trivy image <each released OCI image>       # record the tool(s) + versions
osv-scanner --sbom sbom/<artifact>.cdx.json # actually used, per artifact
```

Every released artifact class (control-plane image, plugin-host image,
worker images, edge-agent image, console bundle) is scanned; a
vulnerability in an unused code path is still recorded with its
exploitability reasoning (the ledger's step 2).

**Step 4 — Operator console (TypeScript):**

```sh
cd console && npm ci && npm audit --json > scan/console-audit.json
# only once the console lockfile exists (P10.19 §6 precondition);
# the tool version is recorded with the output.
```

**Step 5 — Triage, fix, re-scan (the ledger's steps 2–4):** every
finding gets severity, affected component, and exploitability-in-this-
deployment reasoning; every critical/high is fixed or formally
accepted (signed, time-bounded, carried into P10.26/P10.27); the
re-scan after fixes runs the SAME tools against the SAME
candidate-identifier artifacts and is recorded as the final scan. A
scanner version recorded that differs from the one run is the ledger's
named risk — the version is captured from the tool's own output, not
from memory.

## 4. The re-scan trigger rule — any version move

Carried from the P8.36 §4 table (`36_cve_pin_check.md` — cited, not
restated row for row) and extended to the release candidate:

| Trigger event | Re-scan scope |
|---|---|
| Any re-pin, patch bump, or lockfile regeneration before publication | the moved package(s) + the full advisory scan against the NEW candidate identifier |
| Any post-P10.18 candidate change (the P10.01 §6.2 drift rule) | the affected verifications re-run — including P10.20 in full; a scan of the old candidate is evidence for the old candidate only |
| New advisory disclosure affecting any pinned component after scan day | fresh dated lookup for that component before the go decision (P10.27) consumes this record |
| The torch/PyG, Ray/RLlib, grpcio/protobuf, HELICS/pyhelics resolutions (P8.36 §4 rows) | per that table — the checks travel WITH the pins |

A check "run once at audit time and never repeated when versions move"
is the ledger's named failure mode (P8.36 §4's words, carried); this
rule is the guard for the release window.

## 5. The carried advisory state — cited, NOT re-researched

No CVE/GHSA identifier below is new research by this record; each is
cited from its owning record and carries that record's verification
status.

| # | Carried surface | Owning record | Status as recorded there |
|---|---|---|---|
| A-1 | The P8.36 result table — every cell NOT-EXECUTED (ANDES, HELICS, pandapower, PyTorch/PyG, JAX, Ray/RLlib, Gymnasium, grpcio, protobuf, numpy/scipy, pytest, clarabel, wasmtime) | `36_cve_pin_check.md` §6 | **Zero pinned versions cleared**; "zero-clearance statement (honest)" is that record's own §6 conclusion — carried unchanged into this record |
| A-2 | wasmtime `=50.0.1` with the R7-C2 CVE floor (GHSA-897f-g5gm-fmxm / CVE-2026-115889, GHSA-mghf-wjfp-p5xm / CVE-2026-115893, CVE-2026-115384 — each CVSS 9.9, fixed in the listed versions) | `ledger/phase-07-plugin-sdk/40_toolchain_pinning.md` §3, as cited by P8.36 §2 | The advisory-page re-check and the hostile-component run remain the P7.13 §9 human gate; **not re-verified here** |
| A-3 | NATS/JetStream: CVE-2026-33222 (JetStream stream-restore endpoint auth bypass, GHSA-9983-vrx2-fg9c, CVSS 4.9) and CVE-2026-33223 (Nats-Request-Info header spoof, CVSS 6.4) — fixed in 2.11.15 / 2.12.6; the recorded Phase 1 pin NATS 2.15.2 includes those fixes (every 2.15.x does); the "do not run a version affected by CVE-2026-33222 without the fix" rule and the restore-authorization surface carry into the RC backup/restore test | `ledger/phase-01-rust-foundation/RESEARCH.md` §5; `ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` (the RC checklist row: "NATS server version carries the CVE-2026-33222 fix" — EMPTY/UNRUN) | The deployed-server version check is a P10.11 RC run cell, still EMPTY; nothing is re-verified by this record |
| A-4 | The Rust advisory gate (`deny.toml [advisories]`: yanked deny, empty ignore list; tokio pinned 1.54.1, never 1.54.0 — the Phase 1 CVE note) | `deny.toml` (P6.27) | WRITTEN-NEVER-RUN; the first `cargo deny check advisories` is the verification point |
| A-5 | The Python research stack has NO wired CI advisory scanner (the Rust-only coverage gap) | `36_cve_pin_check.md` §7 V-1; P8.03 §5 | Carried as a contract gap; §3 step 2 is the manual equivalent until wired |

## 6. The result table — EMPTY (zero versions cleared)

| Component / stack | Tool + database to query | Result | Critical/high disposition | Check date | Candidate ID scanned |
|---|---|---|---|---|---|
| Rust workspace (Cargo.lock) | `cargo deny check advisories` + `cargo audit` (RustSec) | **NOT-EXECUTED** | (EMPTY) | (EMPTY) | (EMPTY — no candidate exists) |
| wasmtime =50.0.1 | the three GHSA pages (Phase 7 §3) + RustSec | **NOT-EXECUTED — IDs cited (§5 A-2), not re-verified** | (EMPTY — owner: the P7.13 §9 human gate) | (EMPTY) | (EMPTY) |
| NATS server (deployed) | NATS release notes + the P10.11 RC version check | **NOT-EXECUTED** | (EMPTY — owner: P10.11 RC run) | (EMPTY) | (EMPTY) |
| Python workers/research stack | pip-audit + OSV + vendor surfaces | **NOT-EXECUTED** | (EMPTY) | (EMPTY) | (EMPTY) |
| Container images (each released artifact) | Grype/Trivy/OSV-Scanner over the SBOMs (ledger examples — actual tools recorded at run time) | **NOT-EXECUTED — no image exists** | (EMPTY) | (EMPTY) | (EMPTY) |
| Operator console (npm) | `npm audit` | **NOT-EXECUTED — no lockfile** | (EMPTY) | (EMPTY) | (EMPTY) |
| clarabel =0.11.1 (Rust) | `cargo deny check advisories` (RustSec) | **NOT-EXECUTED** | (EMPTY) | (EMPTY) | (EMPTY) |

**Zero-clearance statement (honest):** no version, package, image, or
bundle is cleared by this record. The ledger's acceptance items — (a)
final scan ran against candidate artifacts and SBOMs; (b) every
critical/high fixed or formally accepted; (c) no finding untriaged —
are NOT met and NOT claimed. The empty table is the honest current
state, exactly as the P8.36 §6 table anticipated.

## 7. Triage record — EMPTY template (human gate inputs)

| Field | Value |
|---|---|
| Final scan date / executor | `<EMPTY>` (human) |
| Tools + versions actually used (per stack) | `<EMPTY>` — captured from the tools' own output |
| Candidate identifier scanned | `<EMPTY>` (P10.18 §4 value) |
| SBOMs scanned | `<EMPTY>` (P10.19 §7 C-1 locations) |
| Findings (each: id, severity, component, exploitability reasoning) | `<EMPTY>` |
| Fixed findings (with the fix commit and the re-scan date) | `<EMPTY>` |
| Accepted findings (signed, time-bounded, carried to P10.26/P10.27) | `<EMPTY>` — an acceptance without a time bound is not an acceptance |
| Untriaged findings | `<EMPTY>` — must remain zero at closure; today the count is vacuous (no scan) |
| Re-scan result vs filed triage (the ledger verification method: re-run the scanner against the candidate SBOM) | `<EMPTY>` |

## 8. Post-release vulnerability-management process (pilot period) — recorded PROCEDURE

The ledger's step 5 requires the process note as a deliverable; this is
the recorded process, to be finalised (owners/dates filled) by the
human security owner at closure:

1. **Intake.** Advisory sources checked on a recorded cadence: RustSec
   advisory-db (via `cargo audit`/`cargo deny`), PyPI/OSV (pip-audit),
   npm audit DB, NATS/Wasmtime/PostgreSQL project advisories, and the
   vendor surfaces named in §5 — because no single database covers
   every framework (the P8.36 §5 coverage rule, carried). Critical/new
   disclosures affecting a shipped component are also ingested
   reactively (the P10.30 issue-intake link).
2. **Triage SLA.** Every new finding gets a severity and an
   exploitability-in-this-deployment assessment (unused code path: still
   recorded, with reasoning); critical/high findings on shipped
   artifacts are triaged before the next scheduled review (the cadence
   is recorded in P10.30, first review date set there).
3. **Patch path.** A fix lands as a version move → the §4 re-scan rule
   fires → a patched release follows the P10.12 upgrade path (tested)
   → the edge-agent updates ride the §18.3 signed-update verification
   (`crates/edge-agent/src/update_verification.rs` — verification
   before any write; never a silent downgrade).
4. **Acceptance discipline.** Any finding not fixed is formally
   accepted: signed, time-bounded, owner named, carried in the release
   notes (P10.26) and reviewed at the P10.30 cadence. An expired
   acceptance is an open finding.
5. **Coverage honesty.** A "no issues found" is recordable only when
   the record names which database was queried for that component and
   the component is within that database's coverage (P8.36 §5);
   otherwise COVERAGE-UNKNOWN — never a clear.

## 9. Findings recorded by this record

| # | Finding | Disposition |
|---|---|---|
| F-1 | Zero scans have ever run; zero versions are cleared; the §6 table and §7 template are EMPTY by honest state | Binding honesty statement |
| F-2 | The scan's required inputs (candidate artifacts, SBOMs, lockfiles) do not exist — P10.20 is ordered after P10.18/P10.19 for this reason, and the §3 step-0 preconditions enforce it | §2/§3 |
| F-3 | The carried advisory state is entirely NOT-EXECUTED/UNVERIFIED (P8.36 §6; the wasmtime re-check; the NATS version check) — this record inherits zero clearance, it does not grant any | §5 |
| F-4 | The Python advisory-scanner CI gap (V-1) persists into the release window — §3 step 2 is the manual equivalent until wired | §5 A-5; carried to the human closure |
| F-5 | The post-release process (§8) is written as a procedure; its owners/dates are unfilled human closure inputs | §8 |

## 10. Closure verdict

**Vulnerability and dependency scan closure for the v1.0.0 candidate:
NOT CLOSED.** No scan has run against any artifact; no finding is
triaged (vacuously — there are no findings because there was no scan);
zero versions are cleared. The ledger's acceptance (a)–(d) are
unmet until the human gate executes §3 and fills §6–§7. A go decision
(P10.27) that consumes an EMPTY §6 table is a go on an unscanned
artifact set — this record states that plainly so the meeting cannot
misread it.

## 11. Reviewer-verification procedure (human)

1. Confirm §1/§6/§7: no scan result, no finding, no clearance, no tool
   version is claimed; every Result cell is NOT-EXECUTED.
2. Confirm the carried citations resolve: P8.36 §0/§3/§4/§5/§6; Phase 7
   §3 (via P8.36 §2); Phase 1 RESEARCH.md §5 + P1.14; P10.11's
   CVE-2026-33222 row; deny.toml `[advisories]`.
3. Confirm no advisory database was queried by this record and no new
   CVE/GHSA identifier was introduced beyond the cited ones.
4. Confirm §4's trigger rule matches P8.36 §4 in substance and extends
   it to the candidate-identifier drift rule.
5. Confirm the ledger tick reads "implemented, awaiting human
   scan/triage" and the §10 verdict is NOT CLOSED.

## 12. Filling commands (human, at execution time)

```sh
# After P10.18 (candidate) and P10.19 (SBOMs) are filled:
cargo deny check advisories && cargo audit
pip-audit                                   # resolved worker/research env
grype/trivy/osv-scanner over the SBOMs      # actual tools recorded
(cd console && npm ci && npm audit --json)  # once the lockfile exists
# Fill §6/§7 from the actual outputs; re-run after every fix; record
# the tool versions from the tools' own output.

# Registry verification after any edit to this record:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 13. Status

**WRITTEN-UNRUN / PROCEDURE-ONLY.** The scan closure is recorded as a
per-stack procedure with EMPTY results, carried advisory state cited
honestly, and the verdict NOT CLOSED. No scanner ran, no database was
queried, no version is cleared, and no triage exists to waive.
