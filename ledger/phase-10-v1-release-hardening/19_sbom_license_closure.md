<!-- P10.19 — SBOM and license audit closure (D4). HONEST RECORD: the D4
     re-closure for the v1.0.0 release artifact set is ASSEMBLED BY CITATION
     and the verdict is NOT CLOSED — no SBOM exists (no Cargo.lock, no
     console lockfile, no resolved Python environment), no license scan has
     ever run, and no legal review has happened. The license-audit surface
     is cited (P8.03 §2 pin table; P6.02 audit; deny.toml P6.27), never
     restated as new research. deny.toml ALREADY EXISTS (P6.27) and is cited
     here, NOT recreated and NOT modified — it is WRITTEN-NEVER-RUN. The
     SBOM generation procedure is recorded with exact commands; every
     execution is a HUMAN GATE and every result cell is EMPTY. The
     LLVM-exception allowlist question is carried as a RECORDED DECISION
     with DEFAULT/RECOMMENDATION — never closed by an agent. The §22.2
     "license/security review" CI gate (P10.18 G14/G15) is the automated
     scan only; the legal D4 determination is THIS record's, and it does
     not close here. -->
# P10.19 — SBOM and License Audit Closure — D4 (assembled by citation; verdict NOT CLOSED — no SBOM exists, no scan has run)

Date: 2026-11-14. Subtask: P10.19 (`ledger_phase_10.md` §P10.19).
Owner role: Legal + engineering (the D4 owners per `.agent/DECISIONS.md`).
Report trace: §22.2 (SBOM generation; license/security review); §22.3;
§25.2 (D4); §9.3 (SBOMs for releases and plugin packages).
Inputs: P10.01 scope freeze
(`ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md`); P10.18
CI-gate/candidate record
(`ledger/phase-10-v1-release-hardening/18_ci_gate_run_record.md` — the
supply-chain CI steps this record consumes, all UNRUN); the D4 pin table
(`ledger/phase-08-advanced-coordination/03_research_license_audit.md` §2);
the Rust-side license audit
(`ledger/phase-06-edge-protocol/02_license_audit.md`); the deny.toml policy
file (`deny.toml`, P6.27 — pre-existing); `.agent/PROVENANCE.md`
(read-only — the third-party-copy audit surface); `.agent/DECISIONS.md`
(D4 OPEN).

## 1. Honest status — the plain statement

**D4 is NOT CLOSED for the v1.0.0 release artifact set, and this record
does not close it.** Concretely, on 2026-11-14:

- **No SBOM exists.** The workspace has no committed `Cargo.lock`
  (verified 2026-11-14: the file does not exist), the console has no
  lockfile (P10.14 record), and the Python research/worker stacks have no
  resolved, installed environment (P8.03 §1: nothing installed, no venv).
  An SBOM generated from a manifest instead of a lockfile is the ledger's
  own named failure mode — none is generated here.
- **No license scan has ever run.** No `cargo deny check`, no ORT or
  equivalent scanner, no advisory database query — all banned in this
  loop (ENTRY_NOTE.md) and all human gates.
- **No legal review has happened.** The D4 signature cells in §7 are
  EMPTY. An agent cannot sign a legal determination.

What this record delivers: the D4 re-closure status assembled from the
carried audit surface (§3), the deny.toml disposition (§4 — exists,
cited, never run), the LLVM-exception allowlist RECORDED DECISION (§5),
the SBOM generation procedure with exact commands (§6), the
completeness-verification procedure (§7), and the EMPTY closure record
the human D4 owners fill (§7/§8). Ticking P10.19 in the ledger means
"implemented, awaiting human scan/SBOM generation/legal review" — the
same honest convention as every prior phase.

## 2. What D4 requires at this phase — and what P10.19 is

The ledger header records the binding framing: **D4 was gated before
v0.1.0; its final closure for the v1.0.0 release artifact set is here —
P10.19 is a re-closure for this artifact set, not the first D4
closure.** `.agent/DECISIONS.md` records D4 as SCHEDULED/OPEN with the
core-license recommendation (Apache-2.0) explicitly not a decision.
Nothing in this record changes either state: **D4 remains OPEN** and the
recommended-default status of Apache-2.0 is carried unchanged.

The D4 audit scope is broader than code: §25.2 names protocol stacks,
solvers, **models, and datasets**. The models/datasets leg has NO audit
surface yet — no model or dataset artifact has ever been produced,
imported, or licensed in this repository's history (the research-stack
records, P8.03, cover the *planned* dependencies; there is no dataset
inventory to audit). This gap is a finding (F-3, §8), not something this
record papers over.

## 3. The carried license-audit surface — cited, not restated

| Audit surface | Location | What it covers | Status as recorded there |
|---|---|---|---|
| Rust workspace direct-dependency audit | `ledger/phase-06-edge-protocol/02_license_audit.md` | The permissive families of the direct workspace set (MIT; MIT/Apache-2.0; Apache-2.0; ISC family) | Recorded; transitive set UNVERIFIED until the first lockfile-backed scan |
| `deny.toml` policy configuration | `deny.toml` (P6.27) | The cargo-deny `[licenses]` allow-list, `[advisories]`, `[bans]`, `[sources]` | WRITTEN-NEVER-RUN (the file's own header; no Cargo.lock existed when it was written — still true) |
| Research-stack pin table (the D4 follow-through) | `ledger/phase-08-advanced-coordination/03_research_license_audit.md` §2 | ANDES, HELICS, pandapower, PyTorch/PyG (DECISION-POINT), JAX (unselected), Ray/RLlib, Gymnasium, grpcio, protobuf, numpy/scipy, pytest | STATIC audit; every license cell UNVERIFIED until first install; D4 remains OPEN (that record's own §1) |
| Copyleft isolation design | `03_research_license_audit.md` §3 | ANDES GPLv3 isolation (worker-boundary only; outputs as data; never linked) | Recorded; legal confirmation is a named gate (P8.01 G-4) — not granted |
| Copyleft posture of the cargo tree | `deny.toml` `[licenses]` | No GPL/AGPL/EPL family allowed; Ipopt (EPL) the recorded expected-but-absent example | Configured, never scanned |
| Third-party-copy audit (agent harness) | `.agent/PROVENANCE.md` | No third-party skill/document/code copied without a license check; the Anthropic skills rejection recorded | Checked by citation on 2026-11-14 (read-only surface; ledger P10.19 step 6) — this is the ONE static check this record can honestly make |

The §2 pin table is the **SBOM input list for the research stack**
(P8.03 §7): when the research environment is first resolved, the SBOM
must reconcile against it — a dependency present in the environment but
absent from that table (or vice versa) is a finding owned by Legal +
research lead. That reconciliation has NOT happened (no environment
exists) and is carried as part of §7 below.

## 4. The deny.toml policy file — EXISTS (P6.27); cited, NOT recreated, NOT modified

`deny.toml` exists at the repository root, written by P6.27 as the
backing configuration for the `edge-license-gate` CI job. The ledger's
P10.19 step does not dictate a second policy file, and P8.03 §6 already
recorded the "recording over editing" decision for it. Therefore:

- **No new deny.toml was created and no existing entry was changed.**
  The file is cited as the D4 cargo-deny policy, in the state P6.27
  wrote and P8.03 §6 reviewed.
- The file is **WRITTEN-NEVER-RUN** — its own header says so, and no
  `cargo deny` invocation has happened since (still banned in this
  loop). No verdict of any kind is derived from it.
- Its key recorded postures, carried by citation: `unlicensed = "deny"`
  (fail closed on a dependency with no license entry); copyleft
  (GPL/AGPL/EPL) intentionally NOT allowed; `yanked = "deny"` with an
  empty ignore list (advisories are never silenced without a dated,
  owned record); the three carried D4 open items (rustls/rustls-pemfile
  RE-VERIFY; OpenEMS license UNVERIFIED/FLAG — REST-API boundary, not a
  crates dependency; transitive-set verification at the first scan).
- **Every allow/block entry must be traceable to the license audit** at
  the human run. The traceability check is §7 step 4; the entries whose
  audit trace is weakest today are flagged in §5/§7 (the "common
  permissive transitive set" entries — BSD-2-Clause, Zlib, BSL-1.0,
  CC0-1.0, CDLA-Permissive-2.0, MPL-2.0, OpenSSL — are labelled
  UNVERIFIED by the file's own comment until the first lockfile-backed
  run).

## 5. RECORDED DECISION — the LLVM-exception allowlist question (carried, NOT closed)

The `deny.toml` `[licenses]` allow-list contains
`"Apache-2.0 WITH LLVM-exception"`. The question P10.19 must carry
(ENTRY_NOTE.md boundary 6: "the D4 cargo-deny allowlist question stays a
recorded decision point") is whether that allow entry stands for the
v1.0.0 release artifact set.

| Item | Recorded content |
|---|---|
| The question | Is `Apache-2.0 WITH LLVM-exception` an acceptable license family for code in the v1.0.0 release artifact set (and its plugin-package SBOMs)? |
| Why the question exists | The entry is in the P6.27 config's "common permissive transitive set" — recorded so the first scan does not false-fail, NOT as a legal determination. The family is common in the Rust ecosystem for generated/static-table crates, but **no specific workspace dependency carrying this SPDX expression has ever been resolved** (no lockfile exists), so the entry has no confirmed in-tree referent. |
| Compatibility note (labelled a recommendation basis, not a finding) | The LLVM-exception clause is designed to permit linking code that would otherwise be GPL-family-incompatible under the Apache-2.0/GPL linking interaction; it is broadly treated as Apache-2.0-compatible in ecosystem practice. **This is recorded context, not a legal determination** — the determination is the D4 owners'. |
| DEFAULT (recorded, not decided) | Keep the entry as P6.27 wrote it (fail-closed posture unchanged; removing it before any scan would guarantee a false failure with no compensating benefit). |
| RECOMMENDATION | The human D4 owners (Legal + engineering) confirm the entry at the first lockfile-backed `cargo deny check licenses` run, together with the P6.27 header's RE-VERIFY items — at that point every allowed family maps to an actual resolved dependency and the traceability check of §7 step 4 becomes executable. If legal confirms, the RECORDED DECISION is closed in `.agent/DECISIONS.md` by its owners; if not, the offending dependency gets a disposition (replace / isolate / comply / escalate) per the ledger's step 3. |
| Status | **RECORDED DECISION CARRIED — never closed by an agent.** No agent action in this loop resolved it; no scan ran that would inform it. |

## 6. SBOM generation procedure — exact commands, NEVER RUN (human gate)

Formats: §9.3 requires SBOMs for releases and plugin packages in a
machine-readable format (SPDX or CycloneDX); the exact format versions
are verified at time of use per the phase RESEARCH.md tooling notes —
no version is asserted here. Every SBOM is generated **from lockfiles /
resolved environments, never from a hand-written list** (the ledger's
named risk). All commands below are the human's; none was executed in
this loop.

```sh
# 0. Preconditions — every one must hold before any SBOM is meaningful:
#    - a committed Cargo.lock exists (candidate build, P10.18 §3 step 7)
#    - the console lockfile exists (the recorded P5.28 decision: npm ci once one lands)
#    - the Python worker/research environments are resolved (scratch venv,
#      P8.03 §8 method — never committed)
#    - the candidate identifier (commit/tag/build ID) is recorded (P10.18 §4)

# 1. Rust workspace SBOM (the G15 CI step, P10.18 §2):
cargo cyclonedx --all --format json
#    -> one SBOM per workspace crate; the RELEASE SBOM is the set for the
#       released apps/binaries, reconciled against Cargo.lock (§7).

# 2. Container SBOMs (the G19 CI step):
syft <each released OCI image> -o spdx-json > sbom/<image>.spdx.json

# 3. Python worker/research SBOM — from the RESOLVED environment:
pip freeze                                  # recorded next to the SBOM
pip-audit --format cyclonedx-json > sbom/python-workers.cdx.json
#    (or the equivalent resolved-environment SBOM generator; the tool and
#     version actually used are recorded — the ledger's unnamed-tool rule)

# 4. Console SBOM — only after the console lockfile exists:
cd console && npm ci && npx @cyclonedx/cyclonedx-npm --output-file ../sbom/console.cdx.json

# 5. Plugin-package SBOMs (§9.3): one SBOM per plugin package produced by
#    the plugin packaging step of the release build, same tooling.

# 6. License scan over the resolved tree (the G14 gate; the automated half ONLY):
cargo deny check licenses        # the deny.toml policy — see §4/§5
#    plus the ORT-or-equivalent scanner the D4 owners select at run time
#    (the ledger names ORT as an example; the tool + version actually used
#     are recorded, never assumed).
```

## 7. SBOM completeness verification + the D4 closure record — EMPTY (human gate inputs)

The ledger's steps 4–6 and acceptance (a)–(e), as an EMPTY record. No
cell is pre-filled; there is no SBOM, no scan output, and no legal
review to fill them from.

| # | Closure item | Required evidence | Status / value |
|---|---|---|---|
| C-1 | Release SBOMs exist, generated from lockfiles | SBOM file locations (release, per-container, Python, console, plugin packages) | `<EMPTY — no SBOM exists>` |
| C-2 | SBOM completeness spot-check | Every direct dependency appears; a sample of transitive dependencies appears; diffed against `Cargo.lock` / `npm` lockfile / `pip freeze` | `<EMPTY — procedure below>` |
| C-3 | Research-stack SBOM reconciliation | Diff against the P8.03 §2 table; any unlisted dependency is a finding (owner: Legal + research lead) | `<EMPTY — no environment exists>` |
| C-4 | License determination per dependency | Scanner tool + version actually used; per-dependency determinations | `<EMPTY — no scan has run>` |
| C-5 | Incompatible/unclear licenses have dispositions | Per finding: replace / isolate / comply / escalate-to-legal; no incompatible dependency ships silently | `<EMPTY — zero findings recorded, because zero scans ran — this is NOT a zero-findings claim>` |
| C-6 | deny.toml allowlist traceability | Each allow entry mapped to an audit row or a resolved dependency; untraceable entries dispositioned | `<EMPTY — §5 carries the one flagged question>` |
| C-7 | Models/datasets license audit | The D4 scope's models-and-datasets leg | `<EMPTY — no model/dataset artifact exists to audit (finding F-3)>` |
| C-8 | Legal review | Reviewer, date, scope | `<EMPTY — UNSIGNED>` |
| C-9 | D4 closure signature | The D4 owners (Legal + engineering) sign | `<EMPTY — UNSIGNED>` |

**Completeness-verification procedure (the human steps for C-2/C-3):**
(1) `cargo metadata --locked` (or the SBOM tool's dependency graph) and
diff the direct set against the P6.02 audit rows — any unlisted direct
dependency fails the check; (2) sample transitive dependencies from
`Cargo.lock` and confirm presence in the SBOM; (3) `pip freeze` diffed
against the P8.03 §2 table; (4) map every `deny.toml` allow entry to a
resolved dependency or an audit row — an entry with no referent is a
finding, not an error; (5) file the diff outputs next to the SBOMs as
the closure evidence.

## 8. Findings recorded by this record

| # | Finding | Disposition |
|---|---|---|
| F-1 | No SBOM of any kind exists (no Cargo.lock, no console lockfile, no resolved Python environment); every SBOM cell in §7 is EMPTY by honest state | Binding honesty statement; the procedure of §6 is the fill path |
| F-2 | No license scan has ever run (`cargo deny check` never executed; deny.toml WRITTEN-NEVER-RUN); zero determinations exist — the empty C-4 cell is NOT a clean result | §4/§7; first human scan is the verification point |
| F-3 | The D4 scope's **models and datasets** leg has no audit surface at all — no dataset/model inventory exists in the repository, and no record covers it | Carried to the human D4 closure; a go decision (P10.27) cannot claim D4 closed while C-7 is EMPTY |
| F-4 | The LLVM-exception allowlist entry has no confirmed in-tree referent (no lockfile) — carried as a RECORDED DECISION with DEFAULT/RECOMMENDATION, never closed by an agent | §5 |
| F-5 | deny.toml already exists (P6.27); this record creates nothing and modifies nothing — recording over editing, the P8.03 §6 precedent | §4 |
| F-6 | The P10.18 G14 `security-and-supply-chain` cargo-deny step carries `continue-on-error: true` (pre-deny.toml artifact); the FAILING gate is `edge-license-gate` — the automated CI half and the legal D4 determination are distinct, and neither closes the other | Cited from P10.18 §2 G14 / §6; carried to the human run |
| F-7 | The P8.36 advisory surface (NOT-EXECUTED table) is adjacent to but distinct from this record's license scope — it is P10.20's input | Handled in `20_vuln_scan_closure.md`; not duplicated here |

## 9. Reviewer-verification procedure (human)

1. Confirm §1/§7: no SBOM location, no scan result, no determination, no
   legal signature is claimed anywhere; every C-cell is `<EMPTY>`/UNSIGNED.
2. Confirm `deny.toml` matches the P6.27-written state (no edit by this
   record): diff against the phase-6 ledger record of the file; confirm
   this record adds no `[licenses]` entry and creates no second policy file.
3. Confirm no `Cargo.lock` exists (`ls Cargo.lock`) — the §1 statement.
4. Confirm the D4 citations resolve: P8.03 §2/§3/§6/§7, P6.02, P6.27
   header, `.agent/DECISIONS.md` D4 row (SCHEDULED/OPEN), P10.18 §2 G14/G15.
5. Confirm §5 reads as a carried decision with DEFAULT/RECOMMENDATION —
   not a resolution.
6. Confirm the ledger tick reads "implemented, awaiting human
   scan/SBOM generation/legal review" and the verdict line (§10) is
   NOT CLOSED.

## 10. Closure verdict

**D4 for the v1.0.0 release artifact set: NOT CLOSED.** The audit
surface is assembled and the procedure is recorded; the SBOMs, the
scan, the determinations, the dispositions, and the legal signature do
not exist. Per the ledger's step 5, if D4 cannot close at the human
gate, the release does not go (P10.27) — that decision is the meeting's,
not this file's.

## 11. Filling commands (human, at execution time)

```sh
# After P10.18's build lands the lockfiles and artifacts:
cargo cyclonedx --all --format json
cargo deny check licenses
# ... the §6 procedure, then fill §7 C-1..C-9 from the actual outputs,
#     update .agent/DECISIONS.md (D4 owners only), and record the tool
#     + version actually used for every scan.

# Registry verification after any edit to this record:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 12. Status

**WRITTEN-UNRUN / ASSEMBLED-BY-CITATION.** The D4 re-closure for the
v1.0.0 artifact set is recorded as OPEN with an EMPTY closure table, an
existing never-run policy file, and a human-gate procedure. No SBOM
exists; no scan has run; no legal determination was made or claimed.
