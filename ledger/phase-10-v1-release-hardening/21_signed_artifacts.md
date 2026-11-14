<!-- P10.21 — Signed artifacts, checksums, and support matrix. HONEST
     RECORD: nothing is published, nothing is signed, no checksum table
     is filled. The artifact set v1.0.0 WOULD publish is DESCRIBED from
     the §21.4 build/distribution table (cited via the P10.14 record);
     the checksum procedure is sha256sum over the P10.18-built
     artifacts (human gate, EMPTY); the signing procedure carries the
     production-signing DEPLOYMENT REQUIREMENT honestly — NO production
     key material exists, the code's default verifier refuses
     everything (NoUpdateVerifier), and the test-grade FNV-1a-64
     digests in crates/edge-agent/src/update_verification.rs are NOT
     release signatures. The support matrix is LINKED to P10.14
     (console/SUPPORT_MATRIX.md) whose result cells are ALL EMPTY —
     nothing is listed as supported. The release-publication
     precondition chain is recorded; P10.28 stays
     prepared-NOT-EXECUTED (ENTRY_NOTE boundary 1). -->
# P10.21 — Signed Artifacts, Checksums, and Support Matrix (described artifact set + EMPTY checksum/signature/verification record — nothing is published or signed)

Date: 2026-11-14. Subtask: P10.21 (`ledger_phase_10.md` §P10.21).
Owner role: Platform engineering. Report trace: §21.4 (every release
publishes checksums, an SBOM, and a support matrix); §9.3
(signatures/provenance for production plugin installation); §18.3
(signed and verified update bundles). Inputs: P10.18 candidate record
(`ledger/phase-10-v1-release-hardening/18_ci_gate_run_record.md` — the
build + candidate-identity procedures this record's checksums consume;
the candidate DOES NOT EXIST); P10.19 SBOM record
(`ledger/phase-10-v1-release-hardening/19_sbom_license_closure.md` —
SBOM locations EMPTY); P10.14 matrix record
(`ledger/phase-10-v1-release-hardening/14_browser_os_matrix.md` +
`console/SUPPORT_MATRIX.md` — every result cell EMPTY); the signing
precedents `crates/edge-agent/src/update_verification.rs` (P9.34) and
`crates/flexcore-research/src/worker_identity.rs` (P8.33 — identity
material as deployment-held references); `ledger/phase-06-edge-protocol/
34_secure_boot_position.md` (the signed-update position).

## 1. Honest status — the plain statement

**No v1.0.0 artifact exists, so nothing is checksummed, nothing is
signed, and nothing is published.** The release candidate is a
described procedure, never a built artifact (P10.18 §1; ENTRY_NOTE
boundary 1); P10.28 (tagging/publication) is prepared-NOT-EXECUTED and
stays that way. Every checksum cell (§4), signature cell (§5), and
publication-verification cell (§7) in this record is EMPTY. The
support matrix (§6) is LINKED, not filled — P10.14's result cells are
all EMPTY, so no cell of the published matrix may claim support.

What this record delivers: the artifact set v1.0.0 WOULD publish (§3,
described from the §21.4 table), the checksum procedure (§4), the
signing procedure with the production-signing DEPLOYMENT REQUIREMENT
carried honestly (§5), the support-matrix linkage (§6), the
distribution channels and support commitment (§8 — recorded as
UNDECIDED where the report is silent), the publication-verification
procedure (§7), and the release-publication precondition chain (§9).
Ticking P10.21 in the ledger means "implemented, awaiting human
build/checksum/sign/publish" — the same honest convention as every
prior phase.

## 2. What this subtask consumes — and what does not exist yet

The ledger's inputs are the P10.18 candidate, the P10.19 SBOMs, and
the P10.14 matrix results. Status: candidate identity EMPTY (P10.18
§4), SBOM locations EMPTY (P10.19 §7 C-1), matrix result cells EMPTY
(P10.14 §5). This record is therefore a **procedure + linkage
record**: it defines exactly what the human publication step produces
and verifies, and it pre-commits the honesty rules (nothing unsigned
ships; nothing untested is listed as supported) so the filling step
cannot quietly skip them.

## 3. The artifact set v1.0.0 WOULD publish (§21.4, described — nothing built)

The §21.4 build/distribution table is quoted verbatim in the P10.14
record §2.1; the artifact classes below carry it into this record with
their checksum/signature/SBOM obligations. **Every row is a
DESCRIPTION of a future build — no artifact of any row exists.**

| # | Artifact (per §21.4) | Form | Checksum (§4) | Signature (§5) | SBOM (P10.19) |
|---|---|---|---|---|---|
| A-1 | Operator Console | Static React/TypeScript bundle (served by the control plane or a CDN) | required | required | required |
| A-2 | Control Plane | Linux x86-64 and ARM64 OCI image plus native binary | required (per artifact) | required (signed OCI image per §21.4) | required |
| A-3 | WASM Plugin Host | Linux x86-64/ARM64 container | required | required | required |
| A-4 | Numerical/Research Worker containers | Linux Python container images pinned by lockfile | required | required | required (per §9.3, plugin packages included) |
| A-5 | Site Edge Agent | Versioned Linux x86-64/ARM64 package/OCI image **with signed updates** (§18.3) | required | required — the update-manifest path of `crates/edge-agent/src/update_verification.rs` | required |
| A-6 | CLI/Simulation Lab | Release archives (CLI on Linux/macOS/Windows) + OCI images (simulators) | required (per archive/image) | required | required |
| A-7 | Helm chart | Chart package (from v1.0.0, §21.4) | required | required | required |
| A-8 | SBOMs | Per artifact class, from lockfiles (P10.19 §6) | required (checksummed like any artifact) | required | — |
| A-9 | Checksum manifest | The sha256sum listing of A-1..A-8 | — | signed (§5) | — |
| A-10 | Plugin packages | Per §9.3: signed manifests + SBOMs for production plugin installation | required | required (signatures/provenance are a production-install requirement, §9.3) | required |

## 4. Checksum procedure — sha256sum over the built artifacts (human gate, EMPTY)

The candidate build (P10.18 §3 step 7) and its artifact-hash capture
(P10.18 §3 step 9 / §7 job) are the source; this record publishes the
checksums. Nothing below was executed.

```sh
# 0. Preconditions: the P10.18 candidate identity is FILLED
#    (commit/tag/build ID/artifact hashes) — checksums are published for
#    THAT identifier or for nothing (the ledger's named risk: "a checksum
#    published for a different build than the candidate").
# 1. Over every artifact of §3 (binaries, bundles, images exported as
#    tarballs, archives, chart package, SBOMs):
sha256sum <artifact...> | tee checksums-v1.0.0.txt
# 2. Record, per artifact: name, sha256, byte size, candidate commit/tag.
# 3. The checksum manifest itself (checksums-v1.0.0.txt) is signed (§5)
#    and published WITH the artifacts — a checksum set without the
#    artifacts' signatures is tamper-EVIDENCE only, never a trust root.
```

| Artifact (§3 row) | sha256 | Size (bytes) | Candidate tag/build ID |
|---|---|---|---|
| A-1 … A-10, one row each at publication | `<EMPTY — nothing built>` | `<EMPTY>` | `<EMPTY>` |

## 5. Signing procedure — the production-signing DEPLOYMENT REQUIREMENT, carried honestly

**No production signing key material exists.** No key was generated,
stored, or referenced anywhere in this repository's history, and no
release was ever signed. The signing precedents in code are honest
about this, and this record inherits their honesty verbatim in
substance:

- `crates/edge-agent/src/update_verification.rs` (P9.34): the DEFAULT
  verifier is `NoUpdateVerifier` — **refuses EVERYTHING** ("the absence
  of signing infrastructure can never look like success"); the
  `TestOnlyFnv1a64UpdateVerifier` is the loudly-labelled test-grade
  FNV-1a-64 checksum — **"UNSAFE for update-integrity by construction…
  a deployment that wires this in production has wired nothing"**; and
  `UPDATE_STATEMENT` records that "no update has ever been built,
  signed, staged, or applied; no cryptographic signature exists…
  wiring a production SHA-256 (or stronger) verifier is a DEPLOYMENT
  REQUIREMENT."
- `crates/flexcore-research/src/worker_identity.rs` (P8.33): identity
  and key MATERIAL are deployment-held; code carries scheme-prefixed
  REFERENCES only (`key-store:` / `env:` / `file:` — the
  `UPDATE_KEY_REFERENCE_SCHEMES` closed set), never values.

**The test-grade digests in code are NOT release signatures.** Any
checksum or digest computed by the repo's test-grade helpers (FNV-1a-64)
is integrity-test furniture; a release signature is a production
verifier under reviewed key management. The two must never be
conflated in a publication record.

**The procedure the human signing step follows (recorded, never
executed):**

1. **Key material** — generated and held by the DEPLOYMENT (key store;
   the `key-store:` reference scheme; the P9.09 certificate inventory
   feeds the trusted signer-reference set). Key material never enters
   the repository, an artifact, or an audit record (the references-only
   rule).
2. **Signer identity** — the signing identity (who/what key) is
   recorded per artifact class at signing time; the trusted signer
   REFERENCE set the edge `UpdateGate` checks is the deployment's,
   configured from that inventory. The signing TOOL for OCI images and
   release archives is not yet recorded in this repository — the tool
   and version actually used are recorded at signing time (the ledger's
   unnamed-tool rule; no tool is assumed here).
3. **Per artifact** — the §3 signature column is satisfied: signed OCI
   images (§21.4), the edge update bundle carries its §18.3 manifest
   (version, bundle digest, `sha-256` algorithm, signer key REFERENCE,
   minimum-version floor, operator approval reference — the
   `UpdateManifest` fields; `approval_reference` empty = refused,
   fail-closed), plugin packages carry signature + provenance (§9.3),
   and the checksum manifest (§4 step 3) is signed.
4. **Algorithm floor** — `sha-256` or stronger under reviewed key
   management; the `fnv1a64-test` algorithm is refused for anything
   real (the closed `UPDATE_DIGEST_ALGORITHMS` vocabulary carries both
   names precisely so the test-grade one is never mistaken for the
   production one).
5. **Honest classification** — the digest-and-record scheme is
   tamper-EVIDENCE against casual tampering; it is NOT a cryptographic
   guarantee and NOT secure boot (the P6.34 position stands: every
   hardware determination is PENDING PILOT HARDWARE SPEC). No
   certification or security claim is made.

| Signing item | Value |
|---|---|
| Production key material exists | **NO — none exists (binding statement)** |
| Signing identity per artifact class | `<EMPTY>` |
| Signing tool + version actually used | `<EMPTY>` |
| Trusted signer references configured | `<EMPTY>` |
| Artifacts signed | `<EMPTY — nothing built, nothing signed>` |

## 6. Support matrix — LINKED to P10.14, VERIFIED cells EMPTY

The published support matrix is assembled ONLY from filled P10.14
results (the P10.14 record's own rule: "P10.21 publishes the support
matrix ONLY from filled results here"), the frozen contract versions
(P10.02), and the SBOM locations (P10.19 §7 C-1):

| Matrix input | Source record | Status today |
|---|---|---|
| Browser/OS tested cells (console, CLI, containers) | P10.14 §5 + `console/SUPPORT_MATRIX.md` | **ALL result cells EMPTY — no browser/OS run has ever happened; nothing may be listed as supported** |
| Contract versions (REST, Protobuf, WIT, NATS vocabulary/streams) | P10.02 freeze record | Recorded positions; the compatibility matrix's linkage is P10.02's deliverable |
| SBOM locations | P10.19 §7 C-1 | `<EMPTY — no SBOM exists>` |
| Artifact ↔ supported-runtime mapping | §3 of this record | Described targets (§21.4), zero verified cells |

**Rule carried verbatim in substance (§22.3 gate 10):** no untested
cell is listed as supported. Until P10.14's cells are filled, the
honest published support matrix is an EMPTY matrix with its targets
stated as targets — never a support claim.

## 7. Publication verification — EMPTY (human gate)

The ledger's step 4: verify the published artifacts from the
publication location, not the build directory.

| Verification item | Status |
|---|---|
| Download each artifact class from the publication location; re-compute sha256; compare to the published checksum manifest | `<EMPTY — nothing published>` |
| Verify signatures from the publication location (production verifier; trusted signer set) | `<EMPTY>` |
| Confirm the edge update bundle verifies on a test edge host (§18.3 — the `UpdateGate` verify-before-staging path against the real signed bundle) | `<EMPTY — no update has ever been built, signed, staged, or applied>` |
| Independent download-and-verify of one artifact per class (the ledger's verification method) | `<EMPTY>` |

## 8. Distribution channels and support commitment — recorded as UNDECIDED where silent

- §21.4 names the distribution FORMS (bundled console; signed OCI;
  release archives; versioned edge package with signed updates; Helm
  chart from v1.0.0) but does not name the hosted channels (registry
  endpoints, download host, key distribution). **The concrete channels
  are UNDECIDED** — recorded as such per the ledger's step 5 ("or
  record that it is undecided"); they are filled at P10.28 preparation,
  and P10.28 remains prepared-NOT-EXECUTED.
- **The support commitment (what "supported" means and for how long) is
  UNDECIDED.** No duration, patch cadence, or support-window promise is
  invented here — a support promise in a release record is a product
  commitment only the go/no-go meeting and product own.

## 9. The release-publication precondition chain

Publication (P10.28) is legal only at the END of this chain, every link
evidenced, no link waivable silently (a waiver is explicit, signed,
reasoned, time-bounded — P10.27's rule):

1. **P10.18** — CI gates run on the gated commit; candidate built;
   identity (commit/tag/build ID/hashes) recorded and filled into
   P10.01 §6.1. *(EMPTY today — the candidate does not exist.)*
2. **P10.19** — SBOMs generated from the lockfiles; license audit run;
   D4 closed and signed by its owners. *(NOT CLOSED — verdict recorded
   there.)*
3. **P10.20** — final scan against the candidate artifacts and SBOMs;
   every critical/high fixed or signed-time-bounded; zero untriaged.
   *(NOT CLOSED.)*
4. **P10.14** — every §21.4 matrix cell tested or explicitly removed.
   *(ALL cells EMPTY.)*
5. **P10.21 (this record)** — §4 checksums over the candidate artifacts;
   §5 signing under the production DEPLOYMENT REQUIREMENT (real key
   material, real verifier — the test-grade digests never count); §6
   support matrix assembled from filled cells only; §7 publication
   verification from the publication location.
6. **P10.26** — release notes with known limitations (including any
   accepted vulnerabilities from P10.20 and removed matrix cells).
7. **P10.27** — the go/no-go decision says go.
8. **P10.28** — tag + publication, then external verification. Only
   then does "published" become a true statement.

**Today, zero links of this chain are satisfied.**

## 10. Findings recorded by this record

| # | Finding | Disposition |
|---|---|---|
| F-1 | No artifact exists; every checksum/signature/verification cell is EMPTY; nothing is published | Binding honesty statement; ENTRY_NOTE boundary 1 |
| F-2 | No production signing key material exists; the code's signing surface refuses everything by default (`NoUpdateVerifier`) and its test-grade FNV-1a-64 digests are NOT release signatures — the production SHA-256-or-stronger verifier under reviewed key management is a standing DEPLOYMENT REQUIREMENT | §5; carried to the human signing step |
| F-3 | The signing tool for OCI/archive artifacts is not recorded anywhere in the repository; it is recorded at signing time, never assumed | §5 step 2 |
| F-4 | The support matrix has zero verified cells; publishing any "supported" claim before P10.14 fills would breach §22.3 gate 10 — this record pre-commits the rule | §6 |
| F-5 | Distribution channels and the support commitment are UNDECIDED — no channel or duration is invented | §8 |
| F-6 | The edge-update verification path (§18.3) has never processed a real signed bundle; its first real run is a §7 publication-verification item, not a claim | §7; cited from `update_verification.rs` honesty bounds |

## 11. Reviewer-verification procedure (human)

1. Confirm §1/§4/§5/§7: no artifact, checksum, signature, signing
   identity, or publication claim exists anywhere in this record; every
   value cell is `<EMPTY>` or an explicit NO.
2. Confirm the §3 artifact set matches the §21.4 table as quoted in the
   P10.14 record §2.1 (no artifact class invented, none dropped,
   Helm chart included per the v1.0.0 requirement).
3. Confirm §5 quotes-in-substance the `NoUpdateVerifier` /
   `TestOnlyFnv1a64UpdateVerifier` / `UPDATE_STATEMENT` honesty lines of
   `crates/edge-agent/src/update_verification.rs` and does not soften
   them; confirm the references-only key rule matches
   `UPDATE_KEY_REFERENCE_SCHEMES`.
4. Confirm §6 links P10.14/`console/SUPPORT_MATRIX.md` and P10.02/P10.19
   by path, and that no support claim is made.
5. Confirm §9's chain matches the ledger dependency edges (P10.14,
   P10.18, P10.19 → P10.21 → P10.28; P10.27 before P10.28).
6. Confirm the ledger tick reads "implemented, awaiting human
   build/checksum/sign/publish" and the §12 verdict is NOT CLOSED.

## 12. Closure verdict

**Signed artifacts / checksums / support matrix for v1.0.0: NOT
CLOSED — nothing is published, signed, or checksummed, because nothing
is built.** The artifact set, procedures, and precondition chain are
recorded; the filling is the human publication gate, upstream of which
P10.18/P10.19/P10.20/P10.14 are all unfilled. No statement in this
record may be read as "the release is ready to publish".

## 13. Filling commands (human, at execution time)

```sh
# After the chain of §9 links 1–4 are filled:
sha256sum <artifacts> | tee checksums-v1.0.0.txt
# sign per the §5 procedure (deployment key store; tool + version recorded)
# publish through the (then-decided) channels; verify from the
# publication location; run the §18.3 edge-host verification.
# Fill §4/§5/§7 tables from the actual outputs.

# Registry verification after any edit to this record:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 14. Status

**WRITTEN-UNRUN / PREPARED-NOT-EXECUTED.** The artifact set is
described, the checksum/signing/verification procedures are recorded
with their honesty bounds carried from code, the support matrix is
linked with zero verified cells, and the precondition chain ends at a
publication that has not happened and is not prepared to happen in
this loop.
