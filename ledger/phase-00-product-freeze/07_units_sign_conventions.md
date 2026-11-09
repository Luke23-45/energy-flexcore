# P0.07 — Canonical units and sign conventions

| Field | Value |
|---|---|
| Subtask | P0.07 — Freeze canonical units and sign conventions |
| Report trace | §10.2 (normative source), §11.2 (BESS net power), §11.3 (P–Q envelope), §0 (revision note); foundations specification §6 (§6.2 governs on conflict) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`; `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.pdf` / `.docx` |
| Status | Draft for Stage 0 review |
| Consumers | Phase 1 (API contracts) and Phase 2 (telemetry model) must cite this table by ID; no re-definition allowed (ledger P0.07) |

---

## 1. Canonical internal units (§10.2, normative)

One canonical convention is defined and enforced at every adapter boundary (§10.2).

| Quantity | Canonical unit | Contract notes |
|---|---|---|
| Active power P | MW | Sign rule in §2 below |
| Reactive power Q | MVAr | Sign rule in §3 below |
| Apparent power S | MVA | Used in P–Q envelope bounds (§11.3) |
| Energy E | MWh | SOC is not energy; see §4 |
| Voltage | kV at the nominal base in the network model; per-unit in the solver representation | Base declared explicitly in every solver representation |
| Current | A or per-unit on an explicit base | With phase identification |
| Frequency | Hz | |
| State of charge (SOC) | Fraction in [0,1] | Single canonical representation at the API boundary; see §4 |

No kW, kWh, or other scaled unit appears in any row of this contract table. Scaled units are display-only (§5).

## 2. Active-power sign rule (§10.2)

Sign conventions are stated at the asset/site point of connection:

- **P > 0 means export/injection into the grid** at the point of connection.
- **P < 0 means import/consumption from the grid.**

## 3. Reactive-power sign rule (§10.2)

Reactive-power sign is **not** a universal constant:

1. Each grid model and each device profile declares whether **Q > 0** means injection (generator convention) or consumption (load convention).
2. Each adapter records the mapping it applies.
3. Platform contracts carry the declared convention explicitly rather than assuming one.

No component, adapter, or document may assume a universal Q sign. This rule is reinforced by foundations §6.2 ("never inferred from sign alone").

## 4. SOC representation rule (§10.2)

- SOC is a **fraction in [0,1]** — the single canonical representation at the API boundary.
- Percentages exist only as display or vendor-source values and are **converted at the adapter**. A percent value is never persisted as canonical and never appears in an API contract as canonical SOC.

## 5. Adapter rule (§10.2)

- Adapters convert vendor/protocol units into the canonical units and record both the **unit mapping** and the **sign mapping profile** they apply. Do not rely on implicit unit or sign assumptions.
- Persisted values and API contracts use the canonical units of §1.
- The UI may display kW/kWh and other scaled units, but must not persist them as canonical and must not emit them in a canonical API contract.

## 6. BESS net-power relation (§11.2)

Let `P_dis` denote discharge power and `P_ch` denote charge power, both non-negative:

```text
P_net = P_dis − P_ch        with P_dis ≥ 0 and P_ch ≥ 0
```

`P_net` is the platform's net active-power export at the point of connection and therefore follows the sign rule of §2: `P_net > 0` is export, `P_net < 0` is import. Simultaneous charge and discharge prohibition is an optimization-formulation concern (§11.2: valid mixed-integer mode formulation or justified convex relaxation plus post-solution checks — never labelled a continuous SOCP); it is recorded here as vocabulary only, implementation is Phase 3.

## 7. Foundations §6 cross-check (P0.07 step 7, feeds the P0.26 table)

Line-by-line comparison of §1–§6 against foundations §6.2 ("Units and sign conventions", MANDATORY CONVENTION). Outcome: **zero unresolved conflicts**.

| This contract | Foundations §6.2 | Outcome |
|---|---|---|
| Active power MW; P > 0 = export at POI (§1, §2) | "MW. Positive = injection/export to grid at the POI." | Consistent |
| Reactive power MVAr; sign declared per grid model and device profile (§1, §3) | "MVAr. Positive/negative direction is explicitly configured to match the network model and device convention; never inferred from sign alone." | Consistent; the report's per-adapter recording and platform-contract carrying are refinements, not conflicts |
| Apparent power MVA (§1) | No apparent-power row in the §6.2 table (§6.3 names "apparent-power rating" as a capability element) | Foundations silent; report addition, no conflict |
| Energy MWh (§1) | "MWh." | Consistent |
| Voltage kV nominal base / per-unit solver (§1) | "kV at nominal base in network model; per-unit in solver representation." | Consistent; the report's explicitly declared base is a refinement |
| Current A or per-unit on explicit base (§1) | "A or per-unit on explicit base." | Consistent; the report's phase identification is a refinement |
| Frequency Hz (§1) | No frequency row in the §6.2 table | Foundations silent; report addition, no conflict |
| SOC fraction [0,1] at the API boundary (§4) | "Fraction in [0,1] or percent, but one representation is selected per API and stated explicitly." | **Known divergence — see §8** |
| Adapter conversion and recording rule (§5) | "No implicit unit conversions are allowed at protocol boundaries"; "stores a unit with every external measurement" | Consistent and mutually reinforcing |

Foundations §6.2 also carries a Time row ("UTC epoch/time plus original timezone metadata where relevant. Simulation time is separate from wall-clock time."). This is cross-checked in the P0.09 time model, which records it; no conflict with report §10.3.

## 8. SOC representation divergence (recorded for the P0.26 table)

- Foundations §6.2 permits "fraction in [0,1] or percent, but one representation is selected per API and stated explicitly".
- Report §10.2 mandates **fraction [0,1]** at the API boundary.
- The report rule is the **stricter selection that foundations §6.2 permits** — a **compatible refinement, not a conflict**. The freeze adopts fraction [0,1] as the single selected representation and records it as such in the P0.26 cross-check table.

## 9. Acceptance criteria coverage (ledger P0.07)

- [x] All seven canonical units are listed (§1); no kW/kWh appears in any contract row.
- [x] P > 0 = export is stated with the point-of-connection qualifier (§2).
- [x] The reactive-power rule states per-model/per-profile declaration and per-adapter mapping; no universal Q sign is assumed anywhere (§3).
- [x] SOC [0,1] at the API boundary is stated; percentage handling is adapter-only (§4).
- [x] Foundations §6 cross-check is recorded, with zero unresolved conflicts (§7).
- [x] The SOC representation divergence is recorded as a compatible refinement for the P0.26 table, not as a conflict (§8).
