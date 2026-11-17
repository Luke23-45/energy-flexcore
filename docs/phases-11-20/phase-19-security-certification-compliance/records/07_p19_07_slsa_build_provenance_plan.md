<!-- P19.07 — SLSA L1→L2 build provenance plan. HONEST RECORD: a DESIGN —
     GitHub Actions provenance design, cosign keyless signing design,
     verification points. No artifact has been built, no provenance
     generated, nothing signed, no transparency-log entry exists. The
     P10.21 posture (nothing built/signed; no production key material)
     stands unchanged and is cited, never restated. Tool-version specifics
     are UNVERIFIED; the design consumes the [S-78]/[S-79] concept posture
     only. The not-a-certification discipline of P10.25 §7 (§9) carries
     verbatim. -->
# P19.07 — SLSA L1→L2 Build Provenance Plan (GitHub Actions provenance design; cosign keyless signing design; registry-pull and admission verification points)

Date: 2026-11-17. Subtask: P19.07 (`ledger_phase_19.md` §P19.07).
Plan trace: `../plan.md` §2 row P19.07, §3 blocking order row 5 (consumed
by P19.08 — SBOMs feed the attested artifact set — and by the P19.17
freshness checks, both by path). Sources: [S-78] (SLSA levels,
slsa.dev/spec/v1.0/levels — the register's own note: "v1.2 current —
verify"), [S-79] (Sigstore docs); the P10.21 signed-artifacts record
(`ledger/phase-10-v1-release-hardening/21_signed_artifacts.md` — the
not-yet-existing artifact set); the P18.02 update-manifest record
(`../../phase-18-edge-fleet-lifecycle-ota/records/02_p18_02_update_bundle_format.md`).

## 1. Honest status — the plain statement

**No artifact exists, so no provenance has been generated and nothing has
been signed.** No production signing key material exists (P10.21 §5 —
cited, its honesty carried unchanged); no GitHub Actions run has ever
executed (the CI workflows are UNRUN — `../context.md` §2); no Rekor
entry, no OIDC-issued certificate, and no attestation of any kind exists.
This record is the DESIGN the human implementation work consumes when the
build pipeline first exists. Every tool-behavior claim below is the
concept posture of [S-78]/[S-79]; exact tool versions, flags, and
configuration grammar are UNVERIFIED and verified at implementation time
(the unnamed-tool rule).

## 2. What this design consumes — the P10.21 artifact set

The artifact set the plan attaches provenance/signatures to is the P10.21
§3 set (A-1..A-10: console bundle, control plane, plugin host, workers,
edge agent, CLI/lab, Helm chart, SBOMs, checksum manifest, plugin packages)
— cited by path, never restated. That set DOES NOT EXIST (P10.21 §1:
"nothing is checksummed, nothing is signed, and nothing is published").
The design below binds to the artifact CLASSES, so it survives unchanged
when the set materializes; nothing here moves P10.21's verdict.

## 3. SLSA level posture — target levels recorded as TARGETS

| Level (per [S-78]; the spec-page note "v1.2 current — verify" carried) | What it requires (concept posture) | FlexCore design position | Status |
|---|---|---|---|
| L1 | Provenance exists: the build system records how the artifact was built | GitHub Actions generates build provenance for every release artifact (§4) | TARGET — nothing generated |
| L2 | Hosted build platform with signed provenance | The provenance is signed via the platform's OIDC identity (§4–§5); artifact signing via cosign keyless (§5) | TARGET — nothing generated or signed |
| L3+ | Hardened builds, non-falsifiable provenance (concept) | Out of scope for this plan's first increment; revisited at implementation | NOT A TARGET OF THIS RECORD |

**No SLSA level is achieved, measured, or claimed.** The levels are
targets recorded so the implementation work has a direction; assessment
against the spec text is an implementation-time human gate (the "verify"
note on [S-78] is itself carried as UNVERIFIED).

## 4. GitHub Actions provenance — design

1. **Trigger surface**: provenance is generated ONLY by the release
   pipeline's build job for artifacts destined for publication (the P10.21
   §3 classes) — never for scratch/CI-test builds. No CI run exists today
   (all 24 jobs UNRUN — `../context.md` §2).
2. **Provenance generation design**: the build job generates a provenance
   attestation per artifact recording the builder identity, build
   parameters (source ref, workflow path), and output digests. The
   generation mechanism (the SLSA GitHub-generator class of tooling vs.
   native attestation) is chosen and recorded at implementation time —
   the tool is named there, never assumed here.
3. **Provenance signing**: the provenance attestation is signed through
   the platform's OIDC-issued identity (the L2 requirement, concept per
   [S-78][S-79]) — short-lived, no long-lived key held by the repo.
4. **Binding to the candidate identity**: provenance digests are bound to
   the candidate identifier the P10.18 record defines (commit/tag/build
   ID) — a provenance whose subject digest does not match the published
   artifact digest fails verification (the P10.21 §4 candidate-binding
   rule, carried in substance).
5. **Verification**: provenance is verified at the §6 verification points,
   not at generation time only — a provenance that is never checked is
   decoration, not a control.

## 5. Cosign keyless signing — design

| Element | Design position | Status / honesty |
|---|---|---|
| Identity issuance | Short-lived certificates issued from an OIDC identity at signing time (the [S-79] keyless concept) — no long-lived signing key is created, stored, or rotated by this program | DESIGN — no certificate ever issued |
| Transparency | Signatures/expectations recorded to a transparency log (the [S-79] Rekor concept) so signing events are discoverable | DESIGN — no log entry exists |
| Trust configuration | The verifier's trust configuration (identity constraints: repo, workflow, signer subject) is DEPLOYMENT-HELD configuration, following the references-only rule (P10.21 §5 step 1-2: material and trust configuration never enter the repository or artifacts) | DESIGN — no trust configuration exists |
| Relationship to the P10.21 requirement | Keyless signing is a CANDIDATE mechanism for satisfying the P10.21 §5 production-signing DEPLOYMENT REQUIREMENT (sha-256-or-stronger under reviewed key management). It does not satisfy it by existing: the requirement is met when real artifacts are signed under reviewed trust configuration, evidenced at P10.21's gates. P10.21's verdict is untouched | Cited; nothing closed |
| Edge-agent verification path | The edge agent verifies update manifests against deployment-held signer REFERENCES (the P9.34/P18.02 `UpdateGate`; closed scheme vocabulary `key-store:`/`env:`/`file:`). A keyless/transparency-based verification path on an offline-capable edge host is a DIFFERENT trust model (it requires online verification or a bundled trust anchor) — recorded as an OPEN DESIGN QUESTION for the edge trust-model owner, never assumed solved here. The edge keeps verifying via the existing manifest path until that question closes | OPEN — carried as a named question |
| What keyless does NOT do | It does not make the checksum manifest redundant (the P10.21 §4 checksum set stays), does not replace the SBOM binding (P19.08), and does not constitute any certification of the artifacts' contents | Stated, not implied |

## 6. Verification points — where signatures and provenance are checked

| # | Verification point | What is verified | Consumer | Status |
|---|---|---|---|---|
| VP-1 | Registry pull (deploy-time) | The OCI image's signature and provenance verify against the deployment trust configuration BEFORE the image is used; an unverified or unverifiable image is not deployed (fail-closed) | Deployment tooling (human-operated; no tooling exists) | DESIGN — never exercised |
| VP-2 | Admission (cluster policy) | The cluster admission policy re-verifies signature + provenance at admission, independent of VP-1; both checks must pass | Cluster admission (no cluster exists) | DESIGN — never exercised |
| VP-3 | Release publication (P10.21 §7) | Signature + provenance verified FROM the publication location, alongside the existing checksum re-computation | The P10.21 publication-verification gate | DESIGN — P10.21 §7 cells stay EMPTY |
| VP-4 | Artifact download by third parties | Published verification instructions for consumers (how to verify a release) — a documentation deliverable at release time | External consumers | NOT-STARTED |

Fail-closed rule carried from the code posture: an absent verifier or an
unresolvable trust configuration refuses everything (the
`NoUpdateVerifier` posture — P10.21 §5 — is the program-wide default
direction: "the absence of signing infrastructure can never look like
success").

## 7. Thresholds (X-8)

None set here beyond the fail-closed rule: no freshness/validity window
for provenance or certificates is invented — the values follow the
tooling's actual semantics at implementation time and are recorded THEN
(before first operational use), never pre-asserted.

## 8. Handoffs

- **P19.08**: the SBOM process (record 08) binds SBOMs to the same
  artifact digests the provenance covers — one artifact identity, two
  evidence kinds, both cited by digest.
- **P10.21 (frozen)**: this design changes nothing in that record; its §9
  publication precondition chain is unchanged.
- **Edge trust-model owner**: the §5 open question on edge-side keyless
  verification.

## 9. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

Signing and provenance are supply-chain integrity mechanisms — tamper
EVIDENCE and provenance attestation, never a certification of the
software's behavior, safety, or conformance.

## 10. Status

**DESIGN-ONLY / WRITTEN / NEVER-EXECUTED.** No artifact, provenance,
signature, certificate, or log entry exists. Ticking P19.07 means
"implemented (design written), awaiting the first human build pipeline".

## 11. Reviewer-verification procedure (human)

1. Confirm §1: no run, artifact, signature, or log entry is claimed; the
   P10.21 verdict is untouched.
2. Confirm §3: levels are targets only, with the [S-78] "verify" note
   carried as UNVERIFIED.
3. Confirm §5: no long-lived key is introduced; the edge keyless question
   is OPEN, not assumed solved.
4. Confirm §6: every verification point is DESIGN, fail-closed posture
   carried.
5. Confirm §9's verbatim block matches `AGENTS.md` §3 rules 2–3 exactly.
