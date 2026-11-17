# P15.01 — Forecasting architecture record: the forecasts-as-inputs boundary, artifact types and horizons, producer permission vocabulary, provenance + signing mirror

| Field | Value |
|---|---|
| Subtask | P15.01 (the ADR-010 extension as an architecture record: forecast artifact types load/PV/price, per site + portfolio; horizons DA/intra-day; provenance + signing mirroring the P8.32 artifact-signing shape; `forecast.publish`/`telemetry.read` permission classes only, NO dispatch permission) |
| Wave / agent | Phase 15 wave 1, Agent A |
| Status | WRITTEN-UNRUN: typed architecture modules with WRITTEN-UNRUN tests — written, never compiled or executed (conventions §1; the first human build is the program gate). No forecast is produced by this program; no skill is claimed anywhere. |
| Sources | [S-41] (GE Vernova GridOS DERMS — forecasting model library as commercial evidence, gap analysis §3 row 3), [S-50] (Tesla Autobidder/Opticaster 15-minute price forecasts), [S-51] (GridBeyond price + consumption forecasts) — commercial table-stakes evidence only; none is a FlexCore source or integration. Seams: `crates/flexcore-research/src/artifact_signing.rs` (P8.32), `crates/flexcore-application/src/forecast_fixture.rs` (P3.31 labeling pattern), `plugins/manifests/forecast-worker.manifest.json` (permission classes), `crates/flexcore-kernel/src/manifest.rs` (never-granted set) |
| Files touched | `crates/flexcore-application/src/forecast_architecture.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive `pub mod` + `pub use` block) |

## 1. What landed (deliverable class: architecture record as typed module)

| Item | Content |
|---|---|
| The boundary statement | `FORECASTING_ARCHITECTURE_STATEMENT`: forecasting produces INPUTS, RECORDS, and PROPOSALS — never commands (ADR-010 extension, program conventions §6.3); structural, not conventional |
| Artifact vocabularies | `ForecastArtifactKind` (Load / PvGeneration / Price — closed; the price kind is an INGESTION reference only, P15.04; never a bid input), `ForecastScope` (PerSite / PerPortfolio), `ForecastHorizon` (DayAhead / IntraDay — no third class; adding one is a deliberate vocabulary extension, never a string field) |
| Provenance + signing mirror | `ForecastArtifactProvenance` (`flexcore-forecast-artifact-provenance-v1`), mirroring the P8.32 `ModelArtifactProvenance` shape BY CITATION (no flexcore-research dependency — the crate manifest policy): artifact ref, closed digest-algorithm vocabulary (`sha-256` / `fnv1a64-test`), lowercase-hex digest validation, scheme-prefixed signer-key REFERENCE (key-store:/env:/file:; the rejected VALUE never echoed), and the three LINKED provenance fields REQUIRED at construction (input-data ref, producing-model ref, eval-set ref) — provenance cannot silently detach from the signature |
| Fail-closed digest gate | `ForecastDigestVerifier` port + `NoForecastVerifier` (refuses EVERYTHING — the P8.32/P7.14 posture) + `UnsafeTestOnlyForecastDigest` (loudly-labelled FNV-1a 64, NOT a security primitive) + `verify_forecast_artifact` with five distinct stable refusal tags (`FORECAST_ARTIFACT_UNSIGNED`-class vocabulary: ref mismatch, algorithm mismatch, digest mismatch, verifier refused); a production SHA-256 verifier is a DEPLOYMENT REQUIREMENT (no crypto crate added this wave) |
| Producer permission gate | `FORECAST_PRODUCER_PERMISSIONS = ["telemetry.read", "forecast.publish"]` (the forecast-worker manifest's classes; a test pins that NO member is dispatch/secret/network-shaped) + `NEVER_GRANTED_PERMISSIONS_MIRROR` byte-identical to the kernel set (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) + `validate_forecast_producer_permissions` (never-granted checked FIRST, then the closed vocabulary) |

## 2. The mirror discipline (recorded — do not drift silently)

The mirror of the P8.32 shape is BY CITATION: `crates/flexcore-research/src/artifact_signing.rs` is the pattern source, this crate does not depend on flexcore-research (manifest policy), and the JSON record path remains the research crate's pattern (this crate imports no serialization library, so the mirror is TYPED — constructed from validated fields; the composition root may parse JSON and call the constructor). A change on either side is a deliberate, recorded seam update, never a silent drift (`KERNEL_PERMISSION_ALIGNMENT_SEAM` records the same discipline for the permission vocabulary).

## 3. The ADR-010 extension, structurally

The module has no function that constructs, mutates, or carries a dispatch command; the artifact envelope's closed field vocabulary cannot express one; the producer permission vocabulary has no dispatch member; and the never-granted set is pinned byte-identical by a test (`the_never_granted_mirror_is_byte_identical_to_the_recorded_set`). A verified forecast artifact is a DATA-INTEGRITY fact, never a dispatch authority.

## 4. Honest boundaries

1. No forecast is produced, no artifact exists, no evaluation cell is filled — the artifact types and the gate are structure only.
2. The signing layer is the digest-and-record STRUCTURE: no PKI, no release keys, no signature algorithm (the P8.32 honesty, mirrored verbatim).
3. Every test is WRITTEN-UNRUN; expected values are hand-derived from the closed vocabularies or the FNV-1a offset-basis definition.
4. The price kind's unit is "currency/MWh (declared per series)" — no default currency exists anywhere in the platform.
