<!-- P10.28 — v1.0.0 tagging and release publication. PREPARED-NOT-EXECUTED
     (ENTRY_NOTE.md boundary 1). HONEST RECORD, stated plainly: v1.0.0 IS
     NOT RELEASED. No tag exists (verified: `git tag -l v1.0.0` returns
     EMPTY at writing time), no release exists, no artifact exists, and
     the loop bans (no commit, no tag, no push, no build, no install, no
     publication of any kind) PERMANENTLY forbid the agent from creating
     any of them. This record delivers the publication PROCEDURE (exact
     commands, described and never executed), the precondition chain
     quoted verbatim from 21_signed_artifacts.md §9 — ZERO links
     satisfied — and the publication authorization as a HUMAN GATE
     (UNSIGNED). Every execution, verification, and signature cell is
     EMPTY. Nothing here may be read as "the release is ready to
     publish" or "the release is published". -->
# P10.28 — v1.0.0 Tagging and Release Publication (PREPARED-NOT-EXECUTED — v1.0.0 IS NOT RELEASED; procedure recorded, nothing tagged, nothing published)

Date: 2026-11-14. Subtask: P10.28 (`ledger_phase_10.md` §P10.28). Owner
role: Platform engineering (with product). Report trace: §21.4; §22.3.
Inputs per the ledger: P10.27 go decision
(`ledger/phase-10-v1-release-hardening/27_go_no_go_decision.md` —
concurrent wave-5 sibling, cited by expected path; **does not exist at
this record's writing and no go decision exists**); P10.21 published
artifacts (`21_signed_artifacts.md` — **the artifact set is DESCRIBED,
nothing built, nothing signed, nothing published**); P10.26 release
notes (`26_release_notes.md` — concurrent sibling, cited by expected
path; **not written at this record's writing**). Deliverable: this
record only. No tag, no commit, no push, no release, no artifact, no
announcement was created — and none may be, by the loop bans recorded
in `ENTRY_NOTE.md` and in the task boundaries (no commit/tag/push, no
build, no install, no publication of any kind).

## 1. Honest status — the plain statement

**v1.0.0 IS NOT RELEASED.** As of 2026-11-14:

| State | Value | How verified |
|---|---|---|
| Does a `v1.0.0` git tag exist? | **NO** | `git tag -l v1.0.0` run in the writing loop: EMPTY output, 0 lines; `git tag -l` overall: no tags of any name |
| Does a published release exist? | **NO** | There is no distribution channel (P10.21 §8: channels UNDECIDED), no upload, no release record anywhere; nothing was published by any means |
| Does a release artifact exist? | **NO** | P10.21 §1/§4: no artifact of any §21.4 class exists; no checksum manifest exists |
| Does a release-candidate build exist? | **NO** | P10.18 §1: the candidate DOES NOT EXIST; the CI gate run is UNRUN |
| Does a go decision exist? | **NO** | P10.27 is a concurrent wave-5 sibling; its decision meeting has not been held; no decision record exists |
| Is a tag/release/artifact prepared for creation by this agent? | **NO — and PERMANENTLY FORBIDDEN** | The loop bans (ENTRY_NOTE; task boundaries) forbid commit, tag, push, build, and publication of any kind for this agent; P10.28 is recorded prepared-NOT-EXECUTED per ENTRY_NOTE.md boundary 1 |

The ledger's acceptance criteria (a)–(d) are **all UNMET and none is
claimed**: (a) the P10.27 decision is not go — it does not exist; (b) no
tag exists to match a candidate commit and no artifact hashes exist to
match a tag; (c) nothing is published, so nothing verifies from a
publication location; (d) no release notes are published (P10.26 not
yet written).

What this record delivers: the publication PROCEDURE (§3 — exact
commands, described and never executed), the precondition chain quoted
verbatim from `21_signed_artifacts.md` §9 with its status carried
honestly (§2 — **zero links satisfied**), the publication-authorization
gate (§4 — UNSIGNED), the external verification procedure (§5 — EMPTY),
and the announcement step with the limitations-visible rule (§6). The
ledger tick for P10.28 means "recorded, awaiting the human
tag/publish/verify" — the same honest convention as every prior phase.
**P10.28 stays prepared-NOT-EXECUTED; it does not become executed by
this record being written.**

## 2. The precondition chain — quoted verbatim from `21_signed_artifacts.md` §9

From `ledger/phase-10-v1-release-hardening/21_signed_artifacts.md` §9,
verbatim (that record owns the chain; this record carries it and
re-verifies its status, never re-decides it):

> Publication (P10.28) is legal only at the END of this chain, every
> link evidenced, no link waivable silently (a waiver is explicit,
> signed, reasoned, time-bounded — P10.27's rule):
>
> 1. **P10.18** — CI gates run on the gated commit; candidate built;
>    identity (commit/tag/build ID/hashes) recorded and filled into
>    P10.01 §6.1. *(EMPTY today — the candidate does not exist.)*
> 2. **P10.19** — SBOMs generated from the lockfiles; license audit run;
>    D4 closed and signed by its owners. *(NOT CLOSED — verdict recorded
>    there.)*
> 3. **P10.20** — final scan against the candidate artifacts and SBOMs;
>    every critical/high fixed or signed-time-bounded; zero untriaged.
>    *(NOT CLOSED.)*
> 4. **P10.14** — every §21.4 matrix cell tested or explicitly removed.
>    *(ALL cells EMPTY.)*
> 5. **P10.21 (this record)** — §4 checksums over the candidate
>    artifacts; §5 signing under the production DEPLOYMENT REQUIREMENT
>    (real key material, real verifier — the test-grade digests never
>    count); §6 support matrix assembled from filled cells only; §7
>    publication verification from the publication location.
> 6. **P10.26** — release notes with known limitations (including any
>    accepted vulnerabilities from P10.20 and removed matrix cells).
> 7. **P10.27** — the go/no-go decision says go.
> 8. **P10.28** — tag + publication, then external verification. Only
>    then does "published" become a true statement.
>
> **Today, zero links of this chain are satisfied.**

The closing sentence of the quoted chain is re-verified against each
owning record at this record's writing date (2026-11-14): link 1 EMPTY
(P10.18 §1), link 2 NOT CLOSED (P10.19 verdict), link 3 NOT CLOSED
(P10.20 verdict), link 4 ALL cells EMPTY (P10.14), link 5 nothing
checksummed or signed (P10.21 §1), links 6–7 records not yet written /
meeting not held (concurrent wave-5 siblings). **Zero links are
satisfied.** Until every link is filled by its human gate, the §3
procedure below may not be started, and any tag or publication created
before the chain is filled would be a protocol violation, not a
release.

## 3. The publication procedure — described, NEVER executed

The commands below are the procedure the human executor follows at
execution time, after §2's chain is fully filled. **None of them was
run in this loop; running any of them by the agent is permanently
forbidden.** The concrete distribution channels are UNDECIDED
(P10.21 §8 — recorded as such; the report names distribution FORMS, §21.4,
not hosted endpoints), so the upload step is written channel-shaped with
the channel names EMPTY; a channel decided later that differs from the
example shape changes the command, not the procedure's obligations
(record what was actually run).

### 3.1 Preconditions (all must hold before step 1)

1. The P10.27 decision is **go**, or **go with conditions** where every
   condition is met or explicitly carried as a post-release action with
   a named owner and a date (ledger step 1). A no-go forbids everything
   below (ledger step 5 of P10.27; CHECKLIST.md: "No publication on a
   no-go").
2. The P10.18 candidate identity is FILLED: the release-candidate
   commit hash, build ID, and artifact hashes (P10.18 §4; P10.01 §6.1
   no longer `<PENDING P10.18>`).
3. The P10.21 §4 checksum manifest (`checksums-v1.0.0.txt`) is FILLED
   and SIGNED per P10.21 §5 (production key material, production
   verifier — the test-grade FNV-1a-64 digests never count, P10.21 §5).
4. P10.26 release notes exist WITH the known-limitations section
   complete (every P9.27 register item present, §0.3 non-promises
   stated) and reviewed by product and security (P10.26 acceptance (d)).
5. The distribution channels are DECIDED and recorded (P10.21 §8's
   UNDECIDED cells filled).

### 3.2 Tagging (ledger step 2 — exact commands, never executed)

```sh
# 0. Preconditions §3.1 all hold. Otherwise STOP — no tag under any
#    circumstance before the chain of §2 is filled.
# 1. Confirm the commit to be tagged is the release-candidate commit
#    from the FILLED P10.18 §4 identity (the ledger's named risk: "a tag
#    pointing at a different commit than the tested candidate" — the
#    tag MUST point at the tested candidate commit, never at HEAD,
#    never at a different commit):
git rev-parse <candidate-commit-sha>   # verify the sha resolves
# 2. Create the annotated tag on THAT commit (Semantic Versioning:
#    v1.0.0):
git tag -a v1.0.0 <candidate-commit-sha> -m "Neryva FlexCore v1.0.0 (release candidate <build ID>)"
# 3. Record, in this record's §3.4 table: the tag name, the exact
#    commit sha, the candidate build ID, and the artifact hashes from
#    the signed checksums-v1.0.0.txt — they must agree (the ledger's
#    acceptance (b)).
# 4. Push the tag to the repository remote:
git push origin v1.0.0
# 5. Verify from the outside that the remote tag points at the same sha:
git ls-remote --tags origin v1.0.0
```

### 3.3 Publication (ledger steps 3–4 — upload, attach, verify; never executed)

```sh
# 0. Preconditions §3.1 all hold (including the DECIDED channels).
# 1. Publish through the recorded distribution channels (P10.21 §8,
#    filled): per artifact class of P10.21 §3 (A-1..A-10), the signed
#    OCI images to the recorded registry, the CLI release archives, the
#    Helm chart package, the SBOMs, and the signed checksum manifest.
#    The concrete commands are the decided channel's; e.g. a release-
#    archive channel shaped like (ILLUSTRATIVE SHAPE ONLY — the channel
#    is UNDECIDED until filled; do not run this command against any
#    host before a channel decision exists):
#      gh release create v1.0.0 --verify-tag \
#        --title "Neryva FlexCore v1.0.0" \
#        --notes-file <release-notes-v1.0.0.md> \
#        <artifact files per P10.21 §3, incl. checksums-v1.0.0.txt and the SBOMs>
#    The release notes (P10.26) are attached WITH the release, with the
#    known-limitations section intact — never a separate later edit,
#    never a softened copy.
# 2. The deployment guide (P10.24: docs/operations/deployment-upgrade-model.md
#    — re-issued at v1.0.0 per its own re-issue rule) and the site
#    integration test harness (P10.22: harness/site-integration/) are
#    published through the recorded channels with the release.
# 3. EXTERNAL VERIFICATION (ledger step 4; P10.21 §7) — from the
#    publication location, NOT the build directory:
#      a. download EVERY published artifact class from the publication
#         location;
#      b. re-compute sha256 per artifact and compare to the published,
#         signed checksum manifest;
#      c. verify signatures from the publication location with a
#         production verifier against the trusted signer-reference set;
#      d. confirm the attached release notes match the tag (the notes'
#         version, the limitations section, and the P10.27 conditions
#         they carry);
#      e. run the independent download-and-verify of one artifact per
#         class (the ledger's verification method) by a person who did
#         not produce the artifacts;
#      f. verify the edge update bundle on a test edge host per §18.3
#         (P10.21 §7 row 3) — the first real run of the verify-before-
#         stage path against a real signed bundle.
```

### 3.4 The publication record (EMPTY — filled only at execution)

| Item | Value |
|---|---|
| P10.27 decision (go / go with conditions / no-go) | `<EMPTY — the decision does not exist>` |
| Conditions carried as post-release actions (owner, date per condition) | `<EMPTY>` |
| Tag name | `<EMPTY — no tag exists>` |
| Tagged commit sha (must equal the P10.18 candidate commit) | `<EMPTY>` |
| Candidate build ID | `<EMPTY>` |
| Artifact hashes (per P10.21 §3 classes, from the signed manifest) | `<EMPTY>` |
| Distribution channels used (per P10.21 §8, decided) | `<EMPTY>` |
| Release notes version attached (limitations section included) | `<EMPTY>` |
| Deployment guide + harness versions published | `<EMPTY>` |
| External download-and-verify result per artifact class | `<EMPTY>` |
| Signature verification result from the publication location | `<EMPTY>` |
| §18.3 edge-host bundle verification result | `<EMPTY>` |
| Independent (non-producer) verifier name | `<EMPTY>` |
| Publication timestamp (UTC) | `<EMPTY>` |

## 4. Publication authorization — a HUMAN GATE (UNSIGNED)

**No agent authorizes a release.** The publication authorization is the
P10.27 go/no-go decision — a meeting of product, engineering, security,
power systems, and legal (P10.27 step 2), recorded and signed — plus
the human execution of §3 by the release engineer. Every authorization
cell below is EMPTY/UNSIGNED:

| Authorization item | Value |
|---|---|
| Go/no-go decision exists (P10.27) | **NO — the meeting has not been held; the record does not exist** |
| Decision signed by the chair (product) | `<EMPTY — UNSIGNED>` |
| Waivers (if any) explicit, signed, reasoned, time-bounded | `<EMPTY — none exist>` |
| Release engineer who executed §3 | `<EMPTY — nothing executed>` |
| Approval reference in any published edge update bundle (`approval_reference` — empty = refused, fail-closed, P10.21 §5) | `<EMPTY>` |

A tag or release created without the signed P10.27 go decision is an
unauthorized publication — the standing rule it violates is CHECKLIST.md's
"No publication on a no-go" and the §2 chain's "no link waivable
silently".

## 5. External verification record — EMPTY

The ledger's verification method ("independent download-and-verify of
the published artifacts against the recorded hashes") is §3.3 step 3;
its record is §3.4's verification rows — all `<EMPTY>`. Nothing is
published, so nothing is verified; there is no verification to
summarize, and none is claimed.

## 6. The announcement step (ledger step 5) — procedure, not performed

The release is announced through the agreed channel (the announcement
channel is part of the UNDECIDED channel set, P10.21 §8) with **the
known limitations visible — not buried**: the announcement carries the
release notes' known-limitations section in full (or links it directly
at its limitations anchor), the §0.3 non-promises relevant to the
release (P10.26 step 3), the control-authority position (operator-
approved by default only after the P10.15 qualification chain —
VERDICT: NOT QUALIFIED — and bounded automation post-1.0, not enabled),
and any P10.27 conditions carried as post-release actions. An
announcement that softens, summarizes away, or defers the limitations
section violates the ledger's named risk ("the release notes published
without the limitations section"). **No announcement exists; none was
drafted for sending; the channel is UNDECIDED.**

## 7. Findings recorded by this record

| # | Finding | Disposition |
|---|---|---|
| F-1 | v1.0.0 is NOT RELEASED: no tag exists (verified `git tag -l v1.0.0` → empty), no release exists, no artifact exists; the agent loop bans permanently forbid creating any of them | Binding honesty statement; ENTRY_NOTE boundary 1 |
| F-2 | Zero links of the publication precondition chain are satisfied (re-verified against the owning records at 2026-11-14) | §2, quoted verbatim from `21_signed_artifacts.md` §9 |
| F-3 | The publication authorization gate is UNSIGNED: no P10.27 decision exists, no decision meeting has been held | §4 |
| F-4 | The distribution channels and announcement channel are UNDECIDED (P10.21 §8) — the upload/announce steps are recorded channel-shaped with EMPTY channel cells, never against an invented endpoint | §3.3, §6 |
| F-5 | The tag MUST point at the filled P10.18 candidate commit, never HEAD — the ledger's named failure mode is guarded structurally by §3.2 step 1 | §3.2 |
| F-6 | The release notes attachment rule (limitations intact, no separate softened copy) and the edge-host §18.3 verification are structural publication obligations, not optional steps | §3.3 steps 1, 3f |

## 8. Reviewer-verification procedure (human)

1. Confirm §1's table: no tag, no release, no artifact, no candidate,
   no go decision exists; run `git tag -l` yourself — expect EMPTY.
2. Confirm §2 quotes `21_signed_artifacts.md` §9 verbatim (the eight
   links and the "Today, zero links of this chain are satisfied."
   sentence) and that the re-verification notes match the owning
   records' own verdicts — no link silently upgraded.
3. Confirm §3 contains exact commands marked never-executed, and that
   no command output, no timestamp, and no hash is invented anywhere;
   every §3.4 and §4 cell is `<EMPTY>`.
4. Confirm §4 states the human gate plainly and that no agent-approval
   path exists anywhere in this record.
5. Confirm §6's limitations-visible rule is present and absolute.
6. Confirm no sentence in this record may be read as "ready to publish"
   or "published" — the title, §1, and §12 verdict all carry the
   not-released statement.
7. Confirm the ledger tick wording is "recorded, awaiting the human
   tag/publish/verify" and this record's status is PREPARED-NOT-EXECUTED.

## 9. Filling commands (human, at execution time — the agent may NEVER run these)

```sh
# 0. STOP unless every precondition of §3.1 holds (the §2 chain filled,
#    the P10.27 go decision signed, channels decided). A no-go or a
#    missing link forbids everything below.
# 1. Tag per §3.2 on the FILLED candidate commit; record §3.4.
# 2. Publish per §3.3 through the DECIDED channels; attach the P10.26
#    release notes with the limitations intact; upload artifacts, SBOMs,
#    signed checksum manifest, deployment guide, harness.
# 3. External verification per §3.3 step 3; fill §3.4 from the ACTUAL
#    outputs (download times, recomputed hashes, signature results,
#    edge-host result, independent verifier name).
# 4. Announce per §6 with the limitations visible.
# 5. Registry verification after any edit to this record:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 10. Non-claims

No tag was created; no commit was made; nothing was pushed; no release,
artifact, checksum, signature, SBOM, announcement, or publication of any
kind exists or was produced; no channel was selected or used; no
verification ran because there is nothing to verify; no authorization
was signed; no "ready to publish" or "published" state is claimed or
implied anywhere. The v1.0.0 release ladder row ("Pilot-ready;
operator-approved control by default") remains a TARGET position — this
record does not move it.

## 11. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| `git tag -l v1.0.0` (state check, read-only) | RUN in the writing loop — EMPTY, confirming no tag exists |
| Tagging / publication / external verification / announcement (§3–§6) | **NOT RUN — permanently banned for the agent; human-gate inputs; every cell EMPTY** |

## 12. Status

**WRITTEN-UNRUN / PREPARED-NOT-EXECUTED.** The publication procedure,
the precondition chain, the authorization gate, and the verification
steps are recorded with their honesty bounds carried from the owning
records. **v1.0.0 IS NOT RELEASED**, the chain has zero satisfied
links, and the record stays prepared — the tag, the release, and the
publication are human gates that have not happened and are not
scheduled by anything in this repository.
