# P17.17 — OCPI/OICP roaming recorded OUT OF SCOPE (decision record): the depot-fleet rationale, the concrete exclusions, and the revisit conditions

| Field | Value |
|---|---|
| Subtask | P17.17 (OCPI/OICP out-of-scope record — roaming, OCPI 2.3.0 [S-134], OICP, recorded out of scope with revisit conditions (public CPO network operation), matching gap analysis §8) |
| Wave / agent | Phase 17 wave 2, Agent B (P17.16–P17.18) |
| Deliverable class | RECORD-ONLY (plan §1) — a DECISION RECORD registering an out-of-scope verdict. This record closes nothing that was open (the exclusion was already recorded at planning, gap analysis §8 and phase-17 context §5.1); it compiles the verdict, its rationale, and its revisit conditions into the phase record set. |
| Status | OUT-OF-SCOPE RECORD — no roaming capability exists, is planned, or is promised. **No charge point is connected, no roaming message was ever exchanged, no roaming partner, platform, or tariff was ever touched.** No conformance claim of any kind. |
| Sources | [S-134] OCPI 2.3.0 (evroaming.org, the 2025-02 PDF per the gap-analysis §11 entry) — cited from the program's recorded source list, NOT consulted page-by-page: **every OCPI object-class and module-level statement below is SECONDARY-SOURCED and UNVERIFIED against the [S-134] text**. OICP (Hubject) is named in gap analysis §7 and §8 WITHOUT a dedicated source ID in §11 — no OICP protocol-specific claim is made anywhere in this record, and none can be from this program's evidence base. |
| Files touched | This record only. |

## 1. The recorded decision

**OCPI/OICP roaming is out of scope for Phase 17 and for the FlexCore 0.x
line.** This matches the gap analysis §8 row verbatim in substance:

| Item | Why deferred (gap analysis §8) | Revisit when (gap analysis §8) |
|---|---|---|
| OCPI/OICP roaming | Depot-fleet VPP controls its own chargers via OCPP | If operating a public CPO network |

The gap analysis §8 row cites [S-136] while §11 defines [S-134] as the OCPI
2.3.0 source — a citation-id drift INSIDE the gap analysis (§7 line 200 and
§8 line 241 both carry [S-136] where the §11 definitions give OCPI = [S-134]
and Eichrecht guide = [S-136]). The phase-17 plan and ledger cite [S-134],
which is the §11-defined OCPI 2.3.0 id; this record follows plan/ledger/§11.
The drift is RECORDED as an audit observation in the close record (§3 of the
close); the gap analysis itself is outside this agent's edit scope and was
not touched.

## 2. What "out of scope" concretely excludes

| Excluded | Statement |
|---|---|
| Any OCPI module or endpoint | No OCPI module, no OCPI endpoint, no OCPI client or server exists in the workspace and none is planned for this phase. The OCPI object classes one would expect a roaming implementation to exchange (locations, tariffs, sessions, charge-detail records, tokens) are named UNVERIFIED against [S-134] — as CLASS examples of what is excluded, never as a specification |
| Any OICP interface | OICP carries no source id in this program's evidence base; its exclusion is recorded by NAME only (gap analysis §7/§8). No OICP-specific protocol, message, or process claim is made anywhere |
| Roaming-party roles | FlexCore plays neither the eMSP role (roaming access for a fleet's drivers on third-party networks) nor the public-CPO role (third-party drivers charging on FlexCore-operated infrastructure) in any in-scope deployment |
| Token/authorization via roaming platforms | No roaming-token whitelisting, no remote authorization dependency; charge-point authorization in scope is the depot operator's own (P17.03's identity proofing covers the CHARGE POINT's identity toward the CSMS, never a driver's roaming credential) |
| Tariff/CDR exchange | No tariff exchange, no CDR generation or consumption for roaming settlement; the P16 settlement surface is between the VPP and MARKET parties, entirely disjoint from EV roaming settlement |

## 3. Why (the rationale, matching the gap analysis)

The phase-17 scope is a depot fleet operated for its owner: the fleet
operator controls its own chargers and talks to them over the OCPP CSMS
adapter (records 01–05). Every vehicle in scope is a fleet vehicle with a
fleet identity; the user-override policy (record 25) and the departure
constraints (record 24) assume OWN-fleet drivers and OWN-fleet calendars. No
third party charges on the infrastructure, and no fleet vehicle needs
third-party-network access through FlexCore. Roaming exists to interconnect
PUBLIC charging networks with eMSPs for ad-hoc and contract charging — a
commercial shape no in-scope deployment has.

## 4. The revisit conditions (explicit, matching the gap-analysis trigger)

| # | Condition | What a revisit would mean |
|---|---|---|
| 1 | A FlexCore deployment operates a PUBLICLY ACCESSIBLE charging network (a CPO role) | The gap-analysis §8 trigger fires verbatim. Roaming becomes commercially relevant; a new phase/record scope assesses OCPI (pinned to the then-current version — 2.3.0 is the recorded version today, re-verified at revisit) and OICP against that deployment. Note: this is the SAME condition as P17.16's AFIR verdict trigger #1 (record 61 §5.1) — one trigger, two records, both fire together |
| 2 | A fleet deployment requires its drivers to charge on THIRD-PARTY networks under the operator's own eMSP role | Same treatment: a new scope assessment; the vendor-agnostic port discipline (P17-E-3 DEFAULT) suggests the shape — a roaming PORT, never a hard-coded platform |
| 3 | A D-jurisdiction / market-entry decision changes the commercial shape to include public charging services | Roaming re-enters with the jurisdiction decision (D1-class), never before |

None of the three is currently met: no site is named, no deployment exists,
no public network is operated, no eMSP role is played.

## 5. Honest boundaries

1. This record registers an EXCLUSION — it promises nothing and excludes
   nothing that exists (no roaming code was ever written to exclude).
2. Every OCPI-specific statement is UNVERIFIED against [S-134]; OICP
   carries no source in this program, so no OICP-specific claim exists here
   at all.
3. The exclusion is scoped to the 0.x line and the phase-17 build list; the
   revisit conditions above are the recorded path back in — a revisit is a
   NEW scope decision (plan-level), never a silent widening of this phase.
4. No conformance, qualification, or interoperability claim of any kind;
   no roaming platform or partner is named, contacted, or evaluated.
