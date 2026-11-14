// P5.07 — Market & Programs workspace data (§20.1 item 8): pure mapping
// functions and the honesty statements, no I/O in the pure layer (the
// loader in MarketPrograms.tsx reads the tenant-scoped APIs).
//
// Honesty-first (the ledger P5.07 discipline): the workspace shows what
// EXISTS (the registry's market-region field, the commitments PLACEHOLDER,
// the declared synthetic objective) and states what does NOT exist as an
// explicit gap statement — no fabricated market signal, no invented
// adapter status, no demo data.
//
// What exists and what does not (the recorded inventory, decision record
// 07_31_market_plugins_admin_alarms.md):
// - EXISTS: `market_region` on the P1.17 portfolio record (a registry
//   field, free text — the target jurisdiction is D1, OPEN); the P2.17
//   portfolio-view commitments PLACEHOLDER (populated=false + note,
//   rendered verbatim, never upgraded); the declared scheduling objective
//   `SYNTHETIC_ENERGY_COST_DEVIATION_V1` with provenance SYNTHETIC
//   (P3.14, scheduling_objective.rs — mirrored in
//   components/stateVocabulary.ts) — explicitly not a market product
//   (D1/D2 OPEN).
// - DOES NOT EXIST: any market adapter or integration (0.x — the frozen
//   §15.3 event `MarketSignalReceived` has NO producer in this platform);
//   any program signal; any commitment record; any market delivery
//   evidence. All four are stated as gaps, never rendered as data.
import type { PortfolioDto } from '../api/types';
import type { PortfolioViewDto } from '../api/views';
import { DECLARED_OBJECTIVES } from '../components/stateVocabulary';

/** The registry market region: what the field is and what it is not. */
export const MARKET_REGION_STATEMENT =
  'market_region is a registry text field on the portfolio record ' +
  '(P1.17 create-portfolio contract). In this 0.x line its value is ' +
  'synthetic (the target jurisdiction is decision D1, OPEN): it is ' +
  'displayed as recorded, never interpreted as a connected market.';

/** The adapter-status gap statement (the ledger's required wording). */
export const MARKET_ADAPTER_STATEMENT =
  'No market adapter is configured — none exists in the 0.x simulation ' +
  'line. There is no market integration, no market connection, and no ' +
  'market adapter status to read; this panel therefore shows no adapter ' +
  'rather than a fabricated one (ledger P5.07 step 1: show "no market ' +
  'adapter configured" rather than fabricated signals).';

/** The program-signals gap statement. */
export const PROGRAM_SIGNALS_STATEMENT =
  'No program signal exists in this phase. The frozen §15.3 event ' +
  'vocabulary names `MarketSignalReceived`, but the platform has no ' +
  'producer for it in 0.x (no market integration; D1/D2 OPEN): no signal ' +
  'is received, none is displayed, and none is invented here. The ' +
  'program-signal element of §20.1 item 8 is an explicit empty state.';

/** The commitments statement (the PLACEHOLDER is rendered verbatim). */
export const COMMITMENTS_STATEMENT =
  'Commitments are the P2.17 registry PLACEHOLDER: the contract ships ' +
  'populated=false with its recorded note, which is rendered verbatim and ' +
  'never upgraded into a commitment figure. No commitment record exists ' +
  'in this phase (no market integration; D1 OPEN).';

/** The delivery-evidence statement: what exists, honestly. */
export const DELIVERY_EVIDENCE_STATEMENT =
  'Delivery evidence: the only scheduled/dispatched "program" in this ' +
  'phase is the synthetic scheduling objective below, executed against ' +
  'the simulator. There is no market delivery to evidence — no market ' +
  'obligation exists to fulfil. The dispatch/audit evidence chain for the ' +
  'SIMULATED commands lives in the Dispatch and Audit & Reports ' +
  'workspaces (read-only surfaces; acknowledgement is application-layer ' +
  'acceptance, never physical delivery).';

/** The declared synthetic objective summary (from the mirrored vocabulary;
 * exactly one objective is declared, provenance SYNTHETIC, D1/D2 OPEN). */
export function syntheticObjectiveSummary(): {
  name: string;
  provenance: string;
  termNames: string[];
  notAMarketProduct: string;
} {
  const objective = DECLARED_OBJECTIVES[0];
  if (objective === undefined) {
    // The mirror vocabulary is pinned by its own tests; failing honest
    // here would mean the mirror drifted — render nothing rather than an
    // invented objective.
    return {
      name: '(the declared-objective mirror drifted — no objective shown rather than an invented one)',
      provenance: 'UNKNOWN',
      termNames: [],
      notAMarketProduct: '',
    };
  }
  return {
    name: objective.name,
    provenance: objective.provenance,
    termNames: objective.terms.map((term) => term.name),
    notAMarketProduct: objective.notAMarketProduct,
  };
}

export interface MarketPortfolioRow {
  portfolio: PortfolioDto;
  view: PortfolioViewDto | null;
  viewError: string | null;
}

/** The honest market-region rows: one per portfolio, with the view error
 * kept (a forbidden view is an error, never an empty dataset). */
export function marketRegionRows(
  portfolios: PortfolioDto[],
  views: Array<PortfolioViewDto | null>,
  viewErrors: Array<string | null>,
): MarketPortfolioRow[] {
  return portfolios.map((portfolio, index) => ({
    portfolio,
    view: views[index] ?? null,
    viewError: viewErrors[index] ?? null,
  }));
}
