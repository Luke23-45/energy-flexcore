// P5.03 — the Portfolios workspace (report §20.1 item 2), on the Phase 2
// registry contracts (P2.17 views, dto.rs) plus the Phase 3 envelope
// records (dto_phase3.rs).
//
// Elements per §20.1 item 2 / ledger P5.03 step 1:
// - composition (the P2.17 view's site/asset counts; the portfolio's own
//   aggregation rules rendered VERBATIM — the console does not restate
//   them),
// - commitments: the P2.17 contract ships a PLACEHOLDER (populated=false +
//   note); the note is rendered verbatim — never upgraded into a
//   commitment list,
// - flexibility: the latest Phase 3 envelope record for the portfolio with
//   its envelope_version and computation time (P3.12). The computation
//   time travels inside the envelope_payload rendering (the P3.12
//   EnvelopeMetadata.computed_at); when the payload does not carry it, the
//   gap is stated explicitly rather than replaced by the viewer's clock.
//
// Role-specific visibility (P5.03 step 4): the session role comes from
// authenticated claims (RoleScopeLine); visibility is enforced by the
// tenant-scoped API — a forbidden read renders as an honest error block,
// never as an empty dataset (tested with two roles in Portfolios.test.tsx).
import type { JSX } from 'react';
import type { ApiClient } from '../api/client';
import type { EnvelopeRecordDto, PortfolioDto } from '../api/types';
import { listFlexibilityEnvelopes } from '../api/flexibility';
import {
  fetchPortfolioView,
  fetchPortfolios,
  type PortfolioViewDto,
} from '../api/views';
import {
  ErrorBlock,
  HonestEmpty,
  LoadingLine,
  RoleScopeLine,
  errorText,
  useWorkspaceData,
  type LoadState,
} from '../components/workspaceData';
import { FreshnessLine } from '../components/ValueType';
import { formatUnixMillis } from '../components/format';

export interface PortfolioRow {
  portfolio: PortfolioDto;
  view: PortfolioViewDto | null;
  viewError: string | null;
  /** The latest envelope record for this portfolio (max horizon start). */
  envelope: EnvelopeRecordDto | null;
}

export interface PortfoliosData {
  rows: PortfolioRow[];
  envelopeCount: number;
}

/**
 * Extracts the envelope computation timestamp from the verbatim P3.12
 * payload rendering. Purely defensive: the payload is an opaque
 * serde_json::Value on the wire, so the field is read only when it is
 * actually present as a string — never defaulted.
 */
export function envelopeComputedAt(payload: unknown): string | null {
  if (payload !== null && typeof payload === 'object' && 'computed_at' in payload) {
    const value = (payload as { computed_at?: unknown }).computed_at;
    if (typeof value === 'string') return value;
  }
  return null;
}

/** The latest envelope record for one portfolio (max horizon start). */
export function latestEnvelopeFor(
  portfolioId: string,
  envelopes: EnvelopeRecordDto[],
): EnvelopeRecordDto | null {
  let latest: EnvelopeRecordDto | null = null;
  for (const envelope of envelopes) {
    if (envelope.portfolio_id !== portfolioId) continue;
    if (latest === null || envelope.horizon_start_millis > latest.horizon_start_millis) {
      latest = envelope;
    }
  }
  return latest;
}

/**
 * The Portfolios read: the tenant-scoped portfolio list + one P2.17 view
 * read per portfolio + the Phase 3 envelope page. A failed view read is
 * kept as an honest per-row error, never dropped.
 */
export async function loadPortfolios(client: ApiClient): Promise<PortfoliosData> {
  const [portfolios, envelopesPage] = await Promise.all([
    fetchPortfolios(client),
    listFlexibilityEnvelopes(undefined, client),
  ]);
  const rows = await Promise.all(
    portfolios.map(async (portfolio) => {
      try {
        const view = await fetchPortfolioView(portfolio.id, client);
        return {
          portfolio,
          view,
          viewError: null,
          envelope: latestEnvelopeFor(portfolio.id, envelopesPage.data),
        };
      } catch (error: unknown) {
        return {
          portfolio,
          view: null,
          viewError: errorText(error),
          envelope: latestEnvelopeFor(portfolio.id, envelopesPage.data),
        };
      }
    }),
  );
  return { rows, envelopeCount: envelopesPage.data.length };
}

export function PortfoliosWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadPortfolios);
  return <PortfoliosView state={state} />;
}

export function PortfoliosView({ state }: { state: LoadState<PortfoliosData> }): JSX.Element {
  return (
    <section aria-label="Portfolios">
      <h2>Portfolios</h2>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the portfolios" />}
      {state.status === 'error' && <ErrorBlock what="The portfolio registry" error={state.error} />}
      {state.status === 'ready' && <PortfoliosReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function PortfoliosReady({ data, readAt }: { data: PortfoliosData; readAt: string }): JSX.Element {
  if (data.rows.length === 0) {
    return (
      <HonestEmpty
        what="Portfolios"
        reason="The tenant-scoped portfolio read returned no portfolios. No portfolio row is fabricated; register one through the registry API (Phase 1/2) to see it here."
      />
    );
  }
  return (
    <>
      {data.rows.map((row) => (
        <PortfolioSection key={row.portfolio.id} row={row} readAt={readAt} />
      ))}
      <p>
        Envelope records consulted: {data.envelopeCount} (the Phase 3 envelope
        page as returned by the tenant-scoped read).
      </p>
    </>
  );
}

function PortfolioSection({ row, readAt }: { row: PortfolioRow; readAt: string }): JSX.Element {
  const { portfolio, view, viewError, envelope } = row;
  const computedAt = envelope ? envelopeComputedAt(envelope.envelope_payload) : null;
  return (
    <article data-testid={`portfolio-${portfolio.id}`}>
      <h3>
        {portfolio.name} <small>({portfolio.id})</small>
      </h3>
      <dl>
        <dt>Operator</dt>
        <dd>{portfolio.operator}</dd>
        <dt>Market region</dt>
        <dd>{portfolio.market_region}</dd>
        <dt>Aggregation rules (registry, verbatim)</dt>
        <dd>
          <code>{portfolio.aggregation_rules}</code>
        </dd>
        <dt>Created</dt>
        <dd>{portfolio.created_at}</dd>
      </dl>

      {viewError !== null ? (
        <div role="note" data-testid="portfolio-view-error">
          <p>
            The P2.17 portfolio view could not be read for this portfolio; its
            composition is NOT shown rather than guessed:
          </p>
          <p>
            API error (verbatim): <code>{viewError}</code>
          </p>
        </div>
      ) : (
        view && (
          <>
            <h4>Composition (P2.17 view)</h4>
            <p>
              {view.composition.site_count} site(s), {view.composition.asset_count}{' '}
              asset(s).
            </p>
            <p>
              <FreshnessLine readAt={readAt} /> (the P2.17 view record carries
              no per-field event time; the freshness anchor is this console
              read, §20.2 — the record values are registry facts, not
              telemetry, and are never typed measured/estimated/simulated.)
            </p>
            <h4>Commitments</h4>
            <p data-testid="commitments-placeholder">
              {view.commitments.populated
                ? 'Populated (per the registry view).'
                : 'Commitments: PLACEHOLDER — not populated in Phase 2 (registry note, verbatim): '}
              {view.commitments.populated ? '' : <code>{view.commitments.note}</code>}
            </p>
          </>
        )
      )}

      <h4>Flexibility envelope (Phase 3, P3.12)</h4>
      {envelope === null ? (
        <HonestEmpty
          what="Flexibility envelope"
          reason="No Phase 3 envelope record exists for this portfolio in the tenant-scoped read. No envelope figure is fabricated."
        />
      ) : (
        <div data-testid="envelope-summary">
          <p>
            Envelope version: <code>{envelope.envelope_version}</code>; horizon{' '}
            {formatUnixMillis(envelope.horizon_start_millis)} —{' '}
            {formatUnixMillis(envelope.horizon_end_millis)} (
            {envelope.interval_count} interval(s) of{' '}
            {envelope.interval_millis} ms); covering{' '}
            {envelope.covered_site_ids.length} site(s).
          </p>
          <p data-testid="envelope-computed-at">
            {computedAt !== null
              ? `Computation time (from the envelope record payload): ${computedAt}`
              : 'Computation time: not present in the envelope record payload — shown as absent rather than replaced by the viewer\'s clock.'}
          </p>
          <p>
            <FreshnessLine readAt={readAt} />
          </p>
        </div>
      )}
    </article>
  );
}
