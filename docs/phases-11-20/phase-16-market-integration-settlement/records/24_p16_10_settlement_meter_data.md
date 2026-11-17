# P16.10 — Settlement-quality meter data: accuracy classes, meter-device-resource mapping, dual streams

| Field | Value |
|---|---|
| Subtask | P16.10 (ANSI C12.19/C12.20 alignment — class 0.2/0.5 expectations [S-31][S-33]; meter-device-resource mapping; dual streams — settlement-interval meter data and seconds-level telemetry kept DISTINCT, never silently merged [S-59][S-104]) |
| Wave / agent | Phase 16 wave 1, Agent B |
| Status | WRITTEN-UNRUN: class records, mapping types, stream bindings, and the boundary gate are written, never compiled, never executed. No meter is certified; no metering installation is claimed conformant; no meter data exists in this program. |
| Sources | [S-31] (NYISO M-25 metering requirements manual — the settlement-grade metering posture class), [S-33] (ANSI C12.20 accuracy classes — the 0.2/0.5 class expectations), [S-59] (the RMI metering framework — metering as the settlement evidence base), [S-104] (ISO-NE Order 2222 metering & telemetry checklist) |
| Files touched | `crates/flexcore-application/src/settlement_meter_data.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line only) |

## 1. What landed

| Item | Content |
|---|---|
| `MeterAccuracyClass` + `MeterAccuracyClassRecord` | The class-0.2 / class-0.5 expectations [S-33] carried as declared labels, with a mandatory recorded source: the class is the METER's documented property, referenced — never a platform claim ([S-31] posture class). An unsourced class record is unconstructable |
| `MeterDeviceResourceMapping` + `exact_meter_lookup` | The meter ↔ device ↔ jurisdiction-resource mapping with EXACT string-equality linkage (the P8.11/P15.15 discipline): a near-miss resolves to NOTHING and FLAGS, never matches; the resource vocabulary is the jurisdiction's, carried verbatim |
| `SettlementIntervalMeterData` | The settlement stream's record type — deliberately DISTINCT from the Phase-2 `NormalizedTelemetry`: no `From`/`Into` in either direction, exact decimal quantity, verbatim basis and source quality flag (the six-state telemetry vocabulary is NOT applied here — silently re-labeling source flags would be exactly the forbidden merge) |
| `MeterStreamBinding` | One binding per stream kind per meter. A SETTLEMENT binding must declare a cadence ≥ the 5-minute settlement class (300,000 ms); a sub-5-minute settlement cadence is refused (`SETTLEMENT_METER_CADENCE_BELOW_CLASS`) — seconds-level data cannot bind as settlement data |
| `refuse_telemetry_as_settlement` | The explicit fail-closed gate at the stream boundary: ANY attempt to pass a `NormalizedTelemetry` where settlement data is expected is refused with `SETTLEMENT_METER_STREAM_KIND_MISMATCH`, always, by type. The mirror direction needs no gate — the conversion path is structurally absent |
| Tests (written-unrun) | Inline suite: statement pins, class labels, exact-vs-near-miss lookups, cadence validation, sub-5-minute settlement binding refusal, the always-refusing boundary gate |

## 2. Recorded decisions

1. **The platform never certifies metering** (`PLATFORM_NEVER_CERTIFIES_METERING_STATEMENT`, pinned by tests): an accuracy-class record is a REFERENCE to the meter's documentation; certification belongs to the meter and its testing authority. No meter is certified here.
2. **Dual streams, two types, no conversion path** (`DUAL_STREAMS_NEVER_MERGED_STATEMENT`, pinned by tests, [S-59][S-104]): settlement-interval meter data and seconds-level telemetry never merge; the only permitted interaction is the explicit always-refusing gate, so the attempt is NAMED, never absorbed (the non-silencing rule applied at the stream boundary).
3. **Settlement resolution is validated, never assumed**: interval lengths must match the declared binding cadence exactly; re-binning is refused, never performed.
4. **The settlement stream keeps the source's quality vocabulary verbatim** — the canonical six-state telemetry vocabulary is not applied to settlement records, because mapping source meter flags into telemetry states would silently transform settlement evidence (the same non-merge discipline as decision 2).
5. **Metered settlement quantities are exact decimals** (`fixed_decimal`) — settlement quantities are settlement-adjacent records, never floats.

## 3. Honest boundaries

1. No meter is certified; no metering installation is claimed conformant to any ANSI/NYISO/ISO-NE class or checklist; the class labels are declared references, not met requirements.
2. No meter data exists in this program: the simulator produces telemetry, not settlement meter data; every fixture value is synthetic.
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the batch close-out record (26).
