// P5.25 — the console's live-update stream client (§15.1).
//
// RECORDED DECISIONS (decision record 01_25_scaffold_live.md):
//
// - **fetch-based SSE, not EventSource.** The endpoint requires the
//   Authorization bearer header, and the native EventSource API cannot set
//   custom headers; a token in the URL query would be a credential leak
//   into logs/history (P5.25 step 5). The stream is therefore read with
//   fetch + ReadableStream and parsed here (a small, fully-tested SSE
//   frame parser). Revisit if the endpoint ever moves to cookie
//   authentication.
// - **Reconnect with capped exponential backoff + jitter.** After a
//   disconnect the stream is NOT transparently resumed as if nothing
//   happened: the client surfaces "live updates paused — data may be
//   stale" (the paused-stale status) with the last delivery time, keeps
//   retrying with backoff, and marks the session as needing an explicit
//   re-sync (needsResync) — the REST read APIs are the gap-filler, invoked
//   by the caller, never implicitly here (P5.25 step 2, no silent
//   gap-filling).
// - **RESYNC_REQUIRED**: when the server reports a lagged stream (skipped
//   events), the client flags needsResync and surfaces the skip count —
//   the notice is data, not noise.
// - At-least-once: events may duplicate/reorder (§15.4). Deduplication and
//   per-stream sequencing are P5.33; this client does NOT dedup (it
//   delivers every parsed event to the consumer, honestly).

import type { LiveEventEnvelope, LiveResyncNotice } from './types';

/** Connection lifecycle as the UI must show it (§20.2 honesty). */
export type LiveStreamStatus = 'connecting' | 'open' | 'paused-stale' | 'closed';

export interface LiveStreamHandlers {
  onEnvelope(envelope: LiveEventEnvelope): void;
  onResyncNotice(notice: LiveResyncNotice): void;
  onStatusChange(status: LiveStreamStatus): void;
}

export interface LiveStreamOptions extends LiveStreamHandlers {
  /** The live endpoint (same-origin under the dev proxy). */
  url: string;
  /** Bearer token (dev-session; P5.26 replaces with the real session). */
  tokenProvider: () => string | null | undefined;
  /** Injectable fetch (tests). */
  fetchImpl?: typeof fetch;
}

export interface LiveStreamHandle {
  close(): void;
}

/** Backoff schedule: 1s, 2s, 4s, … capped at 30s, +/- 20% jitter. */
export function nextBackoffMs(attempt: number): number {
  const base = Math.min(1000 * 2 ** Math.max(0, attempt - 1), 30_000);
  const jitter = base * 0.2 * (Math.random() * 2 - 1);
  return Math.max(250, Math.round(base + jitter));
}

/**
 * Opens the live stream and manages its lifecycle. Returns a handle whose
 * close() ends the stream permanently (no further reconnects).
 */
export function connectLiveStream(options: LiveStreamOptions): LiveStreamHandle {
  const doFetch = options.fetchImpl ?? fetch.bind(globalThis);
  const controller = new AbortController();
  let closedByCaller = false;
  let attempt = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const setStatus = (status: LiveStreamStatus) => options.onStatusChange(status);

  const run = async (): Promise<void> => {
    while (!closedByCaller) {
      setStatus(attempt === 0 ? 'connecting' : 'paused-stale');
      let opened = false;
      try {
        const headers: Record<string, string> = { accept: 'text/event-stream' };
        const token = options.tokenProvider();
        if (token) headers.authorization = `Bearer ${token}`;
        const response = await doFetch(options.url, {
          headers,
          signal: controller.signal,
        });
        if (!response.ok || !response.body) {
          // 401/403 or a transport problem: paused-stale, retry with
          // backoff. The console does NOT fabricate a "connected" state.
          throw new Error(`live stream status ${response.status}`);
        }
        opened = true;
        attempt = 0;
        setStatus('open');
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        // The read loop: parse SSE frames as they arrive. An SSE frame is
        // a block of `field: value` lines separated by a blank line.
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          const frames = splitSseFrames(buffer);
          buffer = frames.rest;
          for (const frame of frames.complete) {
            const parsed = parseSseFrame(frame);
            if (!parsed) continue;
            if (parsed.event === 'RESYNC_REQUIRED') {
              const notice = safeJsonParse<LiveResyncNotice>(parsed.data);
              if (notice) options.onResyncNotice(notice);
              continue;
            }
            const envelope = safeJsonParse<LiveEventEnvelope>(parsed.data);
            if (envelope) options.onEnvelope(envelope);
          }
        }
        // The server ended the stream (shutdown, proxy timeout): treat as
        // a disconnect — the paused-stale state + backoff below.
      } catch (error) {
        if (closedByCaller || controller.signal.aborted) {
          break;
        }
        // Keep the failure honest in the console's own diagnostics; the UI
        // surface is the status line, not this log.
        console.warn('live stream disconnected', error);
      }
      if (closedByCaller) break;
      if (opened) {
        // The stream opened but then ended: re-sync explicitly.
        options.onResyncNotice({
          notice: 'resync_required',
          skipped_events: 0,
          statement:
            'the stream ended and is reconnecting; re-sync from the REST read APIs ' +
            '(no silent gap-filling, P5.25)',
        });
      }
      attempt += 1;
      setStatus('paused-stale');
      const delay = nextBackoffMs(attempt);
      await new Promise<void>((resolve) => {
        timer = setTimeout(resolve, delay);
      });
    }
  };

  void run();

  return {
    close(): void {
      closedByCaller = true;
      if (timer !== null) clearTimeout(timer);
      controller.abort();
      setStatus('closed');
    },
  };
}

/**
 * Splits a buffer into complete SSE frames and the trailing partial.
 * A frame is text terminated by a blank line (CRLF or LF).
 */
export function splitSseFrames(buffer: string): {
  complete: string[];
  rest: string;
} {
  const normalized = buffer.replace(/\r\n/g, '\n');
  const parts = normalized.split('\n\n');
  const rest = parts.pop() ?? '';
  return { complete: parts, rest };
}

/**
 * Parses one SSE frame into its fields. Returns null for comments
 * (`: heartbeat`) and frames without data. `event` defaults to 'message'
 * (the SSE spec) when the server sends no event line.
 */
export function parseSseFrame(frame: string): { event: string; data: string } | null {
  const lines = frame.split('\n');
  let event = 'message';
  const dataLines: string[] = [];
  for (const line of lines) {
    if (line.startsWith(':')) continue; // comment/heartbeat
    const colon = line.indexOf(':');
    if (colon === -1) continue;
    const field = line.slice(0, colon);
    let value = line.slice(colon + 1);
    if (value.startsWith(' ')) value = value.slice(1);
    if (field === 'event') event = value;
    else if (field === 'data') dataLines.push(value);
    // id/retry are accepted per spec but unused here (P5.33 adds
    // sequencing; the server sends no id field in this phase).
  }
  if (dataLines.length === 0) return null;
  return { event, data: dataLines.join('\n') };
}

function safeJsonParse<T>(text: string): T | null {
  try {
    return JSON.parse(text) as T;
  } catch {
    // A malformed body is dropped — never partially applied, never
    // invented around (§20.2 honesty).
    return null;
  }
}
