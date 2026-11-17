<!-- P19.15 — GDPR compliance closure plan. HONEST RECORD: a PLAN record
     that prepares STRUCTURES (retention-basis consumption rules, DPIA
     templates, records-of-processing register shape) and names the human
     gates. It consumes gap G-7 from the P19.10 evidence map
     (records/21_p19_10_soc2_evidence_chain.md §5) and closes its PROGRAM
     side only — the privacy-regime determination itself stays human.
     NO regime is asserted to apply anywhere, NO jurisdiction is named
     (D1 OPEN), NO legal determination or sign-off exists, and NOTHING in
     this record says or implies that FlexCore or any deployment is
     "compliant" — the word is used only as the subtask's own title and
     always in the negative posture here. Every retention VALUE, legal
     basis, DPIA cell, and register row is EMPTY by design. [S-86][S-87]
     are cited as REFERENCE ARTICLES the eventual determination may name,
     never as applicable law (the P9.10 §3 scope rule, carried). -->
# P19.15 — GDPR compliance closure plan: retention basis per the P13.14 jurisdiction registry, DPIA templates, records-of-processing structure — every legal determination a human gate

| Field | Value |
|---|---|
| Subtask | P19.15 — GDPR compliance closure plan (`ledger_phase_19.md` §P19.15): retention basis per the P13.14 jurisdiction registry; DPIA templates; records-of-processing structure; legal sign-off = human gate (LR-062 direction) |
| Wave / agent | Phase 19 wave 2, Agent B |
| Status | **PLAN RECORD — WRITTEN, NO DETERMINATION, NO SIGN-OFF.** This record consumes gap G-7 (`records/21_p19_10_soc2_evidence_chain.md` §5) and closes only its program side: the structures a determination process consumes. The determination itself (which regime, which jurisdiction, which basis) is a HUMAN GATE and does not exist. Every value cell below is EMPTY by design. |
| Sources | [S-86] (GDPR Art. 17 — cited as the reference frame the eventual determination may name, never asserted as applicable); [S-87] (GDPR erasure exceptions and limits — same conditional status); the P13.13 GDPR/privacy record (`../phase-13-enterprise-identity-integration/records/25_p13_13_gdpr_privacy.md` — inventory, retention matrix, DSAR design, erasure strategy, consumed BY CITATION); the P13.14 jurisdiction registry record (`../phase-13-enterprise-identity-integration/records/26_p13_14_jurisdiction_registry.md` — the registry shape, sign-off gate, and lifecycle, consumed BY CITATION); LR-062 (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` §3.8, direction; the gap itself is `ledger/phase-09-pilot-qualification/10_data_residency_retention.md`, the P9.10 record — cited, never rewritten); the P19.10 evidence map gap G-7 (`records/21_p19_10_soc2_evidence_chain.md` §5); conventions §4 (agents never close E-decisions), §5 (human gates / X-8) |
| Files touched | This record only. No code, no migration, no registry row, no template file — the DPIA template and the records-of-processing register are shapes defined HERE, to be instantiated as data at human implementation time. `docs/foundations/*`, `docs/plan/*`, `.agent/*`, the P13.13/P13.14 records, and the P9.10 record are untouched. |

## 1. Honest status — the plain statement

**No privacy regime applies to anything, because no deployment exists and
no jurisdiction has been decided (D1 OPEN).** No legal determination has
been made, no DPIA has ever been completed, no record of processing
exists, and no legal sign-off exists anywhere in this program. This
record does not change any of that. What it delivers is the missing
PROCESS LAYER above the P13.13/P13.14 engineering seam:

1. the **retention-basis consumption rules** — how a compliance process
   reads the P13.14 registry without ever inventing a value (§3);
2. the **DPIA template** — a shape with every content cell EMPTY, whose
   completion and sign-off are human acts (§4);
3. the **records-of-processing register structure** — the row shape and
   sourcing rules, with zero rows (§5);
4. the **human-gate map** — every legal act named, with its closer
   (§6), so that nothing in the closure plan can be mistaken for an
   agent-executable step.

The word "compliance" appears in this record only as the subtask's own
title and in the negative posture: **nothing here is compliant, nothing
is certified, and no statement in this record may be read as a claim
that FlexCore meets any privacy regime.** The verbatim discipline of
`ledger/phase-10-v1-release-hardening/25_security_consolidation.md` §7
(quoting AGENTS.md §3 rules 2–3) carries into this record (§7).

## 2. What this record consumes — the G-7 seam and the P13 structures

The P19.10 evidence map's gap list names G-7: "Privacy regime
determination — D1 OPEN, no jurisdiction named, no legal sign-off
exists | Privacy | P19.15 (wave 2) + human legal sign-off (LR-062
direction) | P13.13/P13.14 supply the mechanical structure, not the
determination." This record is the P19.15 side of that row. The
division of labor is strict:

| Layer | Owner | What it holds | Status |
|---|---|---|---|
| Engineering structures | P13.13 (inventory, per-table retention matrix, DSAR design, erasure strategy incl. the backup re-materialization caveat) and P13.14 (the jurisdiction registry shape, sign-off gate, lifecycle) | The data model: where a jurisdiction decision, a basis capture, and a sign-off LAND once made | WRITTEN-UNRUN records; every value EMPTY |
| Compliance process (this record) | P19.15 | The process shapes: how the registry is consumed, what a DPIA contains, how processing is recorded | THIS RECORD; every cell EMPTY |
| The determination itself | HUMAN OWNER (D1) + named human legal signatories | Which jurisdiction, which regime(s), which bases, which retention values — signed per registry row | **DOES NOT EXIST; agent-unclosable** |

No producer content is restated: the P13.13 inventory and matrix, the
P13.14 registry shape, and the P9.10 append-only-vs-erasure analysis are
consumed at their paths, each carrying its own honesty status unchanged.

## 3. Retention basis — consumed, never invented (per the P13.14 registry)

The binding rule is inherited verbatim in substance (P0.16 §4; the P1.10
type header; P13.14 §1): **no universal retention period exists anywhere
in the platform, and no retention value enters the posture except from a
SIGNED, basis-referenced jurisdiction registry row.** This plan adds the
compliance-side consumption rules an operator or reviewer follows:

| Rule | Content | What it forbids |
|---|---|---|
| RB-1 | A retention value is read ONLY from the newest SIGNED registry row for the exact (tenant, jurisdiction, data class) key — the P13.14 §4 lifecycle's resolution rule | Reading a retention value from any document, default, or precedent |
| RB-2 | An UNSIGNED registry row authorizes NOTHING: no retention action, no erasure-obligation determination, no matrix fill — fail closed (the P13.14 §3 consumer rule, carried) | Treating a captured-but-unsigned row as a basis |
| RB-3 | A missing registry row resolves to `UNSET`, which is "no retention action authorized" — never "keep forever" and never a default (the P1.10 `RetentionSetting::Unset` semantics) | Interpreting absence as either extreme |
| RB-4 | A filled `retention_period` without a `basis_reference` is not deployable — the P9.10 §6 filling rule expressed as the P13.14 §2 CHECK, carried | Deploying a value whose primary-source citation is absent |
| RB-5 | Every basis value cites a PRIMARY SOURCE (contract clause, market-rule capture, applicable-requirement citation) — never another summary document, and never this record (AGENTS.md §7; conventions §7.2) | Basis-by-summary; basis-by-this-plan |
| RB-6 | The compliance process (DPIA, RoPA, sign-off requests) READS the registry; it never writes basis values. Writing a capture is an operator act; making it SIGNED is the named human's act (P13.14 §4) | A process document that mutates the registry or pre-fills any cell |

The honest current state of the basis: **every retention value and legal
basis cell in the P13.13 matrix, the P13.14 registry shape, and the P9.10
schedule is EMPTY; this record adds no value and changes none of those
cells.** No retention period appears anywhere in this record.

## 4. DPIA template — structure only, every content cell EMPTY

The template below is a SHAPE for a data protection impact assessment.
It is not a completed assessment, not an assessment of anything, and no
DPIA has ever been initiated for FlexCore. Whether ANY FlexCore
processing ever requires a DPIA is itself a legal determination — a
human gate (§6, HG-2). [S-86][S-87] are the reference frames the
completed assessment may name; citing them here asserts nothing.

| Section | What the completed DPIA will contain | Cell status |
|---|---|---|
| D-1 Processing description | The processing activity, its purpose, its data classes (P1.10 vocabulary only), the categories of data subjects, the flow (which stores, which seams — sourced from the P13.13 §1 inventory by citation) | `<EMPTY>` |
| D-2 Necessity and proportionality | The assessment that the processing achieves its stated purpose with minimization applied (the P13.13 §1.B log-minimization posture is the design input, carried) | `<EMPTY>` |
| D-3 Risks to data subjects | Likelihood/severity reasoning per identified risk | `<EMPTY>` |
| D-4 Mitigations | The existing structural controls cited by path (e.g. RLS posture P0.16, redaction P11.08, audit chain P11.15, tenant scoping P13.12) — cited, never restated, and never upgraded from the producer's own status | `<EMPTY>` (structure present; content EMPTY) |
| D-5 Residual risk | What remains after D-4, stated plainly | `<EMPTY>` |
| D-6 Regime frame | The regime(s) the assessment is conducted under — named ONLY after D1 closes and a SIGNED registry row exists (RB-1); before that, this section is structurally UNFILLABLE | `<EMPTY>` — UNFILLABLE until a signed registry row exists |
| D-7 Consultation | Whether the regime's rules require supervisory-authority consultation — a determination only a completed D-6 enables | `<EMPTY>` |
| D-8 Sign-off block | Named human signatory, role, date, scope of the sign-off — the human gate; mirrors the P13.14 `SIGNED` discipline (a sign-off without a named human and timestamp is void) | `<EMPTY>` — UNSIGNED |

Template rules: a DPIA instantiated from this template that leaves D-6
unfilled is not signable; a sign-off block filled by anyone other than a
named human is void; an instantiating deployment may ADD rows but may
not remove the sign-off block or renumber the sections (the X-8 spirit:
the structure is registered before any assessment that could fill it).

## 5. Records-of-processing register — row shape, zero rows

The register is the structured index of processing activities a
determination process maintains. It is DATA (registered rows, versioned
amendments — the P13.21 registration discipline is the pattern), never
scattered prose. The row shape:

| Field | Source rule |
|---|---|
| `activity_id` | Registered id, assigned at instantiation — never invented here |
| `purpose` | Free text, one purpose per row |
| `data_classes` | The closed P1.10 seven-class vocabulary ONLY (`crates/flexcore-storage/src/retention.rs`); no eighth class |
| `subjects_categories` | Described per row (operator identities, contacts — per the P13.13 §1 inventory's personal-data candidates) |
| `stores_seams` | Cited by path to the P13.13 §1 inventory rows — never re-listed (one source of truth) |
| `recipients_categories` | Described per row |
| `retention_key` | The (tenant, jurisdiction, data class) registry key the activity resolves through — RB-1..RB-4 apply; the register stores the KEY, never a period |
| `transfer_key` | The registry `cross_border_transfer` row reference (P13.14 §2 shape) — the register stores the reference, never a transfer decision |
| `dpia_ref` | The instantiated D-sections' reference (§4), or `NOT-REQUIRED-DETERMINATION-PENDING` — the latter is a status, never a legal conclusion |
| `amendment_version` | Monotonic per activity; amendments are new versions superseding old (the P13.14 append-only discipline) |

**The register starts at ZERO rows.** Nothing in this batch instantiates
a row: a RoPA row describing FlexCore processing would presume both that
processing operates (it does not — everything is WRITTEN-UNRUN) and a
purpose (a human act). The first rows are written by the human
implementation step, sourced per the rules above.

## 6. The human-gate map — every legal act, its closer, its state

| # | Gate | What it is | Closer | State |
|---|---|---|---|---|
| HG-1 | D1 — target jurisdiction (LR-062's residual core) | The program-level decision the P9.10 record §2 frames; unchanged by this record | human owner | **OPEN** (`.agent/DECISIONS.md` read-only; P13.14 §5 carries it) |
| HG-2 | Regime determination | Which privacy regime(s) apply to a deployment — the G-7 core; NEVER asserted by any agent or record | human owner + legal counsel | **NOT MADE** |
| HG-3 | Registry capture + sign-off | Operator captures basis values with primary-source citations; the named human signs (`UNSIGNED`→`SIGNED`, P13.14 §4) | operator (capture); named human signatory (sign) | **NO ROW EXISTS** |
| HG-4 | DPIA initiation + completion + sign-off | Whether any processing requires a DPIA; its completion; its D-8 sign-off | human owner + legal counsel | **NEVER INITIATED** |
| HG-5 | RoPA instantiation | Writing the first register rows | human implementation step | **ZERO ROWS** |
| HG-6 | Legal sign-off on the closure plan itself | Human review of THIS plan as the adopted process | human owner | **UNSIGNED — this record claims no adoption** |

Conventions §4 applies: none of these is closable by an agent; this
record implements the process DEFAULTS (the structures) only because the
subtask directs it, and records the dependence.

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

Additionally, the privacy-specific negative posture this record carries:
no privacy regime is asserted to apply to any deployment; no jurisdiction
is named (D1 OPEN); no legal determination, DPIA, register row, or
sign-off exists; and no statement in this record is a claim that any
processing, deployment, or product is lawful, compliant, or certified
under any regime. [S-86][S-87] are reference citations, not asserted
obligations.

## 8. What this record does NOT do (honest limits)

1. It closes gap G-7's PROGRAM side only: the structures a
   determination process consumes. The determination (HG-2) and every
   other gate in §6 remain with their named human closers; G-7's
   residual half is tracked in the close record (`65_phase_19_close_and_consistency_audit.md`)
   as the human-gate queue.
2. No P13 artifact was edited or restated: the P13.13 inventory/matrix,
   the P13.14 registry shape, and the P9.10 record are consumed at their
   paths with their statuses carried unchanged.
3. No retention value, legal basis, jurisdiction, regime name, DPIA
   content, or register row appears anywhere in this record.
4. No template file or register store was created: §4 and §5 are shapes
   in this record; instantiation is the human implementation step (HG-4,
   HG-5).
5. The never-granted permission set stays byte-identical — privacy
   process adds no authority: `dispatch.authorize`, `dispatch.issue`,
   `secret.read`, `network.any` (quoted unchanged per conventions §6.4).

## 9. Reviewer-verification procedure (human)

1. Confirm §1/§7/§8: no regime asserted, no jurisdiction named, no
   "compliant"/"lawful"/"certified" claim anywhere; the AGENTS.md §3
   rules 2–3 block is verbatim.
2. Confirm §3 carries RB-1..RB-6 and that no retention period appears
   anywhere in this record.
3. Confirm §4's cells are all `<EMPTY>` (D-6 marked UNFILLABLE) and the
   sign-off discipline matches the P13.14 `SIGNED` gate.
4. Confirm §5's register starts at zero rows and stores keys, never
   values.
5. Confirm §6's gate map names human closers only, and that the
   P13.13/P13.14/P9.10 citations resolve on disk and are untouched
   (`git status` on the phase-13 records and the P9.10 file).

## 10. Status

**WRITTEN / PLAN-ONLY — G-7's program side prepared; the determination
itself is a human gate that does not exist.** Ticking P19.15 means
"implemented (plan record written), awaiting human determination,
sign-off, and review" — never "compliant", never "assessed", never
"determined".
