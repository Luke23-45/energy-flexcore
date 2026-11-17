# P17.14 — OCPP conformance test suite (WRITTEN-UNRUN): the OCA use-case matrix SUBSET + the negative family; execution and certification are human lab gates

| Field | Value |
|---|---|
| Subtask | P17.14 (OCPP conformance test suite, written-unrun): OCA conformance use-case matrix SUBSET (provisioning, transactions, smart charging, security events, firmware status) + negative tests (malformed frames, subprotocol mismatch, certificate failures); execution and certification are human lab gates [S-121][S-127] |
| Wave / agent | Phase 17 wave 2, Agent A |
| Status | **WRITTEN-UNRUN**: the suite is complete test code that has never compiled or executed. No conformance, certification, qualification, or interoperability claim is made anywhere; no lab was ever engaged; nothing has ever run. |
| Sources | [S-121] OCA OCPP protocols page, [S-127] OCA Security Operations Guide; the Edition 2 parts 5/6 certification-profile publication-date conflict is recorded in `ledger/phase-06-edge-protocol/RESEARCH.md` §3 (open verification task); wave-1 records 01/03/04/05 (the surfaces the suite pins) |
| Files touched | `crates/adapters/protocol-ocpp/src/conformance.rs` (NEW, additive module: the declared matrix + shared negative vectors); `crates/adapters/protocol-ocpp/src/lib.rs` (additive `pub mod conformance;` entry — shared file, re-read before edit); `crates/adapters/protocol-ocpp/tests/conformance_suite.rs` (NEW, written-unrun) |

## 1. What "conformance suite" honestly means here

OCA/DEKRA-class OCPP conformance is a human-gated lab process: the OCA's
own test harness, fees, sign-offs, and (for 2.1) the Edition 2 parts 5/6
certification profiles whose publication-date conflict is unresolved
(phase-06 research §3). This program's suite is EVIDENCE PREPARATION, not
a result: the use-case ids (`UC-…`) are THIS PROGRAM's subset keys,
pre-registration ready (the X-8 discipline, conventions §5) — never OCA
certification-profile case ids, and the mapping to the OCA's official
cases is UNVERIFIED until checked against the OCA text. A green run of
the suite, if a human ever runs it, is evidence about THIS adapter's
recorded behaviour only.

## 2. The declared matrix SUBSET (23 rows, all pinned by the suite)

| Family | Rows | What the suite pins (against real wave-1 surfaces) |
|---|---|---|
| PROVISIONING | UC-PROV-01…05 | Boot decode + fail-closed allowlist policy + domain boot mapping; StatusNotification through the declared domain table; unmapped status / unknown EVSE refused (never approximated, never auto-created); Heartbeat liveness-only (absence structural); link loss touches ONLY the link state, statement travels verbatim |
| TRANSACTIONS | UC-TXN-01…04 | Started opens Active / Updated refreshes / Ended closes; an update or end for a never-started transaction is refused (the registry never invents one); invalid payloads refused by the model |
| SMART_CHARGING | UC-SC-01…04 | The wire payload built ONLY from ISSUED (every other frozen lifecycle state refused by typing, found state named); Accepted = stored-not-applied (statement verbatim); composite reconciliation like-with-like with NAMED mismatches; one-outstanding-CALL discipline + the named overflow refusal |
| SECURITY_EVENTS | UC-SEC-01…04 | Event ingestion as bounded records with the declared classification (no automatic reaction surface); unknown type = Unclassified, empty type refused; the certificate flow REQUESTED → SIGNED_AVAILABLE → INSTALLED / REJECTED with issuance EXTERNAL (no CA stood up); unproofed registration + double-open refused |
| FIRMWARE_STATUS | UC-FW-01…02 | FirmwareStatusNotification is OUTSIDE the closed handled vocabulary — the decode refuses WHOLE with the named UnknownAction error (no loose pass-through); whether the wire translation is an OCPP NotImplemented CALLERROR is an UNVERIFIED transport-seam check, recorded in the row, never claimed; no firmware action was ADDED for the suite (the closed vocabulary is pinned) |
| NEGATIVE | UC-NEG-01…04 | Malformed frames refused whole (the shared vector); subprotocol mismatch (1.6 with the P17-E-1 reason, unsupported offers, empty offer — no unspecified default); certificate failures with named errors (empty CSR, double open, signed-send on ABSENT flow, unproofed registration, empty trust anchors); unmatched replies refused, never applied |

## 3. The negative vectors and the wave-1 reuse

Integration-test files cannot import each other, so the wave-1 close-out
handoff ("P17.14's conformance suite should import
`tests/framing_codec.rs`'s negative cases") is implemented as RESTATEMENT
WITH CITATION: `conformance::malformed_frame_negatives()` restates the
nine `framing_codec.rs` negative cases verbatim (same wire texts, same
expected `FramingError` debug renderings), and both the wave-1 module
tests and the suite pin them. A divergence between the two vectors is a
recorded edit, never a silent reinterpretation. The subprotocol-mismatch
vector maps every row through the REAL `negotiate` function (including the
empty offer → `NoSubprotocolOffered`); the certificate-failure vector's
five rows are each matched against the real security surface's named
error.

## 4. What the suite did NOT do

- No action, subprotocol, session state, or model type was added for the
  suite's sake (the closed vocabulary is unchanged; P17-E-1 stays OPEN;
  the subprotocol set is not widened — the hard ban honoured).
- No test was RUN; no build was made; no OCA text was consulted (none is
  in the repository); no lab, fee, or sign-off exists.
- No external dependency was added; the module uses only the crate's
  existing types (`serde_json` was already a dependency).

## 5. Honest boundaries

1. **WRITTEN-UNRUN** — conventions §1.3; the suite's first execution is a
   human gate, and the OCA-text verification pass over every pinned
   spelling (the wave-1 UNVERIFIED registers) is a second one.
2. **Zero conformance claims** — conventions §7.3; the statements
   (`WRITTEN_UNRUN_STATEMENT`, `MATRIX_SCOPE_STATEMENT`) travel in the
   module and are pinned verbatim by tests.
3. The suite pins THIS adapter's recorded behaviour; where a use case
   exercises a refusal path (e.g. UC-FW-01), the covered behaviour IS the
   refusal — the row says so, and no capability is implied beyond it.

## 6. Handoffs

1. **To the human lab gate**: run order is (a) the first workspace build,
   (b) the suite, (c) the OCA-text verification pass over the wave-1
   registers and this suite's spellings, (d) only then any OCA
   engagement (a commercial/human decision entirely outside this
   program).
2. **To wave-2 Agent B**: the matrix is the evidence-preparation
   inventory for the phase's close; P17.16/P17.18 should cite it as
   written-unrun evidence preparation, never as conformance progress.
3. **To P17.15** (same wave): the simulated charge points exercise the
   same surfaces the suite pins (codec, negotiation, tracker, relay) —
   the two are complementary fixtures, and neither is a conformance
   result.
