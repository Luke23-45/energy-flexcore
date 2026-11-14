// P5.01 — API client tests (error-envelope mapping, credential handling,
// the §16.2 no-tenant-selector rule). UNRUN — human executes via
// `npm test` after install (decision record: 01_25_scaffold_live.md).
import { describe, expect, it } from 'vitest';
import { ApiError, createApiClient, devSessionToken } from './client';

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

describe('createApiClient', () => {
  it('GETs and unwraps the JSON body', async () => {
    const calls: RequestInfo[] = [];
    const client = createApiClient({
      fetchImpl: async (input: RequestInfo) => {
        calls.push(input);
        return jsonResponse(200, { data: { id: 'x' } });
      },
    });
    const body = await client.get<{ data: { id: string } }>('/api/v1/tenants');
    expect(body.data.id).toBe('x');
    expect(calls[0]).toBe('/api/v1/tenants');
  });

  it('sends the bearer token from the provider when present', async () => {
    let seenAuthorization: string | null = null;
    const client = createApiClient({
      tokenProvider: () => 'dev tenant=abc subject=u role=viewer',
      fetchImpl: async (_input, init) => {
        seenAuthorization =
          new Headers(init?.headers).get('authorization') ?? null;
        return jsonResponse(200, { data: [] });
      },
    });
    await client.get('/api/v1/portfolios');
    expect(seenAuthorization).toBe('Bearer dev tenant=abc subject=u role=viewer');
  });

  it('sends NO tenant selector of any kind (§16.2: tenancy is claims-derived server-side)', async () => {
    let headers = new Headers();
    const client = createApiClient({
      tokenProvider: () => 'dev tenant=abc subject=u role=viewer',
      fetchImpl: async (_input, init) => {
        headers = new Headers(init?.headers);
        return jsonResponse(200, { data: [] });
      },
    });
    await client.get('/api/v1/assets');
    const names = [...headers.keys()].join(',');
    expect(names).not.toContain('tenant');
    expect(names).not.toContain('x-tenant');
  });

  it('maps the stable error envelope onto ApiError verbatim', async () => {
    const client = createApiClient({
      fetchImpl: async () =>
        jsonResponse(403, {
          error: { code: 'forbidden', message: 'authorization denied: no matching rule' },
        }),
    });
    await expect(client.get('/api/v1/tenants')).rejects.toMatchObject({
      status: 403,
      code: 'forbidden',
      message: 'authorization denied: no matching rule',
    });
  });

  it('fails closed with an opaque code when the envelope is unparseable', async () => {
    const client = createApiClient({
      fetchImpl: async () =>
        new Response('not json', { status: 500 }),
    });
    const result = client.get('/api/v1/tenants');
    await expect(result).rejects.toBeInstanceOf(ApiError);
    await result.catch((error: ApiError) => {
      expect(error.code).toBe('unparsed_error');
      // The server's raw text is never surfaced.
      expect(error.message).not.toContain('not json');
    });
  });

  it('maps a 401 exactly (unauthenticated)', async () => {
    const client = createApiClient({
      fetchImpl: async () =>
        jsonResponse(401, {
          error: { code: 'unauthenticated', message: 'missing or invalid authentication' },
        }),
    });
    await expect(client.get('/api/v1/live')).rejects.toMatchObject({ status: 401 });
  });
});

describe('devSessionToken (the recorded Phase 1 dev-auth path)', () => {
  it('renders the auth.rs dev token format verbatim', () => {
    expect(
      devSessionToken({
        tenantId: '00000000-0000-0000-0000-000000000001',
        subject: 'user:op',
        role: 'operator',
      }),
    ).toBe(
      'dev tenant=00000000-0000-0000-0000-000000000001 subject=user:op role=operator',
    );
  });
});
