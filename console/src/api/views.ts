// P5.03 — Phase 2 registry-view DTO mirrors (P2.17 shapes, dto.rs) + typed
// fetch wrappers over the P5.01 registry client.
//
// SHAPE-MIRROR NOTE (read before editing): these types mirror
// apps/control-plane/src/dto.rs field-for-field using the WIRE names
// (snake_case) — see types.ts for the drift discipline. The authoritative
// shapes are pinned by the apps/control-plane contract tests and the
// generated OpenAPI; the P5.01 step-4 drift test lands with the first
// install.
import type {
  AssetDto,
  CurrentStateDto,
  DataEnvelope,
  PortfolioDto,
  SiteDto,
} from './types';
import { getAssetView, getPortfolioView, getSiteView, listAssets, listPortfolios, listSites } from './registry';
import { api, type ApiClient } from './client';

/** The commitments placeholder of the portfolio view (dto.rs). */
export interface CommitmentsPlaceholderDto {
  populated: boolean;
  note: string;
}

/** The composition summary of the portfolio view (dto.rs). */
export interface PortfolioCompositionDto {
  site_count: number;
  asset_count: number;
}

/** The P2.17 portfolio view (dto.rs PortfolioViewDto). */
export interface PortfolioViewDto {
  id: string;
  tenant_id: string;
  name: string;
  description?: string;
  operator: string;
  market_region: string;
  aggregation_rules: string;
  composition: PortfolioCompositionDto;
  commitments: CommitmentsPlaceholderDto;
  created_at: string;
}

/**
 * The P2.17 edge-connectivity report — HONEST BY CONTRACT (dto.rs): Phase 2
 * has no site integration (first site integration is v0.4.0), so the status
 * is `simulator` when the site carries an edge identity and
 * `not_connected` otherwise. `live` is always `false` in Phase 2; the API
 * never presents a simulated edge as live. The console renders these
 * fields verbatim and never upgrades them.
 */
export interface EdgeConnectivityDto {
  status: string;
  edge_identity: string;
  live: boolean;
}

/** The overview summary of the site view (dto.rs). */
export interface SiteOverviewDto {
  asset_count: number;
}

/** The P2.17 site view (dto.rs SiteViewDto). */
export interface SiteViewDto {
  id: string;
  portfolio_id: string;
  tenant_id: string;
  name: string;
  description?: string;
  location: string;
  timezone: string;
  point_of_common_coupling: string;
  grid_model_reference: string;
  edge_connectivity: EdgeConnectivityDto;
  overview: SiteOverviewDto;
  created_at: string;
}

/** The stored limits of the asset view (dto.rs): rated magnitudes the
 * registry holds — NOT runtime capability envelopes, which are Phase 3. */
export interface AssetLimitsDto {
  rated_power_mw: number;
  rated_energy_mwh?: number;
}

/** The P2.17 asset view (dto.rs AssetViewDto). `maintenance_outage_status`
 * is the stored registry lifecycle field (`operational` | `deactivated`),
 * not a live maintenance system's state. */
export interface AssetViewDto {
  id: string;
  site_id: string;
  tenant_id: string;
  name: string;
  description?: string;
  asset_type: string;
  vendor_model: string;
  capability_profile: string;
  limits: AssetLimitsDto;
  lifecycle_status: string;
  maintenance_outage_status: string;
  current_state?: CurrentStateDto | null;
  created_at: string;
}

// --- typed fetch wrappers (defaults: the module-singleton client) -----------

export function fetchPortfolios(
  client: ApiClient = api,
): Promise<PortfolioDto[]> {
  return listPortfolios(undefined, client).then((page) => page.data);
}

export function fetchSites(client: ApiClient = api): Promise<SiteDto[]> {
  return listSites(undefined, client).then((page) => page.data);
}

export function fetchAssets(client: ApiClient = api): Promise<AssetDto[]> {
  return listAssets(undefined, client).then((page) => page.data);
}

export function fetchPortfolioView(
  portfolioId: string,
  client: ApiClient = api,
): Promise<PortfolioViewDto> {
  return getPortfolioView(portfolioId, client).then(
    (envelope) => envelope.data as PortfolioViewDto,
  );
}

export function fetchSiteView(siteId: string, client: ApiClient = api): Promise<SiteViewDto> {
  return getSiteView(siteId, client).then((envelope) => envelope.data as SiteViewDto);
}

export function fetchAssetView(assetId: string, client: ApiClient = api): Promise<AssetViewDto> {
  return getAssetView(assetId, client).then((envelope) => envelope.data as AssetViewDto);
}

/** Re-exported for callers that want the raw envelope shape. */
export type { DataEnvelope };
