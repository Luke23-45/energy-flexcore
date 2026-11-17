// FRV1-R9 — the design-system structural pin test (Vitest). UNRUN — never
// executed in the FRV1 wave (no install/build/test; the human gate runs
// `npm test` after the first install, as recorded for the whole console).
//
// METHOD (same recorded precedent as honestyStructural.test.ts): a
// grep-based STRUCTURAL assertion. The test reads the console's own
// stylesheet (src/styles/design-system.css) and the shell documents
// (src/main.tsx, index.html) with node:fs and asserts, structurally, the
// non-negotiables of the design contract
// (docs/reviews/final_review_for_v1/01_reviews/design_spec_console_ios.md):
//
//   1. BOTH color-scheme token blocks exist: the :root light block (with
//      `color-scheme: light dark`) and the
//      @media (prefers-color-scheme: dark) override block;
//   2. `font-variant-numeric: tabular-nums` is declared ON a selector that
//      covers td, th, AND dd (alignment is non-negotiable in an operations
//      console — spec §3);
//   3. the .console-header rule carries backdrop-filter AND its
//      -webkit- prefixed form (the frosted sticky chrome — spec §4);
//   4. the prefers-reduced-motion gates exist: both the no-preference
//      block that hosts the (only) animations and the reduce block that
//      structurally kills anything ungated (spec §5 tail);
//   5. NO external asset: the stylesheet contains no "url(" anywhere
//      (no external fonts, no images — system stacks only);
//   6. NO cascade import: the stylesheet contains no at-import rule
//      (no CDN link, one self-contained file);
//   7. the segmented-control pin: an aria-current="page" selector exists;
//   8. the touch-target pin: the nav link rule declares min-height: 44px;
//   9. the stylesheet is actually wired into the app entry point;
//  10. index.html declares color-scheme + the light/dark theme-color metas
//      so UA chrome follows the same two schemes the tokens define.
//
// What a structural scan CANNOT prove (recorded honestly): it proves the
// contract text exists in the stylesheet, not how it renders — the visual
// review on macOS/iOS Safari AND Windows/Chrome (the Segoe fallback path)
// is the human gate, per the spec's acceptance bar §7.2.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const HERE = dirname(fileURLToPath(import.meta.url));
const CSS_PATH = join(HERE, 'design-system.css');
const MAIN_PATH = join(HERE, '..', 'main.tsx');
const INDEX_PATH = join(HERE, '..', '..', 'index.html');

const css: string = readFileSync(CSS_PATH, 'utf-8');
const mainTs: string = readFileSync(MAIN_PATH, 'utf-8');
const indexHtml: string = readFileSync(INDEX_PATH, 'utf-8');

/** A parsed top-level-or-nested rule: selector text + declaration body. */
interface CssRule {
  selector: string;
  body: string;
}

/**
 * Minimal rule scanner (comments stripped first). It finds every
 * `selector { declarations }` pair INCLUDING rules nested inside at-media
 * and at-keyframes blocks; at-block prelude text (e.g. the media query)
 * attaches to nothing and is asserted separately from the raw text. Good
 * enough for the structural pins below — this is a contract pin, not a CSS
 * parser, and the limitation is recorded here rather than hidden.
 */
function parseRules(source: string): CssRule[] {
  const withoutComments = source.replace(/\/\*[\s\S]*?\*\//g, ' ');
  const rules: CssRule[] = [];
  const pattern = /([^{}]+)\{([^{}]*)\}/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(withoutComments)) !== null) {
    rules.push({ selector: match[1].trim(), body: match[2].trim() });
  }
  return rules;
}

const rules: CssRule[] = parseRules(css);

/** All rules whose selector mentions the given fragment. */
function rulesWith(fragment: string): CssRule[] {
  return rules.filter((rule) => rule.selector.includes(fragment));
}

describe('design-system.css exists and is well-formed (FRV1-R9)', () => {
  it('the stylesheet is non-empty and brace-balanced', () => {
    expect(css.trim().length).toBeGreaterThan(0);
    const opens = css.split('{').length;
    const closes = css.split('}').length;
    expect(opens).toBe(closes);
  });
});

describe('spec §2 — both color-scheme token blocks exist', () => {
  it('the :root light token block declares color-scheme: light dark', () => {
    const root = rulesWith(':root');
    expect(root.length).toBeGreaterThan(0);
    const anyRootBody = root.map((rule) => rule.body).join('\n');
    expect(anyRootBody).toContain('color-scheme: light dark');
  });

  it('the dark override block exists (prefers-color-scheme: dark)', () => {
    expect(css).toContain('@media (prefers-color-scheme: dark)');
  });

  it('the light and dark grouped backgrounds are both declared', () => {
    expect(css).toContain('--bg-grouped: #F2F2F7');
    expect(css).toContain('--bg-grouped: #000000');
    expect(css).toContain('--bg-elevated: #FFFFFF');
    expect(css).toContain('--bg-elevated: #1C1C1E');
  });
});

describe('spec §3 — tabular numerals on td, th, and dd', () => {
  it('a selector covering td, th AND dd declares tabular-nums', () => {
    const tabular = rules.filter((rule) =>
      rule.body.includes('font-variant-numeric') &&
      rule.body.includes('tabular-nums'),
    );
    expect(tabular.length).toBeGreaterThan(0);
    const covering = tabular.filter((rule) =>
      rule.selector.includes('td') &&
      rule.selector.includes('th') &&
      rule.selector.includes('dd'),
    );
    expect(covering.length).toBeGreaterThan(0);
  });
});

describe('spec §4 — the frosted sticky header', () => {
  it('.console-header carries backdrop-filter (standard + -webkit- form)', () => {
    const header = rulesWith('.console-header').filter((rule) =>
      rule.body.includes('backdrop-filter'),
    );
    expect(header.length).toBeGreaterThan(0);
    const body = header.map((rule) => rule.body).join('\n');
    expect(body).toContain('-webkit-backdrop-filter');
    expect(body).toContain('backdrop-filter');
  });

  it('.console-header is sticky (position: sticky; top: 0)', () => {
    const header = rulesWith('.console-header');
    const body = header.map((rule) => rule.body).join('\n');
    expect(body).toContain('position: sticky');
  });
});

describe('spec §5 tail — reduced motion is structurally honored', () => {
  it('the no-preference gate block exists (animations live only inside it)', () => {
    expect(css).toContain('@media (prefers-reduced-motion: no-preference)');
  });

  it('the reduce block kills animations and transitions', () => {
    expect(css).toContain('@media (prefers-reduced-motion: reduce)');
    const reduceBlock = css.slice(css.indexOf('@media (prefers-reduced-motion: reduce)'));
    expect(reduceBlock).toContain('animation: none');
    expect(reduceBlock).toContain('transition: none');
  });
});

describe('no external assets, no cascade imports (hard bans)', () => {
  it('the stylesheet contains no "url(" anywhere', () => {
    expect(css.includes('url(')).toBe(false);
  });

  it('the stylesheet contains no at-import rule', () => {
    expect(css.includes('@import')).toBe(false);
  });
});

describe('spec §4 — the segmented-control nav pins', () => {
  it('an aria-current selector exists (the selected-segment chip)', () => {
    expect(css).toContain('aria-current');
  });

  it('the nav link rule declares min-height: 44px (touch target)', () => {
    const navLinks = rules.filter(
      (rule) => rule.selector.includes('nav a') &&
        rule.body.includes('min-height: 44px'),
    );
    expect(navLinks.length).toBeGreaterThan(0);
  });
});

describe('the stylesheet is wired into the entry point', () => {
  it('main.tsx imports ./styles/design-system.css', () => {
    expect(mainTs).toContain("import './styles/design-system.css';");
  });
});

describe('index.html follows the same two schemes (UA chrome)', () => {
  it('declares the color-scheme meta and both theme-color values', () => {
    expect(indexHtml).toContain('<meta name="color-scheme" content="light dark"');
    expect(indexHtml).toContain('name="theme-color"');
    expect(indexHtml).toContain('#F2F2F7');
    expect(indexHtml).toContain('#1C1C1E');
  });
});


// ============================================================================
// DESIGN WAVE 2 — appended assertions (FRV1-R9-002 / FRV1-R9-003 / the
// .honesty-banner recipe gap / §5 badge tints). Everything above is the
// wave-1 pin set, UNCHANGED; the block below pins the wave-2 amendments.
// The header amendment is recorded here explicitly: the wave-1 §4 pins
// above still pass BY CONSTRUCTION because the titlebar rule is written as
// a descendant of .console-header (the wave-1 pins key on that selector
// text); the pins below state the amended reality on its own terms.
// ============================================================================

const APP_PATH = join(HERE, '..', 'App.tsx');
const appSource: string = readFileSync(APP_PATH, 'utf-8');

/** The 14 frozen §14.1 command states, verbatim (stateVocabulary.ts). */
const WAVE2_COMMAND_STATES = [
  'PROPOSED', 'VALIDATED', 'FEASIBLE', 'APPROVED', 'ISSUED', 'ACKNOWLEDGED',
  'TRACKING', 'COMPLETED', 'REJECTED', 'FAILED', 'EXPIRED', 'CANCELLED',
  'SUPERSEDED', 'FALLBACK',
] as const;

describe('FRV1-R9-002 — the sticky chrome is the titlebar, not the whole header', () => {
  it('.console-titlebar is the frosted sticky element (sticky + backdrop-filter, both forms)', () => {
    const titlebar = rulesWith('.console-titlebar').filter((rule) =>
      rule.body.includes('backdrop-filter'),
    );
    expect(titlebar.length).toBeGreaterThan(0);
    const body = titlebar.map((rule) => rule.body).join('\n');
    expect(body).toContain('position: sticky');
    expect(body).toContain('top: 0');
    expect(body).toContain('-webkit-backdrop-filter');
    expect(body).toContain('backdrop-filter');
    expect(body).toContain('var(--bg-chrome)');
  });

  it('.console-titlebar is full-bleed inside the header (negative margins vs --gutter)', () => {
    const body = rulesWith('.console-titlebar').map((rule) => rule.body).join('\n');
    expect(body).toContain('calc(var(--gutter) * -1)');
  });

  it('the .console-header ELEMENT rule is non-sticky (the banner stack scrolls away)', () => {
    const elementRules = rules.filter((rule) =>
      rule.selector.trim().endsWith('.console-header'),
    );
    expect(elementRules.length).toBeGreaterThan(0);
    const bodies = elementRules.map((rule) => rule.body).join('\n');
    expect(bodies).not.toContain('position: sticky');
    expect(bodies).toContain('background: transparent');
  });

  it('App.tsx wraps ONLY the h1 + live-status line in .console-titlebar, above the banner stack', () => {
    expect(appSource).toContain('<div className="console-titlebar">');
    const titlebarOpen = appSource.indexOf('<div className="console-titlebar">');
    const titlebarClose = appSource.indexOf('</div>');
    const h1 = appSource.indexOf('<h1>');
    const live = appSource.indexOf('<LiveStatusLine');
    const modeBanner = appSource.indexOf('className="mode-banner"');
    const modeGate = appSource.indexOf('<ModeGateNotice />');
    const tenant = appSource.indexOf('<TenantBanner />');
    const session = appSource.indexOf('<SessionBanner />');
    const legend = appSource.indexOf('<ValueTypeLegend />');
    // order: titlebar opens before the h1 and the live-status line, and the
    // whole banner stack follows the titlebar's close.
    expect(titlebarOpen).toBeGreaterThan(-1);
    expect(titlebarOpen).toBeLessThan(h1);
    expect(h1).toBeLessThan(live);
    expect(live).toBeLessThan(titlebarClose);
    expect(titlebarClose).toBeLessThan(modeBanner);
    expect(modeBanner).toBeLessThan(modeGate);
    expect(modeGate).toBeLessThan(tenant);
    expect(tenant).toBeLessThan(session);
    expect(session).toBeLessThan(legend);
    // ADDITIVE-ONLY: the pinned wording constants are untouched in the shell.
    expect(appSource).toContain('Control authority: SIMULATION / READ-ONLY (SHADOW) ONLY in this');
    expect(appSource).toContain('Live updates: connected');
  });
});

describe('FRV1-R9-003 — §6 color contract on the frozen vocabularies', () => {
  it('a chip color rule exists for every one of the 14 command states (exact case)', () => {
    for (const state of WAVE2_COMMAND_STATES) {
      expect(css).toContain(`[data-state="${state}"]`);
    }
  });

  it('the §6 palette is mapped (spot-check the class-critical values)', () => {
    const completed = rulesWith('[data-state="COMPLETED"]').map((r) => r.body).join('\n');
    expect(completed).toContain('var(--green)');
    const failed = rulesWith('[data-state="FAILED"]').map((r) => r.body).join('\n');
    expect(failed).toContain('var(--red)');
    const approved = rulesWith('[data-state="APPROVED"]').map((r) => r.body).join('\n');
    expect(approved).toContain('var(--indigo)');
    const issued = rulesWith('[data-state="ISSUED"]').map((r) => r.body).join('\n');
    expect(issued).toContain('var(--orange)');
    const proposed = rulesWith('[data-state="PROPOSED"]').map((r) => r.body).join('\n');
    expect(proposed).toContain('var(--blue)');
    const cancelled = rulesWith('[data-state="CANCELLED"]').map((r) => r.body).join('\n');
    expect(cancelled).toContain('var(--gray)');
  });

  it('a chip color rule exists for all six telemetry quality states (data-quality hook)', () => {
    for (const quality of ['GOOD', 'SUSPECT', 'BAD', 'MISSING', 'ESTIMATED', 'STALE']) {
      expect(css).toContain(`[data-quality="${quality}"]`);
    }
  });

  it('the capsule chip recipe carries the §5 tint (fill 15% / border 30%) with a gray fail-safe', () => {
    const recipe = rules
      .filter((rule) =>
        rule.selector.includes('command-state-badge') ||
        rule.selector.includes('quality-badge') ||
        rule.selector.includes('.state-chip'),
      )
      .map((rule) => rule.body)
      .join('\n');
    expect(recipe).toContain('15%, transparent');
    expect(recipe).toContain('30%, transparent');
    expect(recipe).toContain('var(--gray)');
  });

  it('the dark block raises the chip alphas (fill 22% / border 40%)', () => {
    const darkStart = css.indexOf('@media (prefers-color-scheme: dark)');
    const wave2Dark = css.indexOf('22%, transparent', darkStart);
    expect(wave2Dark).toBeGreaterThan(-1);
    const chipDark = css.slice(css.indexOf('.console [data-testid="command-state-badge"]', wave2Dark));
    expect(chipDark).toContain('40%, transparent');
  });

  it('severity chips exist for CRITICAL / WARNING (INFO keeps the gray base)', () => {
    expect(css).toContain('.severity-critical');
    expect(css).toContain('.severity-warning');
    const critical = rulesWith('.severity-critical').map((r) => r.body).join('\n');
    expect(critical).toContain('var(--red)');
  });

  it('plugin lifecycle/health chips: the state-chip recipe and the FAILING_* red key exist', () => {
    expect(css).toContain('.state-chip');
    expect(css).toContain('[class*="state-failing"]');
    expect(css).toContain('.state-ready');
    expect(css).toContain('.state-not_reported');
  });

  it('solver-outcome chips exist for the §12.6 vocabulary', () => {
    expect(css).toContain('.state-optimal');
    expect(css).toContain('.state-infeasible');
    expect(css).toContain('.state-timeout');
  });

  it('the additive TSX classNames exist (alarmsPanel severity, plugin state chips, solver chip)', () => {
    const alarms = readFileSync(join(HERE, '..', 'components', 'alarmsPanel.tsx'), 'utf-8');
    expect(alarms).toContain('className={`severity severity-${severityToken}`}');
    const plugins = readFileSync(join(HERE, '..', 'workspaces', 'PluginsWorkspace.tsx'), 'utf-8');
    expect(plugins).toContain('className={`state-chip state-${lifecycle.toLowerCase()}`}');
    expect(plugins).toContain('className={`state-chip state-${health.healthState.toLowerCase()}`}');
    const schedules = readFileSync(join(HERE, '..', 'workspaces', 'Schedules.tsx'), 'utf-8');
    expect(schedules).toContain('className={`state-chip state-${run.status}`}');
  });
});

describe('the .honesty-banner recipe exists (wave-2 contract gap)', () => {
  it('.honesty-banner renders as the grouped footnote card with the indigo accent', () => {
    const banner = rulesWith('.honesty-banner').filter((rule) =>
      rule.body.includes('--banner-accent'),
    );
    expect(banner.length).toBeGreaterThan(0);
    const body = banner.map((rule) => rule.body).join('\n');
    expect(body).toContain('var(--indigo)');
    expect(body).toContain('font-size: 13px');
    expect(body).toContain('var(--bg-secondary-fill)');
  });

  it('.honesty-banner is NOT collapsed (the full honesty text stays rendered)', () => {
    // No details/summary/collapse recipe and no display:none anywhere near it.
    const body = rulesWith('.honesty-banner').map((rule) => rule.body).join('\n');
    expect(body).not.toContain('display: none');
    expect(body).not.toContain('details');
  });
});

describe('§5 badge tints enabled (wave 2)', () => {
  it('per-state tint rules exist for the quality classes', () => {
    for (const cls of ['quality-good', 'quality-suspect', 'quality-bad', 'quality-estimated']) {
      expect(css).toContain(`.${cls}`);
    }
    expect(css).toContain('.quality-missing');
    expect(css).toContain('.quality-stale');
  });

  it('.value-type tints only where a §6 key exists (estimated teal, stale gray)', () => {
    expect(css).toContain('.value-type-estimated');
    expect(css).toContain('.value-type-stale');
  });

  it('.demo-data carries the orange tint', () => {
    const demo = rulesWith('.demo-data').map((r) => r.body).join('\n');
    expect(demo).toContain('var(--orange)');
  });

  it('the chip ink tokens exist for BOTH schemes (readable text variants)', () => {
    expect(css).toContain('--chip-ink-yellow: #936D00');
    expect(css).toContain('--chip-ink-green: #1E7E3C');
    expect(css).toContain('--chip-ink-red: #D70015');
    expect(css).toContain('--chip-ink-blue: #0062CC');
    const darkStart = css.indexOf('@media (prefers-color-scheme: dark)');
    const darkBlock = css.slice(darkStart);
    expect(darkBlock).toContain('--chip-ink-yellow: var(--yellow)');
    expect(darkBlock).toContain('--chip-ink-green: var(--green)');
  });
});


// ============================================================================
// DESIGN WAVE 2 (Agent B) — appended assertions. Everything above (wave-1
// pins + the other wave-2 agent's pins) is UNCHANGED. This block pins the
// Agent-B wave-2 blocks: wide-table containment (.table-scroll), the
// loading/error/empty state treatments, the availability accents, the
// control recipe, the pre recipe, the definition-list dense-surface pass,
// and the high-impact confirmation / step-up cards + the print-completeness
// block. All key on selector/declaration TEXT — no rendering.
// ============================================================================

describe('FRV1-R9-001 (Agent B) — .table-scroll wide-table containment', () => {
  it('the wrapper rule scrolls horizontally (touch scrolling included)', () => {
    const wrapper = rulesWith('.table-scroll').filter((rule) =>
      rule.body.includes('overflow-x'),
    );
    expect(wrapper.length).toBeGreaterThan(0);
    const body = wrapper.map((rule) => rule.body).join('\n');
    expect(body).toContain('overflow-x: auto');
    expect(body).toContain('-webkit-overflow-scrolling: touch');
  });

  it('the caption stays visible (sticky, left-pinned, on the card surface)', () => {
    const caption = rulesWith('.table-scroll caption');
    expect(caption.length).toBeGreaterThan(0);
    const body = caption.map((rule) => rule.body).join('\n');
    expect(body).toContain('position: sticky');
    expect(body).toContain('left: 0');
  });

  it('the scroll hint is gated to hover:none and is token-driven (no url, both schemes adapt)', () => {
    const hoverNone = css.indexOf('@media (hover: none)');
    expect(hoverNone).toBeGreaterThan(-1);
    const gated = css.slice(hoverNone);
    expect(gated).toContain('background-attachment: local, scroll');
    expect(gated).toContain('var(--bg-elevated)');
  });

  it('the additive wrappers exist in the wide-table workspaces (source scan)', () => {
    for (const [file, count] of [
      ['Overview.tsx', 2],
      ['Assets.tsx', 1],
      ['Schedules.tsx', 1],
      ['MarketPrograms.tsx', 1],
      ['PluginsIntegrations.tsx', 1],
      ['PluginsWorkspace.tsx', 1],
      ['ResearchWorkspace.tsx', 1],
      ['SimulationLab.tsx', 6],
    ] as const) {
      const source = readFileSync(join(HERE, '..', 'workspaces', file), 'utf-8');
      expect(source.split('<div className="table-scroll">').length - 1).toBe(count);
    }
  });
});

describe('FRV1-R9-007 (Agent B) — loading / error / empty state treatments', () => {
  it('loading keeps secondary text; the shimmer exists ONLY under no-preference', () => {
    const loading = rulesWith('[data-testid="loading"]').map((r) => r.body).join('\n');
    expect(loading).toContain('var(--label-secondary)');
    // The transparent text fill (the shimmer's mechanism) is declared exactly
    // once, and AFTER the last no-preference gate (i.e. inside a gated block).
    const gates = css.split('@media (prefers-reduced-motion: no-preference)');
    expect(gates.length).toBeGreaterThan(1);
    const lastGate = css.lastIndexOf('@media (prefers-reduced-motion: no-preference)');
    const fill = css.indexOf('-webkit-text-fill-color: transparent');
    expect(css.indexOf('color: transparent')).toBeGreaterThan(lastGate);
    expect(fill).toBeGreaterThan(lastGate);
    expect(css).toContain('@keyframes state-loading-shimmer');
  });

  it('the error vocabulary renders as the red-tinted footnote card (red @ 10%)', () => {
    const error = rulesWith('[role="alert"]:not(.live-status)').map((r) => r.body).join('\n');
    expect(error).toContain('color-mix(in srgb, var(--red) 10%, transparent)');
    expect(error).toContain('var(--red)');
  });

  it('the empty vocabulary renders as secondary text (no alarm styling)', () => {
    const empty = rulesWith('[data-testid="empty-state"]').map((r) => r.body).join('\n');
    expect(empty).toContain('var(--label-secondary)');
    expect(empty).not.toContain('var(--red)');
  });

  it('the availability accents key on the EXISTING availability-<state> testids only', () => {
    expect(css).toContain('[data-testid="availability-available"]');
    expect(css).toContain('[data-testid="availability-unavailable"]');
    const avail = rulesWith('[data-testid="availability-"]').map((r) => r.body).join('\n');
    expect(avail).toContain('var(--green)');
    expect(avail).toContain('var(--red)');
  });

  it('verbatim statements (blockquote) get the grouped-card treatment with the indigo accent', () => {
    const quote = rulesWith('blockquote').map((r) => r.body).join('\n');
    expect(quote).toContain('var(--indigo)');
    expect(quote).toContain('var(--bg-secondary-fill)');
  });
});

describe('FRV1-R9-008/012 (Agent B) — control recipe, inspect rows, pre blocks', () => {
  it('buttons carry the 44px tinted-control recipe with an explicit disabled state', () => {
    const button = rulesWith('.console button').map((r) => r.body).join('\n');
    expect(button).toContain('min-height: 44px');
    expect(button).toContain('cursor: not-allowed');
  });

  it('text inputs and selects carry the 44px control recipe', () => {
    const inputs = rulesWith('input[type="text"]').map((r) => r.body).join('\n');
    expect(inputs).toContain('min-height: 44px');
    expect(inputs).toContain('var(--radius-control)');
  });

  it('checkboxes/radios carry the blue accent-color', () => {
    const boxes = rulesWith('input[type="checkbox"]').map((r) => r.body).join('\n');
    expect(boxes).toContain('accent-color: var(--blue)');
  });

  it('the inspect rows get a 44px label and a checked highlight (progressive :has)', () => {
    expect(css).toContain('[data-testid="command-list"] label');
    expect(css).toContain('[data-testid="audit-trail"] label');
    expect(css).toContain('li:has(input:checked)');
  });

  it('verbatim pre blocks scroll inside their card (no page-level overflow)', () => {
    const pre = rulesWith('.console pre').map((r) => r.body).join('\n');
    expect(pre).toContain('overflow-x: auto');
    expect(pre).toContain('var(--font-mono)');
  });
});

describe('FRV1-R9-013 (Agent B) — definition-list dense-surface pass', () => {
  it('multi-value dd rows (inner divs) align flush-left inside the value column', () => {
    const rule = rulesWith('dd > div');
    expect(rule.length).toBeGreaterThan(0);
    expect(rule.map((r) => r.body).join('\n')).toContain('text-align: left');
  });

  it('long-prose dd values key on the stable testids (spot-check)', () => {
    expect(css).toContain('[data-testid^="permissions-"]');
    expect(css).toContain('[data-testid^="blockers-"]');
    expect(css).toContain('[data-testid^="certificate-expiry-"]');
  });

  it('Grid record rows get hairline separation via .grid-check', () => {
    expect(css).toContain('.grid-check');
  });
});

describe('§17.3/§20.2 (Agent B) — high-impact confirmation + step-up cards', () => {
  it('.confirm-high-impact exists with the red accent and a stronger hairline border', () => {
    const card = rulesWith('.confirm-high-impact').map((r) => r.body).join('\n');
    expect(card).toContain('var(--red)');
    expect(card).toContain('border: 1px solid');
  });

  it('.step-up-approval exists with the indigo accent', () => {
    const card = rulesWith('.step-up-approval').map((r) => r.body).join('\n');
    expect(card).toContain('var(--indigo)');
    expect(card).toContain('border: 1px solid');
  });

  it('the additive TSX classes exist and the label/input regex pins still hold', () => {
    const confirm = readFileSync(join(HERE, '..', 'components', 'ConfirmHighImpact.tsx'), 'utf-8');
    expect(confirm).toContain('<div className="confirm-high-impact" data-testid="confirm-high-impact">');
    expect(confirm).toContain('<label htmlFor={inputId}>');
    const stepUp = readFileSync(join(HERE, '..', 'components', 'stepUpApproval.tsx'), 'utf-8');
    expect(stepUp).toContain('<div className="step-up-approval" data-testid="step-up-approval">');
    const placeholder = readFileSync(
      join(HERE, '..', 'workspaces', 'WorkspacePlaceholder.tsx'),
      'utf-8',
    );
    expect(placeholder).toContain('className="state-empty"');
  });
});

describe('print completeness for tables (Agent B wave-2 tail)', () => {
  it('a second print block repeats table header rows and keeps rows intact', () => {
    expect(css).toContain('table-header-group');
    expect(css).toContain('break-inside: avoid');
    expect(css).toContain('page-break-inside: avoid');
    // The wave-1 print block is untouched; the tail block is additive.
    expect(css.split('@media print').length - 1).toBe(2);
  });

  it('printed tables are not clipped by the scroll wrapper', () => {
    const printStart = css.lastIndexOf('@media print');
    const printBlock = css.slice(printStart);
    expect(printBlock).toContain('.table-scroll');
    expect(printBlock).toContain('overflow: visible');
  });
});
