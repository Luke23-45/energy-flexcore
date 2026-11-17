// P13.17 — session-lifecycle store + expiry display tests (WRITTEN-UNRUN —
// never executed in this loop; the human gate runs `npm test` after the
// first install). Conventions follow the P5.26 test files: pure-module
// assertions, boundary pinning, no network.
import { describe, expect, it } from 'vitest';
import {
  EXPIRING_SOON_WINDOW_MILLIS,
  clearSessionLifecycle,
  formatSessionExpiry,
  getSessionLifecycle,
  setSessionLifecycle,
  subscribeToSessionLifecycle,
} from './session';

afterEachCommon();

function afterEachCommon(): void {
  clearSessionLifecycle();
}

describe('the session-lifecycle store (authentication-layer-only writes)', () => {
  it('defaults to no lifecycle facts (the honest state until a deployment composes the login flow)', () => {
    expect(getSessionLifecycle()).toBeNull();
  });

  it('set + get round-trips a copy (the store never leaks its state object)', () => {
    setSessionLifecycle({ sessionId: 'sess-1', expiresAtUnixMillis: 5_000 });
    const read = getSessionLifecycle();
    expect(read).toEqual({ sessionId: 'sess-1', expiresAtUnixMillis: 5_000 });
    expect(read).not.toBe(getSessionLifecycle());
  });

  it('clear resets to null and notifies subscribers', () => {
    let notified = 0;
    const unsubscribe = subscribeToSessionLifecycle(() => {
      notified += 1;
    });
    setSessionLifecycle({ sessionId: 'sess-2', expiresAtUnixMillis: 6_000 });
    expect(notified).toBe(1);
    clearSessionLifecycle();
    expect(notified).toBe(2);
    expect(getSessionLifecycle()).toBeNull();
    unsubscribe();
  });

  it('an unsubscribed listener stops receiving notifications', () => {
    let notified = 0;
    const unsubscribe = subscribeToSessionLifecycle(() => {
      notified += 1;
    });
    unsubscribe();
    setSessionLifecycle({ sessionId: 'sess-3', expiresAtUnixMillis: 7_000 });
    expect(notified).toBe(0);
    clearSessionLifecycle();
  });
});

describe('formatSessionExpiry (the pure expiry display; boundaries pinned)', () => {
  const NOW = 1_000_000;

  it('renders the expired state at and past the deadline (inclusive boundary)', () => {
    expect(formatSessionExpiry(NOW, NOW).state).toBe('expired');
    expect(formatSessionExpiry(NOW - 1, NOW).state).toBe('expired');
    expect(formatSessionExpiry(NOW, NOW).text).toContain('Session expired');
    expect(formatSessionExpiry(NOW, NOW).text).toContain('denied at the resolver');
  });

  it('renders expiring-soon exactly within the registered 5-minute window', () => {
    const boundary = formatSessionExpiry(NOW + EXPIRING_SOON_WINDOW_MILLIS, NOW);
    expect(boundary.state).toBe('expiring-soon');
    const inside = formatSessionExpiry(NOW + EXPIRING_SOON_WINDOW_MILLIS - 1, NOW);
    expect(inside.state).toBe('expiring-soon');
    expect(inside.text).toMatch(/Session expires in 4m 59s/);
    expect(inside.text).toContain('the resolver enforces the deadline regardless of this display');
  });

  it('renders active above the window with the remaining time', () => {
    const display = formatSessionExpiry(NOW + 90 * 60 * 1000, NOW);
    expect(display.state).toBe('active');
    expect(display.text).toMatch(/Session expires in 90m 00s/);
  });

  it('a stale client clock misrenders the countdown but the text never claims enforcement authority', () => {
    // A clock set far in the past makes the display show a huge remaining
    // time — the honest statement in the text is that the resolver, not
    // this display, decides.
    const display = formatSessionExpiry(NOW + 12 * 60 * 60 * 1000, NOW - 6 * 60 * 60 * 1000);
    expect(display.state).toBe('active');
    expect(display.text).toContain('18m');
    // The active/expiring text lines always carry the resolver authority
    // caveat or the deadline wording; none of them grants anything.
    expect(display.text.includes('resolver') || display.text.includes('Session expires')).toBe(true);
  });
});
