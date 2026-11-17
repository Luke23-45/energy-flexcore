// P13.17 — login-flow design tests (WRITTEN-UNRUN; the human gate runs
// `npm test` after the first install). Structural honesty: the module
// performs no fetch and no navigation — pinned by source scan, the P5.26
// precedent.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  AUTH_CALLBACK_PATH,
  LOGIN_ENTRY_PATH,
  LOGIN_FLOW_NOT_IMPLEMENTED_NOTE,
  SESSION_READ_PATH,
  buildLoginUrl,
  parseLoginLanding,
  validateReturnTo,
} from './oidcLogin';

describe('the specified-not-implemented endpoint surface', () => {
  it('names the three recorded paths (the design surface, nothing served)', () => {
    expect(LOGIN_ENTRY_PATH).toBe('/api/v1/auth/login');
    expect(SESSION_READ_PATH).toBe('/api/v1/auth/session');
    expect(AUTH_CALLBACK_PATH).toBe('/api/v1/auth/callback');
  });

  it('the honesty note states the flow is designed, not implemented, and that no login was ever performed', () => {
    expect(LOGIN_FLOW_NOT_IMPLEMENTED_NOTE).toContain('NOT implemented');
    expect(LOGIN_FLOW_NOT_IMPLEMENTED_NOTE).toContain('specified-not-implemented');
    expect(LOGIN_FLOW_NOT_IMPLEMENTED_NOTE).toContain('No login has ever been performed');
    expect(LOGIN_FLOW_NOT_IMPLEMENTED_NOTE).toContain('confidential-client');
  });
});

describe('validateReturnTo (the strict open-redirect guard)', () => {
  it('accepts same-origin absolute paths only', () => {
    expect(validateReturnTo('/')).toEqual({ ok: true, path: '/' });
    expect(validateReturnTo('/workspaces/administration')).toEqual({
      ok: true,
      path: '/workspaces/administration',
    });
    expect(validateReturnTo('/a/b?query=1#frag')).toEqual({
      ok: true,
      path: '/a/b?query=1#frag',
    });
  });

  it('rejects every escape shape with a named reason', () => {
    expect(validateReturnTo('')).toMatchObject({ ok: false });
    expect(validateReturnTo('relative/path').reason).toContain('absolute path');
    expect(validateReturnTo('//evil.example').reason).toContain('protocol-relative');
    expect(validateReturnTo('/a//b').reason).toContain('embedded //');
    expect(validateReturnTo('/a\\b').reason).toContain('backslash');
    expect(validateReturnTo('https://evil.example').reason).toContain('scheme');
    expect(validateReturnTo('javascript:alert(1)').reason).toContain('scheme');
    expect(validateReturnTo('/a\u0000b').reason).toContain('control characters');
    expect(validateReturnTo('/a\u001bb').reason).toContain('control characters');
  });
});

describe('buildLoginUrl (pure URL construction; no navigation performed)', () => {
  it('builds the login-entry URL with an encoded return_to', () => {
    expect(buildLoginUrl('', '/workspaces')).toBe(
      '/api/v1/auth/login?return_to=%2Fworkspaces',
    );
    expect(buildLoginUrl('http://localhost:8080', '/x')).toBe(
      'http://localhost:8080/api/v1/auth/login?return_to=%2Fx',
    );
  });

  it('rejects a guard-failing return_to (the URL is never rendered)', () => {
    expect(() => buildLoginUrl('', '//evil.example')).toThrow('protocol-relative');
    expect(() => buildLoginUrl('', 'https://evil.example')).toThrow('scheme');
  });
});

describe('parseLoginLanding (strict; only the recorded parameter is read)', () => {
  it('a bare landing is success (the console then performs the session read)', () => {
    expect(parseLoginLanding('')).toEqual({ status: 'success' });
    expect(parseLoginLanding('?')).toEqual({ status: 'success' });
  });

  it('reads ONLY the recorded login_error parameter', () => {
    expect(parseLoginLanding('?login_error=state_mismatch')).toEqual({
      status: 'error',
      reason: 'state_mismatch',
    });
    expect(parseLoginLanding('login_error=idp_denied')).toEqual({
      status: 'error',
      reason: 'idp_denied',
    });
    expect(parseLoginLanding('?login_error=a%20b')).toEqual({
      status: 'error',
      reason: 'a b',
    });
  });

  it('unrecorded query content never derives console state', () => {
    expect(parseLoginLanding('?tenant=x&role=platform-admin')).toEqual({
      status: 'success',
    });
    expect(parseLoginLanding('?token=attacker-chosen')).toEqual({
      status: 'success',
    });
  });
});

describe('structural honesty (source scan — the P5.26 precedent)', () => {
  const SOURCE = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), 'oidcLogin.ts'),
    'utf-8',
  );
  const CODE = SOURCE.split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');

  it('performs no fetch, no XMLHttpRequest, no navigation', () => {
    expect(CODE).not.toMatch(/\bfetch\s*\(/);
    expect(CODE).not.toMatch(/XMLHttpRequest|sendBeacon/);
    expect(CODE).not.toMatch(/window\.location|location\.assign|location\.replace/);
  });
});
