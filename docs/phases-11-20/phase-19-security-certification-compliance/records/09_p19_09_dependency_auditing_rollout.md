<!-- P19.09 — Dependency auditing rollout. HONEST RECORD: NEW config files
     written-unrun around the never-run deny.toml (which stays
     BYTE-IDENTICAL — the P8.03 §6 recording-over-editing precedent). No
     scanner has ever run; zero findings are NOT claimed; an unqueried
     advisory database is an UNKNOWN (the P10.20 §1 rule). Every run is a
     human gate. The advisory-feed currency policy registers thresholds
     (X-8) before any run. The not-a-certification discipline of P10.25 §7
     (§10) carries verbatim. -->
# P19.09 — Dependency Auditing Rollout (cargo-deny / cargo-audit / pip-audit / npm-audit configs written-unrun as NEW files; advisory-feed currency policy; every run a human gate)

Date: 2026-11-17. Subtask: P19.09 (`ledger_phase_19.md` §P19.09).
Plan trace: `../plan.md` §2 row P19.09, §4 non-goals (deny.toml
byte-identical), §7 R-P19-3 (unrun configs may false-fail or surface
findings). Sources: `deny.toml` (P6.27, WRITTEN-NEVER-RUN — cited,
untouched); the P10.20 scan-closure record §3/§8
(`ledger/phase-10-v1-release-hardening/20_vuln_scan_closure.md`); the
P10.19 §6 procedure; the P8.03 §6 recording-over-editing precedent.

## 1. Honest status — the plain statement

**No dependency audit, advisory scan, or vulnerability database query has
ever run in this repository's history, and this record runs none.** The
configs below are NEW files, WRITTEN-UNRUN; the first human run of any of
them is the verification point and may false-fail or surface findings
(plan §7 R-P19-3). Zero findings are NOT claimed anywhere; a run's blank
result is never "no issues found" (the P10.20 §1 rule, carried). The
pre-existing `deny.toml` (P6.27) is cited and stays **byte-identical** —
verified at the end of this batch by a hash recorded in the close-out
record (`19_phase_19_wave1_agent_a_close_out.md`).

## 2. Files created by this subtask (all NEW, all WRITTEN-UNRUN)

| File | Purpose | Consumed by |
|---|---|---|
| `security/README.md` | The config-tree index and the human-gate runbook: exact commands, ordering, result-recording rule | The human auditor |
| `.audit.toml` (repo root) | cargo-audit configuration (RustSec advisories), alongside the existing deny.toml; its `ignore` list starts EMPTY and follows the same dated-owned-record rule as deny.toml's `[advisories] ignore` | `cargo audit` (first human run) |
| `security/pip-audit-suppressions.txt` | The registered suppression manifest for Python-stack audit runs (documented program format: vuln id, package, reason, owner, expiry — an expired row is invalid) | The pip-audit runbook step (suppress via the tool's documented flag, values copied from this manifest) |
| `security/npm-audit-suppressions.txt` | The same manifest form for the console TypeScript stack | The npm-audit runbook step |

Placement note: the repo root already hosts `deny.toml` (P6.27); the
`security/` tree is the new home for the rollout configs so the policy
posture stays in one discoverable place. No existing file is moved,
edited, or replaced.

**Config-format honesty**: the `.audit.toml` grammar and the two
suppression-manifest formats are this program's recorded conventions —
the cargo-audit config grammar is UNVERIFIED against the tool's
documentation and is checked at the first human run (the unnamed-tool
rule); a mismatch is a fix of THIS file, never a silent workaround of a
finding.

## 3. What each tool covers (and the known coverage gap, carried)

| Stack | Tool | Config file | Coverage posture |
|---|---|---|---|
| Rust workspace (RustSec advisories + license/ban/source policy) | `cargo deny check` (the existing deny.toml) + `cargo audit` | `deny.toml` (UNTOUCHED) + `.audit.toml` (NEW) | The deny.toml postures carried by citation: `unlicensed = "deny"`, copyleft not allowed, `yanked = "deny"` with empty ignore list — no verdict exists until the first run |
| Python workers/research | `pip-audit` over the resolved environment | `security/pip-audit-suppressions.txt` (NEW) | The P10.20 §5 A-5 gap (no wired CI scanner) persists; the runbook step is the manual equivalent until wired — carried, not closed |
| Console TypeScript | `npm audit --json` | `security/npm-audit-suppressions.txt` (NEW) | Runs only once the console lockfile exists (P10.19 §6 precondition) |

No container-image scanner config is created here (Grype/Trivy/OSV stay
the P10.20 §3 step-3 examples, recorded at run time); this subtask's scope
is the four named tools.

## 4. Advisory-feed currency policy — thresholds registered (X-8) BEFORE any run

| Threshold | Registered value | Consequence |
|---|---|---|
| C-1 Feed-freshness ceiling | A run is CURRENCY-VALID only if every feed it consults was updated (tool's own report of the fetched database revision) within **7 calendar days** of the run date. Registered as the pre-run ceiling; revisable only by a dated, reviewed change BEFORE the first gate that relies on it | A run over a stale feed is recorded **COVERAGE-STALE** — it is not a clear, not a failure either; it is an unknown (the P10.20 §8 coverage-honesty rule) |
| C-2 Multi-database rule | No single database covers every framework (the P8.36 §5 rule, carried): the Rust stacks consult RustSec via both `cargo deny check advisories` and `cargo audit`; the Python stack consults the PyPI advisory service plus an OSV cross-check per the runbook; the console consults the npm advisory DB | A stack with only one database consulted is recorded COVERAGE-PARTIAL — never a clear |
| C-3 Suppression expiry | Every suppression row in the two manifests carries an expiry; an expired row is INVALID and the suppressed finding reverts to an open finding. No suppression exists without a dated reason, an owner, and an expiry (the P6.27 posture: advisories are never silenced without a dated, owned record) | Enforced at each human run by reviewing the manifest |
| C-4 Re-scan triggers | The P10.20 §4 re-scan trigger rule (any version move) applies unchanged to every tool this record rolls out | Carried, not re-derived |

All four thresholds are registered BEFORE any run; none has been applied
(no run exists). Dynamic results (findings, counts, feeds' revisions) are
structurally separate from this record: they land in the run record the
human fills (§5), never back into this file.

## 5. The human-gate run procedure (recorded; the runbook's authority is `security/README.md`)

1. Preconditions per P10.19 §6 step 0 / P10.20 §3 step 0 (lockfiles,
   resolved environments, candidate identity) — unmet today.
2. SBOMs current per P19.08 §6 F-2 (the audits scan what the SBOM
   describes — cited, not re-ordered here).
3. Run the tools in the runbook's order, capturing each tool's own
   version + feed revision from its output (never from memory — the
   P10.20 §3 step-5 rule).
4. Apply C-1..C-4; record every finding with severity, component, and
   exploitability-in-this-deployment reasoning into the run record (the
   triage SLAs are the P19.12 program's — Agent B, PENDING-P19.12, cited
   by path).
5. A finding is fixed, suppressed per C-3 (with expiry), or escalated per
   the P19.06 §5 vocabulary — never silenced silently.

## 6. What this record does NOT do

- It does not edit `deny.toml` (byte-identity verified in the close-out
  record), create a second cargo-deny policy, or reinterpret any P6.27
  entry.
- It does not wire any CI job (the CI workflows are UNRUN and are not
  edited by this phase; wiring is a future implementation batch with
  real gates).
- It does not claim any scan outcome, database currency, or coverage.

## 7. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

A dependency audit is an advisory-coverage check — evidence for triage,
never a security certification of the codebase.

## 8. Reviewer-verification procedure (human)

1. Confirm §2's files exist as NEW files and that `deny.toml` matches the
   P6.27 state byte-for-byte (hash in the close-out record).
2. Confirm the `.audit.toml` `ignore` list is EMPTY and both suppression
   manifests contain no rows (no suppression exists — there has never
   been a finding to suppress).
3. Confirm §4's thresholds are registered with their first application
   NOT-STARTED and no dynamic result appears in this record.
4. Confirm §3's coverage gap (Python CI gap A-5) is carried, not closed.
5. Confirm §7's verbatim block matches `AGENTS.md` §3 rules 2–3 exactly.

## 9. Filling commands (human, at execution time)

```sh
# After the P10.18/P10.19 preconditions hold (lockfiles, SBOMs):
cargo deny check licenses
cargo deny check advisories
cargo audit
pip-audit                                    # resolved env; suppressions per the manifest
(cd console && npm ci && npm audit --json)   # once the console lockfile exists
# Record each tool's own version + feed revision; fill the run record;
# apply C-1..C-4; triage per P19.12's program.
```

## 10. Status

**CONFIGS-WRITTEN-UNRUN.** Four new files exist; nothing has been
installed, built, or run; zero findings are claimed (there has been no
scan to have findings).
