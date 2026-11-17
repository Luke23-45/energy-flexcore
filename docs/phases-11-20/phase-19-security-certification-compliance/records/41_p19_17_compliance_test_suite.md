<!-- P19.17 — compliance test/evidence suite additions. HONEST RECORD:
     every deliverable is WRITTEN-UNRUN: no test was compiled or executed,
     no script was run, nothing was installed or built, no scanner/audit
     tool was invoked. Execution is the human gate (conventions §1 item 3).
     The ledger was NOT edited and no checkbox was ticked by this agent.
     The three PERMITTED verification tools ran once at close-out and their
     outputs are quoted VERBATIM in §7. No certification, attestation,
     scan result, or audit outcome is claimed anywhere. -->
# P19.17 — Compliance test/evidence suite additions (written-unrun)

Date: 2026-11-17. Agent: Phase 19 wave 2, Agent A. Subtask: P19.17
(`ledger_phase_19.md` §P19.17 — not edited by this agent). Plan trace:
`../plan.md` §2 row P19.17. Conventions:
`../../01_PROGRAM_CONVENTIONS.md` §1 item 3 (execution is the human gate),
§2 (ledger discipline — records only; ticks are the orchestrator's), §5
(human gates / X-8), §7 (honesty rules); AGENTS.md §3 rules 2–3 carry
verbatim: **FlexCore is not certified, not a safety case, and not
permission for live grid control.** Nothing here claims a certification,
an attestation, a scan result, an audit outcome, or any tamper-resistance
beyond the P11.15 design's own "keyed tamper-EVIDENCE, never
tamper-PROOF" boundary.

## 0. Anchors consumed (by path, read before writing)

| Anchor | Where | What was consumed |
|---|---|---|
| The P11.15 chain design | `crates/flexcore-audit/src/chain.rs` (READ first, per the subtask) and `../../phase-11-operational-robustness/records/31_p11_15_audit_chain_upgrade.md` | The keyed HMAC-SHA-256 linear chain API: `ChainKey`, `genesis_head`, `chain_step`, `verify_tenant_chain`, `TenantChainEntry`, `export_tenant_anchor`, `verify_against_anchor`, `TenantChainAnchor`, `hex_encode_digest`, `parse_digest_hex`, `ChainError`, the constants. The tests consume ONLY this real public API |
| The CR-derived test list | P19.03 record §4 (this phase, `03_p19_03_iec_62443_4_2_component_mapping.md`) | CR 6.1 (audit log accessibility → P11.15 keyed chain, `<PENDING-P11.15 key signoff>`) and CR 2.1/CR 2.11 (authorization enforcement / never-granted set → P13.05 + conventions §6.4) drive which suites were written. Every evidence cell in that record stays `<PENDING-…>` — this batch WRITES tests and runs none, so it changes NO evidence cell |
| SBOM-freshness checks | P19.08 record §6, thresholds F-1..F-4 (`08_p19_08_sbom_generation_process.md`) | F-1 (artifact-digest currency) and F-3 (the registered 90-day advisory ceiling) are the two machine-checkable thresholds; the scripts implement exactly those, nothing more |
| Audit-run ordering | P19.09 record §5 (`09_p19_09_dependency_auditing_rollout.md`) | The runbook ordering is a PROCESS gate (SBOMs current per F-2 BEFORE the advisory scans); the freshness script deliberately does NOT code the ordering — a script cannot vouch for an ordering it did not observe (stated in the script header) |
| The P13.13 retention matrix | `../../phase-13-enterprise-identity-integration/records/25_p13_13_gdpr_privacy.md` §2/§4, plus `../../phase-13-enterprise-identity-integration/records/26_p13_14_jurisdiction_registry.md` | Retention ENFORCEMENT is design-only (deployment/adapter-owned; no enforcement module exists). The test file pins the CONFIGURATION CONTRACT via the existing P1.10 types and honestly records what enforcement module is expected |
| The P13 role/permission surfaces | `../../phase-13-enterprise-identity-integration/records/05_p13_05_role_model.md`; `crates/flexcore-authz` (`phase4_policies`, `phase13_policies`, `phase16_market_policies`, `policy`, `scope`, `action`, `decision`) | The additive role model (SiteApprover, apikey.*, market.*) as the grant-surface data an access review reconciles against |
| The P19.11 access-review process | `22_p19_11_access_review_process.md` (wave 1 Agent B) | Review scope rows (human principals per P13.05, machine principals/API keys per P13.08, adapter and market grants, never-granted set reviewed-never-altered); mechanics honestly PENDING-P13-RUNTIME — no review-evidence API exists and none is tested |

## 1. Deliverables written (all NEW, all WRITTEN-UNRUN)

| File | Anchors | Content in one line |
|---|---|---|
| `crates/flexcore-audit/tests/p19_17_audit_chain_verification.rs` | P11.15 design; P19.03 §4 CR 6.1 row | Integration-level chain-verification suite over the REAL `flexcore_audit::chain` + `flexcore_audit::record` API: linear chain commits to content/order/count (edit → `DigestMismatch` at the edited sequence; deletion/insertion/reorder → named anchor errors, with the verifier's gate ORDER respected — count before range); non-monotonic sequences refused; key material fails closed at 32 bytes; tenant namespaces cryptographically independent (no cross-tenant replay, wrong-namespace verification fails); anchors chain across windows via the continuation seed and a verifier walks the anchors WITHOUT the records; every anchor-field mismatch has its named error; digest hex round-trips and fails closed; the construction constants (`hmac-sha256-chain-v1`, version 1, 32-byte key minimum) are pinned; the honesty wording (`DEPLOYMENT REQUIREMENT`, `never a release signature`, `tamper-EVIDENCE, not tamper-PROOF`) is asserted VERBATIM in anchor text and tamper verdicts; decision/evidence/correction-bearing records chain and verify; the P0.09 ingest-after-event rule holds on every chain input; an empty entry set is pinned as absence-of-records, never evidence |
| `crates/flexcore-storage/tests/p19_17_retention_design_contract.rs` | P13.13 §2/§4; P13.14 §4; P0.16 §4 via the P1.10 types | The retention DESIGN-CONTRACT suite (see §3 for the honest scope statement): all seven §16.3 classes default `Unset` — no universal period in code; `command_audit_evidence` unset too (append-only governs writes, not deletion); `Unset` is the only state constructible without deployment input; a period appears ONLY where deployment configuration supplied one (no sibling fallback); partial/empty configuration documents leave unnamed classes unset through serde round trips; the fail-closed reading the P13.13 deployability rule consumes. The file's header records that enforcement is design-only and NO nonexistent enforcement module is referenced |
| `crates/flexcore-authz/tests/p19_17_access_review_data.rs` | P13.05; P19.11 §3 scope rows; conventions §6.4 | The grant-surface inventory an access review reconciles against, BEHAVIORAL over the public `evaluate`/`evaluate_action_id` entries and the single public claims path: the Phase 4 command matrix exactly as recorded (TenantAdmin approve at site/asset only; no tenant-level approve for anyone; PlatformAdmin/SiteApprover hold nothing in Phase 4); SiteApprover holds EXACTLY the approval duty (full 39-action × 5-role × 3-scope probe of the Phase 13 set's extent); apikey.* admin-roles-at-tenant-scope-only; the Phase 16 market surface with zero dispatch reach and zero SiteApprover grants; the four never-granted ids deny `UnknownAction` for every role across all three recorded sets; cross-tenant denies even for PlatformAdmin (`ScopeDenied`); missing scope/context deny with their named reasons; unknown ids deny before rules are consulted; every allow cites its phase rule reference (`policy:phase4` / `policy:phase13` / `policy:phase16-market`) — the auditable allow evidence |
| `security/sbom_freshness.py` | P19.08 §6 F-1/F-3; P19.09 §5; the P12.02 `snapshot_manifest.py` pattern | Stdlib-only verifier mirroring the P12 backup-tree script pattern (sorted paths, sha256, counted and NAMED problems, fail-closed exit codes 0/1/64/66). F-1: each `sbom` entry declares artifact + sha256; the verifier recomputes the artifact digest and requires the SBOM's mtime not to predate the artifact's — a changed artifact is F-1 STALE, counted. F-3: each `advisory-scan` entry declares `generated_at_unix_ms`; beyond the registered 90-day ceiling it is SCAN-STALE, never a clear. Declaration-vs-file: declared timestamps must agree with actual file mtimes within a settable tolerance (default 24 h). Fail closed on missing manifest, unknown format, missing/undeclared/malformed/duplicated/future-dated declarations. The clock is INJECTED (`--now-unix-ms` / `--now-iso`) — the tool refuses to invent a wall-clock now. It NEVER writes a manifest (that is the P19.08 §4/§5 machine-generated procedure's output, never hand-maintained) |
| `security/test_sbom_freshness.py` | same; the P12.02 companion-suite pattern | Complete stdlib `unittest` suite for the verifier: manifest structure and format gate; F-1 currency (matching digest+mtime verifies; changed artifact stale; newer-artifact stale; missing artifact/digest fails closed); F-3 window (within verifies, 91 days stale, tighter window configurable); declaration-vs-file mismatch and tolerance acceptance; fail-closed counting (undeclared, duplicate, self-referential, unknown kind, future-dated, no-entries-list); injected-clock requirement (exit 64 with no clock); CLI exit codes (0 clean / 1 counted / 64 usage / 66 not-a-directory); the ISO-now parser checked against a COMPUTED expected value, never a hand-transcribed epoch |
| `41_p19_17_compliance_test_suite.md` | — | This record |

No other file was created or modified. In particular NOT touched:
`ledger_phase_19.md`, `plan.md`, `context.md`, `docs/foundations/*`,
`docs/plan/*`, `.agent/*`, `deny.toml`, `security/README.md`,
`crates/flexcore-audit/src/chain.rs` (read, never edited), any existing
test file, and every Agent-B-owned file (records 21–25, 27, 61–69 and the
GDPR/IR/close deliverables).

## 2. Hard-ban compliance

| Ban | How honored |
|---|---|
| 1. No installation | Nothing installed — no cargo/pip/npm/docker invocation anywhere in the batch |
| 2. No build | Nothing built; the workspace was not compiled |
| 3. No tests run | All five deliverable files are WRITTEN-UNRUN; the only Python executions were the three PERMITTED verification tools (§7). `cargo test` has never run against these files — the first human build/test gate owns every result |
| 4. No placeholders | `grep -rn -i "TODO\|FIXME\|todo!\|unimplemented!"` over all five new files: **zero matches** (grep exit 1 = no match), run 2026-11-17 |
| 5. Permitted verification tools only | Exactly the three tools ran, once each, at close-out; outputs quoted VERBATIM in §7 |
| 6. No out-of-ownership edits | Listed explicitly above; no checkbox ticked anywhere; Agent B's files untouched |
| 7. No certification/attestation/scan/audit claim | The not-a-certification block opens this record; the test files' headers carry the same discipline; the freshness script's green exit text says "a statement about this check's coverage, never a certification" |
| 8. Never test nonexistent APIs | The chain tests reference only symbols verified present in `chain.rs`/`record.rs`; the authz tests only the public `PolicySet`/enum surface; the retention tests only the P1.10 types — the retention ENFORCEMENT layer does not exist and the file says so instead of faking it; the freshness scripts import nothing outside the stdlib |

## 3. Honest scope statements (what these suites can and cannot evidence)

1. **The audit-chain suite evidences DESIGN CONFORMANCE of code that has
   never run.** The chain is implemented code (P11.15), never compiled,
   keyed, deployed, or exercised; key management remains a HUMAN-GATED
   DEPLOYMENT REQUIREMENT and P11-E-1 stays OPEN (the human owner's). A
   future green `cargo test` run of this file would be evidence about the
   construction's behavior under TEST key material only — never about a
   production chain, and never a tamper-resistance guarantee: the
   construction is keyed tamper-EVIDENCE, and a superuser who can rewrite
   both the rows AND the anchor store still defeats detection (the
   residual risk the P11.15 record itself states).
2. **Retention enforcement is DESIGN-ONLY; the suite pins the contract,
   not an engine.** The P13.13 record §2/§4 and the P1.10 type's own
   header are binding: enforcement (partition drops, deletion jobs,
   backup purge, the post-restore erasure sweep) is deployment/adapter
   work that does not exist. The test file says this in its header and
   tests ONLY the documented configuration semantics (all-unset default,
   configuration as the sole source of a period, fail-closed unset).
   The P13.13 matrix's VALUE and BASIS cells stay EMPTY; the backup
   re-materialization caveat stays a REAL gap.
3. **The access-review suite characterizes grant-surface DATA for a
   review that has never run.** The P19.11 mechanics are
   PENDING-P13-RUNTIME; no principal, grant, or revocation exists. The
   suite pins what the review's M-2 reconciliation step would read (the
   recorded matrices and the structural never-granted refusals) — it is
   not review evidence and fills no §5 template cell.
4. **The freshness scripts check DECLARATIONS against FILES; they run
   no scanner.** The SBOM tree they verify does not exist yet (no SBOM
   has ever been generated — the P19.08 §1 posture). A future green run
   is a statement about the declared tree under the registered
   thresholds, never a scan result or a clear.
5. **Every producer evidence cell in the P19.03 §4 CR matrix stays
   `<PENDING-…>`.** Writing tests does not produce run evidence; the
   human gate does.

## 4. Deviations

1. **The retention design-contract suite lives in
   `crates/flexcore-storage/tests/`, not in the two test dirs the
   subtask enumerated.** Justification, recorded rather than silently
   applied: the P13.13 subtask directs the tests to pin the design's
   documented behavior "via the existing types", and the only existing
   retention types (`RetentionPolicy`/`RetentionSetting`) live in
   `flexcore-storage` (`crates/flexcore-storage/src/retention.rs`,
   `pub mod retention`). Testing them from `flexcore-audit` or
   `flexcore-authz` would require a NEW internal dependency edge
   (forbidden by this wave's rules and flagged by
   `tools/check_dependency_rule.py`). The file is additive; no manifest
   or source file was edited. The dependency-rule tool's §7 output below
   confirms the edge set is unchanged.
2. No other deviations from the subtask scope.

## 5. Finding handed to the human gate (observed, NOT edited)

While consuming `crates/flexcore-audit/src/chain.rs`, its in-module unit
test `the_canonical_encoding_covers_every_s10_1_field` (chain.rs line
~1268) assigns the `evidence` field as
`Some(RecordReference::new("evidence-1".to_string()) ...)`, but the
`AuditRecord.evidence` field type is `Option<EvidenceReference>`
(`crates/flexcore-audit/src/record.rs` line ~357 — a DISTINCT newtype
with its own `EmptyEvidenceReference` error). As written-unrun, this
looks like a latent compile defect in the chain's OWN test module, to be
caught at the first human build. This agent did NOT edit `chain.rs`
(outside its ownership: additive test files only) and asserts nothing
about the workspace's compile status. **Handoff: the first human
build/test gate and the P19.18 consistency audit should confirm.** This
suite's own integration file uses `EvidenceReference` for the evidence
field.

## 6. Honesty register (per conventions §7)

- No test, script, or suite in this batch has ever run; no pass/fail
  result exists; defects may surface at the first human run.
- The Rust test files were written against the sources read today; every
  referenced symbol was verified present in the source files, but the
  files have not been compiled — a compile error is a possible, honest
  outcome of the human gate.
- The freshness script's ISO parser is checked against a computed
  expected value in its own suite; the script itself is unrun.
- No SBOM, scan, review, key, anchor, erasure, or DSAR exists anywhere;
  no threshold application cell is filled; every evidence surface cited
  stays `<EMPTY>` / `<PENDING-…>` at its owning record.

## 7. Permitted verification tools — outputs quoted VERBATIM

Run 2026-11-17, repository root, once each (the only Python executions
in this batch):

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 29 workspace members.
EXIT=0
```

(The `ù` character in the third output is the tool's own verbatim console
rendering of its separator on this Windows console — quoted as printed,
matching the wave-1 close-outs' quoting discipline.)

## 8. Handoffs

- **Agent B (concurrent wave 2, records 61–69 + GDPR/IR/close):** the
  GDPR closure plan (P19.15) can cite
  `crates/flexcore-storage/tests/p19_17_retention_design_contract.rs` §3
  for the fail-closed retention-contract posture and this record §3.2 for
  the honest enforcement-is-design-only statement — by path only; B's
  files were not touched. The IR plan v2 and the phase close may cite the
  chain suite's honesty-wording assertions (§1 row 1) as the recorded
  language the human gate will see in verdict text.
- **The orchestrator / P19.18 consistency audit:** confirm (a) the five
  new files exist unrun, (b) the §4 deviation is accepted or rejected,
  (c) the §5 latent-defect finding against
  `crates/flexcore-audit/src/chain.rs`'s test module, (d) the ledger
  ticks for P19.17 apply with THIS record as the citation — never by
  this agent.
- **The human owner (the first build/test gate):** run, in whatever
  order the owner chooses — every order cell is theirs: `cargo test -p
  flexcore-audit`, `cargo test -p flexcore-storage`, `cargo test -p
  flexcore-authz`, and
  `python -I -m unittest discover -s security -p "test_sbom_freshness.py" -v`.
  The freshness scripts' first REAL use additionally requires an SBOM
  tree that does not exist yet (P19.08 §1 posture unchanged).

## 9. Status

**P19.17 BATCH COMPLETE — WRITTEN-UNRUN, AWAITING THE HUMAN
BUILD/TEST GATE.** Five deliverable files (two Rust integration suites,
one Rust design-contract suite, two Python scripts) plus this record;
nothing executed beyond the three permitted verification tools; no
checkbox ticked; no claim of certification, attestation, scan result,
audit outcome, or tamper-resistance anywhere.
