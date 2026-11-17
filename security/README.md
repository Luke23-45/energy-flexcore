# security/ — dependency-audit rollout configs (P19.09)

Status: **WRITTEN-UNRUN**. Every command below is a HUMAN GATE — no
scanner, audit, or advisory-database query has ever run in this
repository, and none is run by any agent loop. Owning record:
`docs/phases-11-20/phase-19-security-certification-compliance/records/09_p19_09_dependency_auditing_rollout.md`
(thresholds C-1..C-4 and the run discipline live there).

## Files in this tree

| File | Purpose |
|---|---|
| `pip-audit-suppressions.txt` | Registered suppressions for Python-stack audit runs. Format documented in the file header. No rows exist — there has never been a finding to suppress. |
| `npm-audit-suppressions.txt` | Registered suppressions for the console TypeScript stack. Same format and rules. |

## Files outside this tree (related, not owned by this tree)

| File | Status |
|---|---|
| `deny.toml` (repo root) | Pre-existing P6.27 cargo-deny policy. WRITTEN-NEVER-RUN. **Byte-identical** — this rollout does not edit it (the P8.03 §6 recording-over-editing precedent). |
| `.audit.toml` (repo root) | NEW (P19.09): cargo-audit configuration. `ignore` list EMPTY; an entry is added only with a dated, owned record and an expiry (the deny.toml `[advisories]` posture). |

## Human-gate run procedure (the authority is record 09 §5)

1. Preconditions (P10.19 §6 step 0 / P10.20 §3 step 0): committed
   lockfiles, resolved Python environments, recorded candidate identity,
   current SBOMs (P19.08 §6 F-2 ordering). All UNMET today.
2. Run, capturing each tool's own reported version + advisory-feed
   revision from its output (never from memory):

```sh
cargo deny check licenses
cargo deny check advisories
cargo audit
pip-audit                                    # resolved env; suppressions per security/pip-audit-suppressions.txt
(cd console && npm ci && npm audit --json)   # once the console lockfile exists
```

3. Apply the currency policy (record 09 §4): a run over a feed older than
   7 calendar days is COVERAGE-STALE (an unknown, never a clear); a stack
   with a single database consulted is COVERAGE-PARTIAL.
4. Record every finding (severity, component, exploitability-in-this-
   deployment reasoning) into the run record; triage per the
   vulnerability-management program (P19.12). Fix, suppress per C-3
   (with expiry), or escalate per the P19.06 §5 vocabulary — never
   silence silently.
5. Zero observed findings from a finite run is a test outcome about that
   run's coverage, never a guarantee (program conventions §5).

## Honesty

This tree contains configuration only. It certifies nothing, claims no
scan result, and asserts no advisory-database coverage. FlexCore is not
certified and no text here may imply otherwise (AGENTS.md §3 rules 2–3).
