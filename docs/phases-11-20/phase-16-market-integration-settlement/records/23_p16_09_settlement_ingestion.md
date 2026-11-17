# P16.09 — Settlement ingestion: parser specifications, statement model types, reconciliation with the dispute queue

| Field | Value |
|---|---|
| Subtask | P16.09 (PJM Manual 29 CSV/XML parser specifications + MSRS/BLI reconciliation workflow [S-93]; CAISO 5-minute SQMD [S-97]; EBGL Art. 52–55 15-minute ISP imbalance statements [S-107][S-108][S-109]; statement→expected reconciliation with the dispute queue feeding P16.12. Parser SPECS + model types + synthetic-fixture designs; no real statement is parsed — none exists here. Consumes Agent A's P16.01 settlement tables by citation) |
| Wave / agent | Phase 16 wave 1, Agent B |
| Status | WRITTEN-UNRUN: model types, fixture grammars, extraction functions, and the reconciliation engine are written, never compiled, never executed. No real statement has been received or parsed — none exists here; every fixture is SYNTHETIC. |
| Sources | [S-93] (PJM Manual 29 settlement formats — the CSV/XML statement class and MSRS/BLI statement class), [S-97] (CAISO Tariff §11 settlements as of Nov 1 2024 — the 5-minute SQMD class), [S-107] (EBGL (EU) 2017/2195, Art. 52–55 — the 15-minute ISP), [S-108] (ACER Decision 18-2020 ISHM), [S-109] (RTE 15-minute ISP transition) |
| Files touched | `crates/flexcore-application/src/settlement_models.rs` (NEW); `crates/flexcore-application/src/settlement_reconciliation.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module lines only) |

## 1. What landed

| Item | Content |
|---|---|
| `settlement_models::PjmBliLineItem` | The MSRS Billing Line Item MODEL TYPE [S-93]: line identity, charge type carried verbatim (UNVERIFIED per-source vocabulary), exact decimal amount with declared currency, statement reference |
| `extract_pjm_m29_bli_csv` | The PJM M29 CSV fixture grammar (DECLARED v1 subset): header grammar `statement_ref,charge_type`; rows `statement_ref,charge_type,amount,currency`. Recorded v1 LIMITATION: no quoted-cell grammar, no escaped commas — a real statement needing quotes is OUTSIDE the subset and refused, never approximated. Header-only statements refuse; every refusal is named |
| `StatementXmlNode` + `extract_pjm_m29_bli_xml` | The internal XML node seam (the protocol-entsoe `xml_seam` precedent — no XML parser library in this crate; the composition root's parser fills the tree) and the M29 XML fixture grammar (`<statement ref currency>` / `<bli ref charge_type>amount</bli>`) |
| `CaisoSqmdIntervalRecord` + `extract_caiso_sqmd_row` | The CAISO 5-minute SQMD model [S-97]: interval length validated to exactly 300,000 ms (re-binning refused, never performed), exact decimal quantity, verbatim quality flag and basis |
| `EbglIspImbalanceRecord` + `extract_ebgl_isp_row` | The EBGL 15-minute ISP imbalance model [S-107][S-108][S-109]: ISP length validated to exactly 900,000 ms (a 60-minute pre-transition ISP is refused, never re-binned), exact decimal imbalance energy in MWh, direction derived under the DECLARED sign convention |
| `settlement_reconciliation::reconcile_bli_statement` | The statement→expected reconciliation engine: per-line outcomes (Matched / AmountMismatch with both values and the exact deviation / MissingInStatement / UnexpectedInStatement), BTreeMap deterministic ordering, exact string line identity (the P8.11 discipline), duplicate-identity and cross-currency run-level refusals, declared tolerance (Exact or Within, inclusive boundary) |
| `DisputeQueueEntry` | The P16.09 → P16.12 handoff record: queue entry reference, line/statement references, derived reason code, BOTH values verbatim |
| Tests (written-unrun) | Inline suites over SYNTHETIC PJM-M29-shaped CSV/XML, CAISO-SQMD-shaped, and EBGL-ISP-shaped fixtures: extraction, wrong-length/wrong-structure/wrong-currency refusals, reconciliation outcomes, tolerance boundary, queue-entry derivation |

## 2. Recorded decisions

1. **Jurisdiction-specific record packs, never a universal engine** (ADR-011; `JURISDICTION_SPECIFIC_STATEMENT`, pinned by tests): three named packs with named sources; a fourth jurisdiction is a NEW pack.
2. **Parser specs, not parsers of real statements** (`NO_REAL_STATEMENT_PARSED_STATEMENT`): each grammar is a declared, deliberately-narrow v1 subset; anything outside it is a named refusal, never a guess. No format-conformance claim is made — risk R-P16-2 (real statements encode tariff detail no fixture grammar captures) is recorded in the plan and inherited here.
3. **Every mismatch is a queue entry, never a silent adjustment** (`NO_SILENT_ADJUSTMENT_STATEMENT`): disputed lines keep BOTH values verbatim; the engine never settles, corrects in place, or writes back.
4. **Line identity is EXACT string equality** (`EXACT_LINE_IDENTITY_STATEMENT`): near-miss references FLAG as Missing/Unexpected, never match; duplicates refuse rather than merge.
5. **Persistence is consumed by citation, not redefined**: the settlement records persist in Agent A's P16.01 market-records store (the phase-16 wave-1 split; migration `0022_market_records_p16_01`). This module declares NO store trait of its own; the composition root binds the record types to A's store. **Wait-check result at batch close: Agent A's artifacts LANDED during the batch** — `crates/flexcore-storage/src/market_records_store.rs` (the `MarketRecordsStore` trait, `MarketSettlementRow`, and the closed `MARKET_SETTLEMENT_SOURCE_FORMATS` vocabulary `PJM_MANUAL_29` / `CAISO_SQMD` / `EBGL_ISP` / `PROGRAM_DECLARED`) and `crates/adapters/postgres/migrations/0022_market_records_p16_01.{up,down}.sql`. The citation is now to the LANDED interface: `MarketSettlementRow` carries `amount_text` (validated decimal text — the same wire-faithful-text discipline as [`crate::fixed_decimal`], no float conversion on either side) and `currency_code` as data, and the trait doc explicitly designates the P16.09 ingestion write as Agent B's consumer. The label layers differ by design (A's SCREAMING_SNAKE persistence vocabulary vs this module's kebab-case record labels); the mapping is one-to-one and recorded here. No A file was edited and no store type is redefined or duplicated.
6. **Money and metered quantities are exact decimals** (`fixed_decimal`); no settlement number is a float and none carries market meaning.

## 3. Honest boundaries

1. No market is connected; no statement has ever been received or parsed; every fixture is hand-built and synthetic; no settlement number is claimed anywhere.
2. The fixture grammars are specifications for the human-gated implementation-for-use phase — they are NOT validated against real M29/SQMD/ISP files, and no conformance claim is made.
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the batch close-out record (26).
