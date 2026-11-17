<!-- P19.08 — SBOM generation process. HONEST RECORD: a PROCEDURE document
     extending the P10.19 §6 commands into a standing process (per-image
     CycloneDX, storage next to artifacts, freshness policy). NO SBOM is
     generated here (no lockfile exists — the P10.19 §1 posture stands
     unchanged), no tool is installed or run, and no freshness result
     exists. Thresholds registered (X-8) before any run. The
     not-a-certification discipline of P10.25 §7 (§9) carries verbatim. -->
# P19.08 — SBOM Generation Process (per-image CycloneDX procedure; storage next to artifacts; freshness policy — procedures only, never run here)

Date: 2026-11-17. Subtask: P19.08 (`ledger_phase_19.md` §P19.08).
Plan trace: `../plan.md` §2 row P19.08, §3 blocking order row 6 (consumed
by P19.09 — the audit configs scan what the SBOM describes — and by
P19.06's intake format, both by path). Sources: the P10.19 SBOM record §6
commands (`ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md`
— cited, extended, never rewritten); [S-79] (Sigstore research surface,
which covered SBOM in SPDX/CycloneDX); the P19.07 provenance design
(`07_p19_07_slsa_build_provenance_plan.md`).

## 1. Honest status — the plain statement

**No SBOM exists, and this record generates none.** The P10.19 §1 posture
stands verbatim in substance: no committed `Cargo.lock`, no console
lockfile, no resolved Python environment — an SBOM from a manifest instead
of a lockfile is the ledger's named failure mode, and nothing here commits
it. The first human SBOM generation is the gate; every result cell below
is EMPTY. What this record adds is the STANDING PROCESS around the P10.19
§6 commands: per-image CycloneDX generation (§4), storage layout (§5), and
a freshness policy with registered thresholds (§6). The P10.19 record's
§6 commands remain the D4 closure procedure — this record extends, never
replaces, and the D4 verdict is untouched.

## 2. Formats — CycloneDX primary, machine-generated only

- Primary format: **CycloneDX (JSON)**, per the plan row. SPDX remains
  accepted where a consumer requires it (the P10.19 §6 `syft -o spdx-json`
  command is retained for that path). The exact format VERSION emitted is
  recorded at generation time from the tool's own output — never asserted
  here (the unnamed-tool rule).
- Machine-generated ONLY, from lockfiles / resolved environments /
  built images — never hand-written, never maintained as a document
  (the P10.19 rule, carried verbatim in substance).

## 3. Preconditions (all must hold; carried from P10.19 §6 step 0)

1. A committed `Cargo.lock` exists (candidate build).
2. The console lockfile exists.
3. The Python worker/research environments are resolved (scratch venv —
   never committed).
4. The candidate identifier (commit/tag/build ID) is recorded (P10.18 §4
   — `<PENDING P10.18>` today).
5. The P19.07 provenance covers the same artifacts (binding §4 outputs to
   the artifact digests — by path).

Today: preconditions 1–3 UNMET, 4 PENDING. No SBOM step runs until all
five hold — the precondition list is the enforcement, as in P10.20 §3
step 0.

## 4. The procedure — exact command shapes, NEVER RUN (human gate)

Extends the P10.19 §6 commands with the per-image rule. The tool NAMES are
the ones the repository's records and CI already name (Syft-class
generators, `cargo cyclonedx`, cyclonedx-npm, pip-audit's CycloneDX
output); the actual tool + version used is recorded at run time.

```sh
# 1. Rust workspace (component-level, from the lockfile):
cargo cyclonedx --all --format json
#    -> per-crate SBOMs; the release SBOM set is the released apps/binaries,
#       reconciled against Cargo.lock (the P10.19 §7 completeness procedure).

# 2. Per-image CycloneDX (THE per-image rule this record adds):
#    one SBOM PER RELEASED OCI IMAGE — never one SBOM for the "release":
syft <image-ref> -o cyclonedx-json > sbom/<candidate-id>/<image-name>.cdx.json
#    per image of the P10.21 §3 classes: control plane, plugin host,
#    worker images, edge agent, simulators. An image without an SBOM is
#    not publishable under this process (the publication precondition).

# 3. Python environments (from the RESOLVED environment):
pip freeze                                   # recorded next to the SBOM
pip-audit --format cyclonedx-json            # the resolved env, not requirements

# 4. Console (only once the lockfile exists):
cd console && npm ci && npx @cyclonedx/cyclonedx-npm

# 5. Plugin packages: one SBOM per plugin package, same tooling (§9.3 rule).

# 6. Digest binding (P19.07 interlock): the SBOM set's artifact digests
#    must match the provenance-covered digests for the same candidate —
#    a mismatch is a finding, not a tolerance issue.
```

## 5. Storage — next to the artifacts

| Rule | Content |
|---|---|
| S-1 | SBOMs are stored NEXT TO the artifacts they describe, keyed by candidate identifier: `sbom/<candidate-id>/…` beside the artifact store of the same candidate — a SBOM detached from its artifact identity is orphan evidence and does not count |
| S-2 | Each SBOM file carries, in its record (not hand-edited content): the candidate id, the artifact digest it binds to, the generator tool + version, and the generation timestamp — captured from the tool output |
| S-3 | SBOMs are checksummed like any artifact (the P10.21 §3 A-8 row already requires this — cited) |
| S-4 | The storage location/registry is the publication channel decided at P10.28 preparation — UNDECIDED today (P10.21 §8, cited); this record pre-commits the layout, not the channel |
| S-5 | SBOMs published with a release are published unmodified from the generation output — no post-hoc editing of SBOM content (a correction regenerates the SBOM) |

## 6. Freshness policy — thresholds registered (X-8) before any run

| Threshold | Registered value | Consequence |
|---|---|---|
| F-1 Build freshness | An SBOM is CURRENT only for the exact artifact digest it was generated from. Any artifact change (a new candidate id / digest) makes every prior SBOM STALE | Stale SBOMs are evidence for the old artifact only; a release publishes only current SBOMs |
| F-2 Regeneration rule | On any re-pin, patch bump, or lockfile regeneration (the P10.20 §4 trigger rule), the affected SBOM set regenerates BEFORE the advisory scans run — the scans consume the current SBOMs, never stale ones | Enforced by the P19.09 runbook ordering |
| F-3 Advisory-window freshness | An SBOM older than 90 days accompanying an UNCHANGED artifact does not block publication, but any advisory scan used in a go decision must be younger than 90 days (registered; the value is the pre-registered ceiling, revisable by a reviewed, dated change BEFORE the first go decision that relies on it) | Scans older than the ceiling are marked SCAN-STALE — never a clear (the P10.20 §8 coverage-honesty rule) |
| F-4 Supplier intake freshness | Supplier SBOMs follow the P19.06 §6 intake-validity threshold (cited by path — not re-registered here) | Per P19.06 |

No freshness RESULT exists: every F-row's application cell is
`<EMPTY — no SBOM has ever been generated>`.

## 7. Relationship to the frozen P10.19 record

- The P10.19 §6 commands remain the D4-closure procedure; this record's
  §4 EXTENDS them (the per-image rule, the digest binding, the storage
  layout) without rewriting that file.
- The P10.19 §7 completeness-verification procedure (C-2/C-3 steps) is
  unchanged and still owns completeness checking.
- The D4 verdict (NOT CLOSED) and the LLVM-exception RECORDED DECISION
  are untouched — cited, never re-decided.

## 8. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

An SBOM is an inventory artifact — evidence a process consumes, never a
certification, security claim, or completeness guarantee.

## 9. Reviewer-verification procedure (human)

1. Confirm §1: no SBOM exists and none was generated; no tool ran.
2. Confirm §3: the preconditions are recorded as unmet/pending, not
   assumed.
3. Confirm §6: thresholds are registered with EMPTY application cells;
   F-3's 90-day value is labeled pre-registered-ceiling.
4. Confirm §7: the P10.19 file is unchanged by this record (diff against
   its §6 commands).
5. Confirm §8's verbatim block matches `AGENTS.md` §3 rules 2–3 exactly.

## 10. Status

**PROCEDURE-ONLY / WRITTEN / NEVER-RUN.** The standing process exists;
the SBOMs, the runs, and every result cell do not.
