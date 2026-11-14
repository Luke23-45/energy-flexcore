// P5.02–P5.04 — small pure display formatters shared by the implemented
// workspaces (Overview, Portfolios, Sites, Assets, Grid).
//
// Discipline: these functions FORMAT data the API returned; they never
// derive, coerce, or default a value into existence. A missing timestamp is
// formatted as an explicit "not present" marker, never as "now" (a missing
// time must not be silently replaced by the viewer's clock — §20.2).

/** The explicit marker for a timestamp the record does not carry. */
export const NOT_PRESENT = 'not present in the record';

/**
 * Formats a duration in milliseconds as a coarse human age. Negative ages
 * (clock skew, §17.4) are never rendered as a plausible age: they are
 * flagged verbatim, because a negative age is a data defect, not a display
 * concern.
 */
export function formatAgeMs(ageMs: number): string {
  if (!Number.isFinite(ageMs)) return 'unusable age (non-finite)';
  if (ageMs < 0) return `${ageMs} ms (NEGATIVE age — clock skew or a data defect, §17.4)`;
  if (ageMs < 1000) return `${ageMs} ms`;
  const seconds = Math.floor(ageMs / 1000);
  if (seconds < 60) return `${seconds} s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ${seconds % 60} s`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ${minutes % 60} min`;
  const days = Math.floor(hours / 24);
  return `${days} d ${hours % 24} h`;
}

/**
 * Formats a UTC unix-millis timestamp as its ISO-8601 instant. Non-finite
 * input is the NOT_PRESENT marker, never a fabricated date.
 */
export function formatUnixMillis(millis: number): string {
  if (!Number.isFinite(millis)) return NOT_PRESENT;
  return new Date(millis).toISOString();
}

/** Formats an RFC 3339 string verbatim (the API's timestamp form). */
export function formatRfc3339(timestamp: string): string {
  return timestamp;
}

/** Formats a canonical MW value with its unit and the frozen sign meaning. */
export function formatMw(mw: number): string {
  return `${mw} MW`;
}
