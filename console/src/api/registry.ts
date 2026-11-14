// P5.01 — Phase 1/2 registry client (routes.rs + routes_phase2.rs path
// groups). SHAPE-MIRROR: see types.ts (drift note). Read-only: the console
// creates/deletes nothing in the registry in this scaffold; the Phase 2
// mutation surface (update/deactivate/delete) is wired only if a later
// wave's workspace needs it, with the API's own authorization as the real
// gate (the console hiding a button is not enforcement, P5.26).
import { api, type ApiClient } from './client';
import type {
  AssetDto,
  DataEnvelope,
  ListEnvelope,
  PortfolioDto,
  SiteDto,
  TenantDto,
} from './types';

const PAGE_PARAM_LIMIT = 'limit';

// Every function takes an optional trailing client (default: the module
// singleton) so tests can inject a recording client and pin the exact
// wire paths — contract drift fails the test run.

export function listTenants(
  client: ApiClient = api,
): Promise<ListEnvelope<TenantDto>> {
  return client.get('/api/v1/tenants');
}

export function getTenant(
  tenantId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<TenantDto>> {
  return client.get(`/api/v1/tenants/${encodeURIComponent(tenantId)}`);
}

export function listPortfolios(
  limit?: number,
  client: ApiClient = api,
): Promise<ListEnvelope<PortfolioDto>> {
  const query = limit === undefined ? '' : `?${PAGE_PARAM_LIMIT}=${limit}`;
  return client.get(`/api/v1/portfolios${query}`);
}

export function getPortfolio(
  portfolioId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<PortfolioDto>> {
  return client.get(`/api/v1/portfolios/${encodeURIComponent(portfolioId)}`);
}

export function listSites(limit?: number, client: ApiClient = api): Promise<ListEnvelope<SiteDto>> {
  const query = limit === undefined ? '' : `?${PAGE_PARAM_LIMIT}=${limit}`;
  return client.get(`/api/v1/sites${query}`);
}

export function getSite(
  siteId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<SiteDto>> {
  return client.get(`/api/v1/sites/${encodeURIComponent(siteId)}`);
}

export function listAssets(limit?: number, client: ApiClient = api): Promise<ListEnvelope<AssetDto>> {
  const query = limit === undefined ? '' : `?${PAGE_PARAM_LIMIT}=${limit}`;
  return client.get(`/api/v1/assets${query}`);
}

export function getAsset(
  assetId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<AssetDto>> {
  return client.get(`/api/v1/assets/${encodeURIComponent(assetId)}`);
}

/** The P2.19 registry views (the Sites/Assets workspaces' data sources). */
export function getPortfolioView(
  portfolioId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<unknown>> {
  return client.get(`/api/v1/portfolios/${encodeURIComponent(portfolioId)}/view`);
}

export function getSiteView(siteId: string, client: ApiClient = api): Promise<DataEnvelope<unknown>> {
  return client.get(`/api/v1/sites/${encodeURIComponent(siteId)}/view`);
}

export function getAssetView(assetId: string, client: ApiClient = api): Promise<DataEnvelope<unknown>> {
  return client.get(`/api/v1/assets/${encodeURIComponent(assetId)}/view`);
}
