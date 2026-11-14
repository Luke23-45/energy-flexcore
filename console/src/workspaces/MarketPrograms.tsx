// P5.07 — the Market & Programs workspace (report §20.1 item 8, §25.2
// D1/D5): built HONESTLY — it shows what exists (the registry's
// market-region field, the P2.17 commitments PLACEHOLDER, the declared
// synthetic objective) and states what does not exist as an explicit gap
// statement (no market adapter, no program signals, no market delivery
// evidence). No fabricated market panel anywhere (ledger P5.07).
//
// Every element carries its data-freshness stamp (§20.2); the workspace
// renders tenant-scoped API reads only (no tenant selector on the wire,
// §16.2) through the injectable client pattern (tests pass a fake).
import type { JSX } from 'react';
import type { ApiClient } from '../api/client';
import { fetchPortfolios, fetchPortfolioView } from '../api/views';
import {
  ErrorBlock,
  HonestEmpty,
  LoadingLine,
  RoleScopeLine,
  errorText,
  useWorkspaceData,
  type LoadState,
} from '../components/workspaceData';
import {
  COMMITMENTS_STATEMENT,
  DELIVERY_EVIDENCE_STATEMENT,
  MARKET_ADAPTER_STATEMENT,
  MARKET_REGION_STATEMENT,
  PROGRAM_SIGNALS_STATEMENT,
  marketRegionRows,
  syntheticObjectiveSummary,
  type MarketPortfolioRow,
} from './marketData';

export interface MarketProgramsData {
  rows: MarketPortfolioRow[];
}

/**
 * The tenant-scoped read: portfolios plus the P2.17 portfolio views (the
 * commitments PLACEHOLDER travels in the view). A failed view read is kept
 * as an honest per-row error, never dropped (one view read per portfolio).
 */
export async function loadMarketPrograms(client: ApiClient): Promise<MarketProgramsData> {
  const portfolios = await fetchPortfolios(client);
  const rows = await Promise.all(
    portfolios.map(async (portfolio): Promise<MarketPortfolioRow> => {
      try {
        const view = await fetchPortfolioView(portfolio.id, client);
        return { portfolio, view, viewError: null };
      } catch (error: unknown) {
        return { portfolio, view: null, viewError: errorText(error) };
      }
    }),
  );
  return { rows };
}

/** The workspace shell the router renders (P5.01 registry pattern; the
 * export name is unchanged from the P5.01 stub). */
export function MarketProgramsWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadMarketPrograms);
  return <MarketProgramsView state={state} />;
}

export function MarketProgramsView({ state }: { state: LoadState<MarketProgramsData> }): JSX.Element {
  return (
    <section aria-label="Market & Programs">
      <h2>Market &amp; Programs</h2>
      <p className="honesty-banner" role="note" data-testid="market-honesty-banner">
        This workspace is built HONESTLY (ledger P5.07): it shows what
        exists and states what does not exist yet — no fabricated market
        signal, adapter, or commitment appears anywhere on it.
      </p>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the market & programs data" />}
      {state.status === 'error' && <ErrorBlock what="The portfolio registry" error={state.error} />}
      {state.status === 'ready' && <MarketProgramsReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function MarketProgramsReady({ data, readAt }: { data: MarketProgramsData; readAt: string }): JSX.Element {
  const objective = syntheticObjectiveSummary();
  const readableViews = data.rows.filter((row) => row.view !== null);
  return (
    <>
      <h3>Market region (registry, synthetic — D1 OPEN)</h3>
      {data.rows.length === 0 ? (
        <HonestEmpty
          what="Market region"
          reason="No portfolio was returned by the tenant-scoped read, so no market_region value exists to show. No portfolio and no region is fabricated."
        />
      ) : (
        <>
          <table>
            <caption>
              The registry `market_region` field per portfolio. Read at{' '}
              {readAt}. {MARKET_REGION_STATEMENT}
            </caption>
            <thead>
              <tr>
                <th scope="col">Portfolio</th>
                <th scope="col">market_region (verbatim)</th>
                <th scope="col">Operator</th>
              </tr>
            </thead>
            <tbody>
              {data.rows.map((row) => (
                <tr key={row.portfolio.id}>
                  <td>{row.portfolio.name}</td>
                  <td>
                    <code>{row.portfolio.market_region}</code>
                  </td>
                  <td>{row.portfolio.operator}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {data.rows.some((row) => row.viewError !== null) && (
            <div role="note" data-testid="portfolio-view-failures">
              <p>
                Some portfolio views could not be read (shown, never dropped);
                the commitments PLACEHOLDER comes from the view:
              </p>
              <ul>
                {data.rows
                  .filter((row) => row.viewError !== null)
                  .map((row) => (
                    <li key={row.portfolio.id}>
                      {row.portfolio.id}: <code>{row.viewError}</code>
                    </li>
                  ))}
              </ul>
            </div>
          )}
        </>
      )}

      <h3>Program signals</h3>
      <HonestEmpty what="Program signals" reason={PROGRAM_SIGNALS_STATEMENT} testId="program-signals-gap" />

      <h3>Commitments (P2.17 PLACEHOLDER, verbatim)</h3>
      <p>{COMMITMENTS_STATEMENT}</p>
      {readableViews.length === 0 ? (
        <HonestEmpty
          what="Commitments"
          reason="No portfolio view was readable, so the P2.17 commitments PLACEHOLDER cannot be rendered from a record. No commitment value is invented."
        />
      ) : (
        <ul>
          {readableViews.map((row) => (
            <li key={row.portfolio.id} data-testid="commitments-placeholder">
              {row.view?.commitments.populated
                ? 'Commitments (populated — the Phase 2 registry contract ships populated=false; if a record ever claims populated, verify the contract): '
                : 'Commitments: PLACEHOLDER — not populated in Phase 2 (registry note, verbatim): '}
              <code>{row.view?.commitments.note}</code>
            </li>
          ))}
        </ul>
      )}

      <h3>Adapter status</h3>
      <HonestEmpty what="Market adapter" reason={MARKET_ADAPTER_STATEMENT} testId="market-adapter-gap" />

      <h3>The declared scheduling objective (the only configured "program", synthetic)</h3>
      <div data-testid="synthetic-objective">
        <p>
          Declared objective: <code>{objective.name}</code> — provenance{' '}
          <code>{objective.provenance}</code>. This is the ONE objective in
          the declared set (P3.14, scheduling_objective.rs, mirrored in the
          console vocabulary): selection means choosing among the declared
          set, never free-text entry. Term names:{' '}
          {objective.termNames.length > 0 ? objective.termNames.join(', ') : '(none rendered — mirror drift)'}.{' '}
          <code>{objective.notAMarketProduct}</code>
        </p>
        <p>
          Every value this objective produces is SIMULATED and is labelled
          simulated wherever it appears (§20.2; the Schedules workspace
          renders the §12.6 records with their full terms and weight units).
        </p>
      </div>

      <h3>Delivery evidence</h3>
      <HonestEmpty what="Market delivery evidence" reason={DELIVERY_EVIDENCE_STATEMENT} testId="delivery-evidence-gap" />
    </>
  );
}
