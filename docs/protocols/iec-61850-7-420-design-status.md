# IEC 61850-7-420 MMS Client — DESIGN-STATUS Document

| Field | Value |
|---|---|
| Scope | The IEC 61850-7-420 DSO-boundary SURFACE as DESIGNED — this is a design-status document, NOT an adapter guide: **no adapter exists** |
| Owner record | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/42_p14_10_iec_61850_7_420_mms_client_design.md` |
| Implementation status | **DESIGN RECORD ONLY.** No crate exists (`crates/adapters/protocol-61850/` is a PROPOSED path, not created, not a workspace member, not in the dependency checker). The design lands as code only in the batch that implements the first NAMED EU DSO integration — the P14-E-3 DEFAULT |
| Conformance status | **No conformance, certification, compliance, or security claim of any kind.** No certification program is named in the repository's source register (UNVERIFIED-ABSENCE — evidence plan, records 101 §2) |
| Control status | Supervisory relay only (in the design): an MMS report of an external control object enters the frozen 14-state command lifecycle at PROPOSED; an MMS write would be a command-relay surface AFTER the three gates — never a direct device write |
| Boundary | **GOOSE and SV are OUT OF SCOPE — entirely, in every batch, without exception.** They are protection-grade communication (foundations §0.3): FlexCore performs no relay protection, no inverter inner-loop control, no sub-cycle control. Reversing this is a foundations-level change, not a phase choice |
| Why this document exists | P14.23 requires operator documentation per protocol adapter; for the two design-record protocols the honest form is a DESIGN-STATUS document that states clearly what does NOT exist yet |

## 1. The designed surface (all details UNVERIFIED secondary-source readings)

| Element | Design reading | Verification gate |
|---|---|---|
| Role | FlexCore as the MMS CLIENT at the DSO boundary; an EU DSO's 61850 server exposes the DER device's data model | the named DSO integration defines the profile |
| Logical nodes | `DRCT` (DER control), `ZRCT` (reactive control), `DRCC` (DER curve/characteristic) — node semantics UNVERIFIED until checked against the 61850-7-420 text [S-12] | the standard text is procured BEFORE any model type lands (record 42 §8) |
| MMS stack | MMS (ISO 9506) over the MMS/ACSE/TCP stack; client reads data, writes control values, receives reports | exact object-name syntax and report-control-block semantics |
| SCL/ICD ingestion | A typed `SclDocument` model behind an `SclSource` SEAM (shipped refusal: `SeamNotWired` — no XML parser dependency); every discovered logical node classified MAPPED / EXPLICITLY_UNMAPPED / UNMAPPED / AMBIGUOUS; protection-grade node families recorded EXPLICITLY_UNMAPPED with the foundations-§0.3 reason — never read, never subscribed, never mapped | the SCL schema sections the ingestion needs |
| Control model | Status-only / direct / SBO variants (spellings UNVERIFIED) | which control model the DSO requires |

## 2. Security posture (design input, NOT a posture statement)

| Topic | Design position |
|---|---|
| MMS security | The security profile of an MMS session (TLS variants per the 61850/62351 family — UNVERIFIED) is a per-deployment review before any session; no claim is made |
| ICD trust | ICD files are untrusted input: classification never guesses, ambiguity blocks; protection-grade families are refused by declaration |
| Secrets | Phase 13 secret REFERENCES only; `secret.read` stays in the never-granted set |

## 3. What does NOT exist (the honest inventory)

| Item | Status |
|---|---|
| `crates/adapters/protocol-61850/` | NOT CREATED — proposed path only |
| Workspace member / checker entry | NONE |
| Any 61850 code, logical-node type, SCL parser, or test | NONE |
| Any MMS session, ICD read, report, or control object | NONE — no ICD was read, no logical node was resolved, no message was exchanged |
| GOOSE / SV anything | NONE — out of scope entirely (protection-grade) |
| Certification path | NONE named (UNVERIFIED-ABSENCE; evidence plan, records 101) |

## 4. What would start it

The named EU DSO integration closes P14-E-3 (record 42 §6); the
implementing batch then follows the crate pattern with record 42 §1/§3/§4
as the design and its UNVERIFIED register as the first gate — including
procurement of the [S-12]/[S-13] texts BEFORE any model type lands.
Until then this surface is design-only, and every statement above is a
statement about a DESIGN, never about behavior.
