# Final Review R9 — Operator Console Visual/UX Layer (Iteration 2)

**Reviewer:** Agent A (design/UX pass)
**Date:** 2026-11-18
**Status:** REVIEWED — static read only (no build/test executed; `python -I` text census only)
**Finding totals:** P0: 1 · P1: 8 · P2: 7
**Design contract:** `docs/reviews/final_review_for_v1/01_reviews/design_spec_console_ios.md` (read first; finding vocabulary below maps onto its §2–§7 recipes).

## 1. Scope reviewed

Full census of the console's rendered surfaces (30 non-test `.tsx` files; counts below via a
`python -I` text census, verified per file). Deep-read = every rendered JSX line read this review.

| Area | Files | Read depth |
|---|---|---|
| Shell (`App.tsx`, `main.tsx`, `index.html`, `router.ts` census only) | 4 | Deep-read App/main/index; router skimmed. |
| Shared components (`components/` non-test) | 11 | Deep-read: `ValueType`, `honesty`, `workspaceData`, `tenantBanner`, `sessionBanner`, `modeGateNotice`, `alarmsPanel`, `availability`, `denialLogDiagnostics`, `ConfirmHighImpact`, `stepUpApproval`, `format`, `stateVocabulary` (markers). |
| Workspaces (`workspaces/` non-test `.tsx`) | 16 | Deep-read all 16: Overview, Portfolios, Sites, SitesEdgeStatus, Assets, Grid, Schedules, Dispatch, MarketPrograms, SimulationLab, ResearchWorkspace, PluginsIntegrations, PluginsWorkspace, AuditReports, Administration, AdminManagementSection (+ `registry.ts`, `WorkspacePlaceholder.tsx` census). |
| Pinning tests (42 written-unrun files) | sampled | Grep-verified which assertions pin text/testids vs class names (§3.10). |

**Machine census (non-test `.tsx`, 30 files):** 23 `<table>` / 23 `<caption>` / 102 `<th>` /
95 `<td>`-open tags (+5 attribute-bearing `<td>`); 7 `<dl>` with 39 `<dt>` and 39 `<dd>`
(attribute-bearing `<dd>` counted); 162 `<code>`; 30 `<ul>` / 4 `<ol>` / 26+ `<li>` (map-generated);
h1:1, h2:16, h3:56, h4:35, h5:2; 10 `<button>`, 13 `<input>`, 5 `<select>`; 3 `<pre>`;
1 `<blockquote>`; 1 `<details>`; 8 `<article>`, 19 `<section>`; 21 `className` literals
(grep census); 2 inline `style=` objects; **0 `.css` files** (`find console -name '*.css'` → 0).

## 2. Method

1. **Design contract first.** Read the spec's token/typography/shell/content/color recipes so
   every finding names the recipe that addresses it (queue table in §6).
2. **Surface inventory.** Every rendered JSX element of the shell, 16 workspaces, and 11 shared
   components was read and classified into a surface class (banner, nav, table, dl, list, badge,
   form control, pre, verbatim block).
3. **Default-rendering impact analysis.** For each surface class, what the browser default does
   wrong (density, hierarchy, alignment, state visibility) was recorded with `file:line`.
4. **State-visibility sweep.** All rendered state vocabularies (14 command states, 6 quality
   states, availability, certificate-expiry treatments, alarm severity, solver outcomes, plugin
   lifecycle/health) were located and checked for any visual differentiator (element, marker,
   class, attribute) a stylesheet could key on.
5. **Pin audit.** Every recommendation was checked against the 42 written-unrun test files:
   which wording/testid is pinned verbatim, and whether additive classes/elements can collide.
6. **A11y-visual pass.** Focus, contrast-carrying structure, touch-target sizes, hover-only
   affordances, reduced-motion exposure, dark-mode completeness — all assessed statically.
7. **Quantification.** Banner-word volumes, cell counts, and nav geometry measured via the
   `python -I` census (numbers quoted in findings are from that run).

## 3. Checks performed, all consistent

1. **No stylesheet anywhere** — `find console -name '*.css' -o -name '*.scss'` → 0 files;
   `main.tsx` (19 lines) imports no CSS; `index.html` carries no `<style>`/`<link rel=stylesheet>`;
   `package.json` declares no style tooling. The only styling in the entire tree is two inline
   `style={{ marginRight: '0.5rem' }}` objects (Administration.tsx:183,195). **Confirmed** (FRV1-R9-001).
2. **Every table has a caption** — 23/23 `<table>` elements carry a `<caption>` as the first
   child (Overview.tsx:228/335, Assets.tsx:229, Schedules.tsx:266/381, SimulationLab.tsx:215/341/397/427/681/784,
   MarketPrograms.tsx:96, PluginsIntegrations.tsx:120, PluginsWorkspace.tsx:286, ResearchWorkspace.tsx:276,
   Administration.tsx:124/160/206, AdminManagementSection.tsx:62/124, alarmsPanel.tsx:197,
   ValueType.tsx:166). **Clean.**
3. **Every table header cell is scoped** — 102/102 `<th>` carry `scope="col"`. **Clean.**
4. **dt/dd balance** — all 7 `<dl>` (Assets 8/8, Sites 4/4, Portfolios 4/4, PluginsWorkspace 9/9,
   ResearchWorkspace 9/9, SitesEdgeStatus 5/5) are balanced; the apparent 39-dt/29-dd gap in a
   naive count is attribute-bearing `<dd>` tags (`<dd data-testid=…>`). **Clean.**
5. **Nav semantics** — all 12 registry items render as `<a>` with conditional
   `aria-current='page'` (App.tsx:218-235; registry.ts:27-40); `<nav aria-label="Workspaces">`
   pinned at a11yPrinciples.test.tsx:42. **Clean** (geometry defects recorded as FRV1-R9-005).
6. **Heading hierarchy count** — exactly one `<h1>` (App.tsx:192); every workspace opens with one
   `<h2>`; 16 h2 for 12 routes + 4 additive sections. Two outline anomalies recorded (FRV1-R9-010).
7. **Live-status state semantics** — three distinct render branches with distinct ARIA roles:
   open/connecting → `role="status"`, paused → `role="alert"` (App.tsx:261-285). **Clean**
   (visual differentiation recorded as FRV1-R9-007's sibling case).
8. **No hover-only affordance, no animation** — grep found no `onMouseOver`-gated content, no
   CSS (hence no transition/animation). `prefers-reduced-motion` is vacuously satisfied today;
   the design contract introduces the first motion (pulse dot), so the gate is a wave-2
   requirement (FRV1-R9-009), not an existing defect.
9. **States are text-borne** — all 14 command states render a distinct marker + exact spelling
   (stateVocabulary.ts:146-160: `· » ✔ ✕ ⏱ ⌫ ⇄ ⇅`), quality renders the literal code, unknown
   values render verbatim with UNKNOWN markers. Colour can therefore be decoration, never the
   carrier (P5.12 discipline holds). **Clean** (the missing *visual* layer is FRV1-R9-003).
10. **Pin audit for wave-2 CSS hooks** — the tests pin **text content and data-testids, not
    class lists**: statePrinciples/designPrinciples/a11yPrinciples assert `toContain(...)` of
    badge text (`[proposed ·]`, `[measured]`, `quality-…` testids); honestyStructural.test.ts pins
    source-containment of constants and imports (e.g. `dispatch.tsx` must contain
    `SIMULATION_ONLY_BOUNDARY_STATEMENT`, lines 115-120); a11yPrinciples.test.tsx:30 pins the
    class string `skip-link`; tenantBanner.test.tsx:103 pins that App.tsx does NOT contain
    `className="tenant-indicator"`. **Conclusion: additive `className` attributes and attribute-keyed
    CSS (`[data-state]`, `[data-testid^=…]`) break no pin.** Existing class hooks already in the DOM:
    `value-type value-type-<x>` (ValueType.tsx:61), `quality quality-<x>` (ValueType.tsx:80),
    `data-state` on the command badge (honesty.tsx:156), `data-quality` (honesty.tsx:181),
    `availability-<state>` testid (availability.tsx:118), `certificate-expiry-<treatment>` testid
    (SitesEdgeStatus.tsx:177), `command-path-<state>` testid (SitesEdgeStatus.tsx:261). **Clean.**
11. **Banner/honesty wording pins** — every long banner note is a verbatim pinned constant:
    `SESSION_BANNER_NOTE` (sessionBanner.test.tsx:89-92), `TENANT_SOURCE_NOTE` /
    `NO_TENANT_SWITCH_NOTE` / `SINGLE_TENANT_NOTE` (tenantBanner.test.tsx:94),
    `MODE_ENFORCEMENT_NOTE`/`MIXED_MODE_NOTE`/`NON_EXERCISED_MODES_NOTE` (modeControl.test.tsx:217-219),
    `SIMULATION_ONLY_BOUNDARY_STATEMENT` (honestyStructural.test.ts:102-107). **No recommendation
    below changes any wording**; progressive-disclosure proposals keep the text rendered (closed
    `<details>` content still serialises, so `toContain` pins on rendered HTML still hold).
12. **Form semantics** — all 10 buttons `type="button"`; the two confirmation checkboxes sit
    inside `<label>` with `aria-required="true"` (ConfirmHighImpact.tsx:138-150,
    SitesEdgeStatus.tsx:304-316, PluginsWorkspace.tsx:374-387, ResearchWorkspace.tsx:343-356);
    the disabled Sign-in control carries `aria-disabled="true"` (sessionBanner.tsx:81-88).
    **Clean** (sizing/affordance defects recorded as FRV1-R9-008/015).
13. **Skip link + main landmark** — skip link is the first focusable element
    (App.tsx:188-190), `main#workspace-main` with `tabIndex={-1}` (App.tsx:243);
    pinned at a11yPrinciples.test.tsx:30-36. **Clean.**
14. **Route composition** — all 12 registry ids resolve (App.tsx:89-107); the four additive
    panels (SitesEdgeStatus, PluginsInventory, ResearchStatus, AdminManagement+DenialLog)
    render in sequence below their host workspace, so styling must treat *two* `<section aria-label>`
    roots per composed route (affects the "one h2 per route" assumption in the shell recipe).
    **Noted** (feeds FRV1-R9-002/010).

## 4. Findings

### FRV1-R9-001 [P0] The console ships zero stylesheets: every rendered surface is browser-default, so the product reads as an unstyled prototype

- Status: OPEN
- Location: `console/src/main.tsx:1-18` (no CSS import — the Vite template's
  `import './index.css'` was removed, never replaced); `console/index.html:1-19` (no
  stylesheet, no `color-scheme`); `find console -name '*.css'` → **0 files**; the only styling
  in the tree is `console/src/workspaces/Administration.tsx:183,195`
  (`style={{ marginRight: '0.5rem' }}`).
- Issue: the semantic layer is excellent (23 captioned tables, 39 scoped dt/dd rows, 102 scoped
  `<th>`, 12-item labelled nav, honest state text) but the visual layer is absent, and browser
  defaults actively damage each surface class:
  - **Header/banners** (`console-header`, `mode-banner`, `mode-gate`, `tenant-indicator`,
    `session-banner`, `live-status`, `honesty-banner`, `control-authority-mode`,
    `denial-diagnostics`): all render as indistinguishable black-on-white paragraphs at 16px,
    full window width (≈1900px on a desktop), no grouping, no accent, no hierarchy — the
    safety-critical mode banner (App.tsx:193-198) looks identical to the descriptive prose
    beneath it (Dispatch.tsx:690-695). The `honesty-banner` class (7 uses: MarketPrograms.tsx:69,
    PluginsIntegrations.tsx:73, PluginsWorkspace.tsx:90, SimulationLab.tsx:146,
    ResearchWorkspace.tsx:99, Administration.tsx:84, AdminManagementSection.tsx:113) has no
    recipe at all in the design contract (§5 names `.demo-data`/`.freshness`/`.availability`/
    `.denial-diagnostics`/`.control-authority-mode` but not `.honesty-banner` — gap to fix in wave 2).
  - **Tables (the console's core — 197 cells)**: no borders, no row rhythm, default caption
    centered above, left-aligned everything, and unscoped auto table layout — long
    `content_hash`/correlation-id cells (Grid.tsx:174-175,200) blow out sibling columns; a
    9-column table (SimulationLab library, SimulationLab.tsx:215-266) becomes unreadable mush.
  - **dl key–value rows**: `dt`/`dd` render as two stacked blocks at equal weight (Sites.tsx:148-161,
    Assets.tsx:155-193) — the Settings-idiom key-left/value-right row is the whole point of these
    39 pairs and it does not exist.
  - **162 `<code>` spans**: default monospace at full body size, no chip, no clipping — UUIDs and
    SHA-style hashes fracture table layouts and wrap mid-token unpredictably.
  - **Nav**: a 12-item vertical `<ul>` with default bullets occupying ~24 lines before any data
    (see FRV1-R9-005).
  - **States**: plain text; an operator cannot pick FAILED out of a 200-row command list
    (see FRV1-R9-003).
  - **Page geometry**: no `max-width` (the §4 `--content-max: 1080px` recipe), no background
    distinction between chrome and content, no `100dvh` root, `body` margin 8px default.
- Recommendation: implement the design contract as ONE stylesheet imported in `main.tsx`
  (spec §2–§5), keyed on the existing classes/attributes; zero TSX renames required (verified
  §3.10). This is the release-blocking finding: a credible v0.x console cannot ship with default
  rendering of 197 table cells and a 3-viewport-tall unstyled honesty wall.

### FRV1-R9-002 [P1] The design contract's sticky frosted header would pin ~2.9k words of honesty prose over every workspace — the header stack needs a collapse recipe that keeps pinned wording rendered

- Status: OPEN (design-contract interaction; must be resolved before the wave implements §4)
- Location: `console/src/App.tsx:191-217` — the header contains, in order: h1, 3-sentence mode
  banner (:193-198), `ModeGateNotice` (**5 paragraphs** + mode chips, modeGateNotice.tsx:39-66),
  `TenantBanner` (session line + three pinned notes ≈ 1,000 chars, tenantBanner.tsx:168-183),
  `SessionBanner` (lifecycle line + `SESSION_BANNER_NOTE` ≈ 600 chars, sessionBanner.tsx:76-123),
  `LiveStatusLine` (paused branch = 2 sentences + 30-word parenthetical, App.tsx:277-284),
  `ValueTypeLegend` (`<details>`, collapsed — the only collapsed element, ValueType.tsx:161-209).
  Spec: design_spec_console_ios.md §4 `.console-header: position: sticky`.
- Issue: measured by the census, the header's always-visible prose is ≈ **2,900 words /
  ~19,000 characters across ~12 paragraphs** before the first data cell. Styled at the spec's
  13–15px body, that is roughly **2–3 viewport heights of sticky chrome**; with `position: sticky`
  it never scrolls away. Every workspace loses more than half its usable viewport to persistent
  prose — the exact "density for a data console" failure the contract's §7 acceptance bar
  forbids. The wordings are pinned verbatim (§3.11), so the fix cannot delete or shorten text.
- Recommendation: (a) style-only minimum — footnote scale (13px), `--label-secondary`,
  980px max-width, tightened rhythm; (b) contract amendment — make **only the h1 + live-status
  row sticky** (or a compact one-line summary bar) and let the banner stack scroll; (c) where a
  DOM change is acceptable, wrap the *secondary notes* (tenant's three notes, session note,
  mode-gate's last four paragraphs) in additive `<details>` blocks **without touching the
  constants** — closed-details content still serialises, so the `toContain` pins
  (sessionBanner.test.tsx:89, tenantBanner.test.tsx:94, modeControl.test.tsx:217-219) hold by
  construction. Do NOT re-word anything; the honesty wall's *content* is the product.

### FRV1-R9-003 [P1] Command states, quality states, severities, and treatment states have no visual differentiation — but CSS-only hooks already exist for almost all of them

- Status: OPEN
- Location (renderers): `CommandStateBadge` honesty.tsx:134-166 (text + `data-state`, **no
  class**), `QualityBadge` honesty.tsx:172-185 (`data-quality`), `QualityStateBadge`
  ValueType.tsx:68-86 (`className="quality quality-<x>"` — hook exists), `ValueTypeBadge`
  ValueType.tsx:59-65 (`value-type value-type-<x>` — hook exists), `SeverityBadge`
  alarmsPanel.tsx:182-192 (**no hook**: testid carries the alarm *code*, not the severity),
  `AvailabilityLine` availability.tsx:117-123 (`availability-<state>` testid),
  certificate-expiry `<dd data-testid="certificate-expiry-<treatment>">` SitesEdgeStatus.tsx:177-183,
  command-path `<div data-testid="command-path-<state>">` SitesEdgeStatus.tsx:261-286,
  plugin lifecycle/health `<strong>[STATE]</strong>` PluginsWorkspace.tsx:232-264 (no hook),
  solver outcomes Schedules.tsx:293-303 (no hook).
- Issue: an operator scanning the Dispatch list (Dispatch.tsx:759-777) or the audit trail
  (AuditReports.tsx:502-519) must read every row to find a FAILED/REJECTED entry; the only
  differentiator is 8 unicode markers buried in bracketed prose (`[failed ✕]` vs
  `[completed (simulated) ✔]`, stateVocabulary.ts:154-156). Plain text cannot support
  at-a-glance triage of a lifecycle where FAILED and COMPLETED are both terminal. Same problem
  for the frozen six quality states, alarm CRITICAL/WARNING/INFO, and cert-expiry EXPIRED vs VALID.
- Recommendation: implement the spec §6 color contract **primarily as attribute-keyed CSS** —
  `[data-state="FAILED"]`, `[data-quality="BAD"]`, `[data-testid^="certificate-expiry-"]`,
  `[data-testid^="command-path-"]`, `[class*="quality-"]`, `[class*="value-type-"]` — which
  requires **zero TSX edits** and therefore zero pin risk (§3.10). Add classes only where no
  attribute exists: `SeverityBadge` (alarmsPanel.tsx:184), plugin lifecycle/health
  (PluginsWorkspace.tsx:232-264), solver outcome (Schedules.tsx:299) — each an additive
  `className` that no test pins against. Colors decorate the frozen vocabulary; text stays the
  carrier (P5.12 rule intact).

### FRV1-R9-004 [P1] No tabular numerals anywhere: every timestamp, MW/MWh figure, and count in 197 table cells and 39 dd rows is set in proportional digits

- Status: OPEN
- Location: pervasive; worst cases: schedule horizon pairs
  `2026-01-01T00:00:00.000Z — 2026-01-02T00:00:00.000Z` (Overview.tsx:248-251), per-record
  event/ingest pairs (AuditReports.tsx:283-285), objective values with units
  (Schedules.tsx:311-315, SimulationLab.tsx:560-564), aggregate power
  `<strong>{power.total_mw} MW</strong>` (Overview.tsx:188-194), buffer depth/capacity
  (SitesEdgeStatus.tsx:186-190), benchmark runtime columns (SimulationLab.tsx:397-459),
  margins JSON (Grid.tsx:197-202).
- Issue: proportional figures make digit columns jitter — in a console whose primary objects
  are ISO timestamps and signed MW values, column comparison is the core reading task. The
  contract already mandates `font-variant-numeric: tabular-nums` on "every table cell, dd, and
  metric" (spec §3); nothing implements it.
- Recommendation: the §3 recipe verbatim (one selector: `td, dd, .freshness, .availability,
  code { font-variant-numeric: tabular-nums; }` covers the census surface). No DOM change.

### FRV1-R9-005 [P1] The 12-item workspace nav renders as a full-width bulleted vertical list with ~18px targets and no current-page affordance

- Status: OPEN
- Location: `console/src/App.tsx:218-235` (`<nav aria-label="Workspaces"><ul>` with one `<a>`
  per registry item; `aria-current` set but visually meaningless without styles).
- Issue: browser-default `<ul>` = disc markers + 12 stacked lines ≈ 500px tall on desktop and a
  full-screen block on mobile, pushing all data below the fold before any stylesheet exists.
  Link boxes are text-height (~18px) — below the 44px minimum for the operator's primary
  navigation. `aria-current="page"` is set correctly on all 12 items (verified §3.5) but has no
  visual counterpart, so keyboard users get a "current" announcement that looks identical to
  every other item.
- Recommendation: the spec §4 segmented-control recipe (horizontal scroll strip, pill chips,
  filled selected segment, `:focus-visible` ring, 44px min height, `overflow-x: auto` with
  hidden-but-scrollable scrollbar). CSS-only; `aria-current` selector `[aria-current="page"]`
  is the hook.

### FRV1-R9-006 [P1] Dark mode does not exist: unstyled pages are forced light regardless of `prefers-color-scheme`, and the meta declarations that would let the OS help are absent

- Status: OPEN
- Location: `console/index.html:3-13` — no `<meta name="color-scheme">`, no
  `<meta name="theme-color">`; no stylesheet to carry `@media (prefers-color-scheme: dark)`;
  `main.tsx:1-18` (the import point that must carry the token block).
- Issue: unstyled content paints black-on-white in both OS schemes. For an operations console
  used in control rooms and on night shifts, a forced-light surface with no dark variant is a
  genuine UX defect, not polish. The contract's token table (spec §2) defines both schemes;
  the missing `color-scheme` declaration additionally keeps UA scrollbars/form controls light in
  a dark OS.
- Recommendation: ship `color-scheme: light dark` on `:root` + `<meta name="theme-color">` for
  both schemes together with the token block; the dark values must be verified on the Segoe
  fallback path (Windows/Chrome), per the acceptance bar (spec §7.2). Also fold in the
  `viewport-fit=cover` addition (FRV1-R9-014) so `env(safe-area-inset-bottom)` works.

### FRV1-R9-007 [P1] Alert/status/note/verbatim blocks have no visual species: `role="alert"` error blocks and `role="note"` honesty notes render as ordinary paragraphs

- Status: OPEN
- Location: `ErrorState` honesty.tsx:49-67, `ErrorBlock` workspaceData.tsx:143-161,
  `LoadingState`/`LoadingLine` honesty.tsx:39-46 / workspaceData.tsx:133-140, `EmptyState`
  honesty.tsx:70-87, `HonestEmpty` workspaceData.tsx:174-183, `VerbatimStatement`
  honesty.tsx:113-127 (`<blockquote>`), `FreshnessStamp` honesty.tsx:94-106, capability-denial
  blocks Dispatch.tsx:467-489 / AuditReports.tsx:526-543, `TenantMismatchBadge`
  tenantBanner.tsx:150-156, alarms error/unavailable panels alarmsPanel.tsx:85-105.
- Issue: 20+ `role`-bearing blocks across the console are semantically distinct (alert vs status
  vs note vs verbatim contract statement) but visually identical black prose. On the Dispatch
  workspace alone, six consecutive same-looking paragraphs separate the data from the
  evidence (Dispatch.tsx:690-723). An operator cannot see *that an error occurred* without
  reading; a screen-reader user gets the distinction, a sighted operator does not — the
  inverse of the usual a11y failure, and just as real.
- Recommendation: one system, many accents (spec §5 footnote-row recipe): alerts get the red
  accent + `[role=alert]` selector; status gets neutral; notes get secondary scale;
  `blockquote` verbatim statements get the grouped-card treatment with the label as overline.
  CSS-only (`[role="alert"]`, `[role="status"]`, `[role="note"]`, `blockquote` all already in
  the DOM).

### FRV1-R9-008 [P1] Every interactive control is default-sized: buttons/inputs/selects/radios are below touch-target minimums, and disabled vs enabled is indistinguishable

- Status: OPEN
- Location: 10 `<button>`s — sessionBanner.tsx:81-88 (disabled "Sign in"), :110-116 ("Clear the
  local display"), Dispatch.tsx:726-739 (`<select>` filter), AuditReports.tsx:469-475 (Query
  trail), :487-489 (next page), :522-524 (export), :564-566 (build chain), :681-683 (SvA run),
  SitesEdgeStatus.tsx:319-334, PluginsWorkspace.tsx:389-404, ResearchWorkspace.tsx:359-375
  (record-request buttons); 13 `<input>`s (6 text filters AuditReports.tsx:459-467, 3 selects'
  siblings, 4 checkboxes/radios); 5 `<select>`s (Dispatch.tsx:726, Schedules.tsx:519/541/558,
  PluginsWorkspace.tsx:357).
- Issue: default UA controls are ~24-28px tall — under the 44px operator minimum, in the two
  flows that matter most (dispatch-state filter, audit query). The disabled "Sign in" button
  (sessionBanner.tsx:81-88) and the enabled "Clear the local display" (:110-116) render
  identically except for cursor behaviour — on a display-only banner whose honesty depends on
  "this control does nothing," a grey-less disabled state invites attempts. The
  `checked={false}` confirmation checkboxes reset visually per re-render flow and need a
  visible checked state the moment they are used.
- Recommendation: spec §2 `--radius-control` + 44px min-height control recipe; explicit
  `:disabled` styling (reduced opacity + `not-allowed` cursor + secondary label colour); checkbox
  accent-color from `--blue` (native `accent-color` is enough; no new component). CSS-only.

### FRV1-R9-009 [P1] The design contract introduces the console's first motion (the pulsing live-status dot) — it must ship gated or it breaks the reduced-motion contract on day one

- Status: OPEN (forward-looking; verified no motion exists today — §3.8)
- Location: recipe at design_spec_console_ios.md §4 `.live-status` (pulsing dot, green/orange/red);
  render sites App.tsx:261-285; reduced-motion clause spec §5 ("every animation/transition
  longer than 80ms gated behind `prefers-reduced-motion: no-preference`").
- Issue: the pulse animation is the only planned animation, on the element whose paused state is
  `role="alert"` — i.e. precisely the element a vestibular-sensitive operator must be able to
  read statically. Nothing in the current DOM needs a gate; the risk is entirely in the wave-2
  implementation.
- Recommendation: implement the dot as a static coloured circle by default and add the pulse
  **only inside** `@media (prefers-reduced-motion: no-preference)`; keep the three states
  distinguishable by colour AND the existing text ("connected / connecting… / paused") so the
  motion carries no information (P5.12-consistent).

### FRV1-R9-010 [P2] Heading outline anomalies: an `<h2>` inside a composed subsection and an `<h5>` with no `<h4>` ancestor

- Status: OPEN
- Location: `denialLogDiagnostics.tsx:49` (`<h2>Local denial diagnostics</h2>` — rendered on the
  Administration route *below* `AdminManagementSection`, whose sections use h3/h4:
  AdminManagementSection.tsx:112/117/60); `stepUpApproval.tsx:180` (`<h5>Step-up approval for…</h5>`
  — inside the Dispatch approval flow, whose nearest heading is `<h4>` Dispatch.tsx:493; h5 with
  a missing h4 level); contrast: SimulationLab's composed sections correctly reuse h3
  (SimulationLab.tsx:160/213/279/395/494).
- Issue: the outline jumps two levels down in Administration (h2 → h2 sibling confusion: two h2s
  on one route) and skips a level in Dispatch. Screen-reader users navigating by heading get a
  wrong structure picture for the two newest panels. Cosmetic-vs-semantic: the semantics are
  declared; only the levels are off.
- Recommendation: change `denialLogDiagnostics.tsx:49` h2→h3 and `stepUpApproval.tsx:180` h5→h5→
  wrap in an h4 or demote to h4 — both are additive-minimal single-line TSX edits; grep shows no
  test pins either heading tag (`Local denial diagnostics` and `Step-up approval for` appear in
  no test file).

### FRV1-R9-011 [P2] Two inline `style` objects are the only styling in the codebase and will fight the design system

- Status: OPEN
- Location: `console/src/workspaces/Administration.tsx:183` and `:195`
  (`<code key={action} style={{ marginRight: '0.5rem' }}>` for the action-vocabulary chips and
  the excluded-action chips).
- Issue: the only inline styles in the tree hand-space a chip row that the §5 "badges/capsule
  pill" recipe will restyle anyway; inline margins survive token changes and contradict the
  "one design system keyed on classes" rule the contract sets (spec §1).
- Recommendation: replace with the chip recipe (gap on the parent, no per-child margin); the
  parent `<td>`/`<p>` gets a list-like chip container class. Additive; no test pins the inline
  style (grep: `marginRight` appears in no test).

### FRV1-R9-012 [P2] Three unbounded `<pre>` blocks cause horizontal page overflow with realistic payloads

- Status: OPEN
- Location: Dispatch.tsx:602 (`command-payload` — full persisted JSON, Dispatch.tsx:602),
  AuditReports.tsx:690-692 (`sva-requested` — command payload JSON),
  SitesEdgeStatus.tsx:201-203 (`adapter-health-verbatim` — HealthSnapshot JSON).
- Issue: default `pre` neither wraps nor scrolls; a realistic command payload or health snapshot
  (hundreds of characters) forces horizontal page scroll on every viewport, dragging the whole
  workspace with it — worst on the Dispatch detail view where the pre sits under the journal.
- Recommendation: card-contained `pre` with `overflow-x: auto`, mono stack, hairline border
  (spec §5 code recipe extended to pre). CSS-only; the `data-testid` attributes are untouched.

### FRV1-R9-013 [P2] 162 `<code>` spans include unbreakable long tokens (UUIDs, content hashes, correlation ids) with no overflow strategy

- Status: OPEN
- Location: representative: model content hashes Grid.tsx:174-175, margins JSON Grid.tsx:200,
  correlation ids Grid.tsx:170, Dispatch.tsx:301-303, session reference sessionBanner.tsx:107,
  dev-token format stepUpApproval.tsx:204, endpoint ids AdminManagementSection.tsx:87.
- Issue: without `overflow-wrap`, a 64-char hash or UUID inside a table cell or dl row expands
  its column past the container (the unstyled layout's worst breakage), and once the card
  layout lands the same tokens will clip mid-word with no recovery affordance.
- Recommendation: spec §3 code recipe verbatim (chip + `overflow-wrap: anywhere` on long-ID
  contexts). CSS-only.

### FRV1-R9-014 [P2] `index.html` head is missing the UX meta set: `viewport-fit`, `theme-color`, `color-scheme`, and an icon

- Status: OPEN
- Location: `console/index.html:3-13` — viewport is bare `width=device-width, initial-scale=1.0`
  (no `viewport-fit=cover`, which the spec §4 main recipe's `env(safe-area-inset-bottom)`
  padding silently requires); no `theme-color` (light+dark); no `color-scheme`; no favicon link
  (default `/favicon.ico` 404 in the shipped tree — no icon asset exists in `console/`).
- Issue: safe-area padding no-ops on notched devices, PWA/browser chrome never tints, and the
  console has no identity mark. Small, but all four are one-line additions that wave 2 must not
  forget.
- Recommendation: add `viewport-fit=cover` to the existing viewport meta, `<meta
  name="color-scheme" content="light dark">`, two `theme-color` media-scoped tags, and either a
  simple inline-SVG favicon or the honest absence noted in SUPPORT_MATRIX (no asset invented
  beyond a neutral mark).

### FRV1-R9-015 [P2] The record-selection affordance ("inspect" radios) has low information scent and tiny targets in both list-driven workspaces

- Status: OPEN
- Location: Dispatch.tsx:759-777 (radio per command, label text "inspect"),
  AuditReports.tsx:502-519 (radio per audit record, "inspect").
- Issue: the primary master–detail interaction of the two most operationally dense workspaces is
  a bare default radio followed by the word "inspect," visually disconnected from the record
  text below it (`CommandListItem`/`AuditRecordCard` render *after* the label). Default radios
  are ~16px; the row is not indicated as clickable; the selected record's detail loads far below
  with no visual link back.
- Recommendation: style-only — row-height label (44px), hover/checked row highlight
  (`:has(input:checked)` where supported or `:checked + *` ordering is already label-first so a
  sibling selector works), and a card treatment for the record list items (`.command-list li`,
  `[data-testid="audit-trail"] li`). No DOM change required; if wave 2 reorders label/record for
  visual grouping, keep the label text and testids intact (no test pins the order; grep verified
  `inspect` appears in no test).

### FRV1-R9-016 [P2] Table captions carry multi-sentence honesty prose and render centered — 23 captions need the footnote recipe, and two double as content paragraphs

- Status: OPEN
- Location: all 23 captions; the longest embed full contract sentences with inline verbatim
  statements: alarmsPanel.tsx:197-201 (severity/sort contract), PluginsIntegrations.tsx:120-127
  (5 lines), PluginsWorkspace.tsx:286-292, ResearchWorkspace.tsx:276-283, SimulationLab.tsx:398-401
  and :427-434 (benchmark honesty), ValueType.tsx:166-170 (legend + `TREATMENT_RECORD_NOTE`).
- Issue: default captions are centered bold-ish text — the honesty sentence reads as a title
  rather than the qualifier it is; centered multi-line prose above a data table is exactly the
  mis-hierarchy the §5 recipe (left, footnote, secondary colour) exists to fix. No wording
  changes are needed — placement/typography only.
- Recommendation: spec §5 table recipe (caption above, left, 13px, `--label-secondary`,
  `text-align: left`). CSS-only.

## 5. Release-readiness verdict for the console's visual layer

The console's semantics are release-grade and its honesty layer is pinned and coherent; the
visual layer is at zero. **FRV1-R9-001 blocks a credible release**: 197 unstyled table cells, a
~19k-character unstyled honesty wall, 44px-target violations on the primary nav, and no dark
scheme are not "rough edges," they are the difference between an operator tool and an unstyled
prototype. The good news, verified statically: the design contract can be implemented almost
entirely as CSS keyed on attributes and classes that already exist (`data-state`, `data-quality`,
`quality-*`, `value-type-*`, `aria-current`, `role=*`, testid prefixes), with no pinned text
moved and no class renamed (§3.10). The two places where the contract itself needs adjustment
before implementation are FRV1-R9-002 (sticky header vs the honesty wall — decide the collapse
recipe first) and FRV1-R9-009 (gate the pulse dot). Must-fix set for the wave: R9-001,
R9-002, R9-003, R9-004, R9-005, R9-006; the P2 set is polish that nonetheless cheaply protects
the acceptance bar (overflow, outline, meta).

## 6. Wave-2 implementation queue

| # | Finding | Surface | Design-spec recipe that addresses it | TSX edits needed? |
|---|---|---|---|---|
| 1 | FRV1-R9-001 | Everything | §2 tokens + §3 typography + §4 shell + §5 content (one stylesheet, imported in `main.tsx:4`) | No (CSS only) |
| 2 | FRV1-R9-002 | `console-header` stack | §4 header recipe — **amended**: sticky h1+live-status row only; banner stack scrolled or in additive `<details>` (wording untouched) | Optional additive wrappers |
| 3 | FRV1-R9-003 | state badges, severity, lifecycle/health | §6 color contract via `[data-state]`/`[data-quality]`/`[class*=quality-]`; additive classes only on alarmsPanel.tsx:184, PluginsWorkspace.tsx:232-264, Schedules.tsx:299 | 3 additive `className`s |
| 4 | FRV1-R9-004 | tables, dl, freshness | §3 `tabular-nums` rule | No |
| 5 | FRV1-R9-005 | `nav[aria-label=Workspaces]` | §4 segmented-control strip, `[aria-current="page"]` chip, 44px | No |
| 6 | FRV1-R9-006 | document root | §2 dark token block + `color-scheme`/`theme-color` (with R9-014) | index.html meta only |
| 7 | FRV1-R9-007 | alert/status/note/verbatim | §5 footnote-row family keyed on `[role]` + `blockquote` | No |
| 8 | FRV1-R9-008 | buttons/inputs/selects | §2 `--radius-control` + 44px control recipe, `:disabled`, `accent-color` | No |
| 9 | FRV1-R9-009 | `.live-status` dot | §5 reduced-motion gate (`no-preference` only) | No |
| 10 | FRV1-R9-010 | denial-diagnostics h2, step-up h5 | §4 heading scale (and 2 one-line level fixes) | 2 one-line edits |
| 11 | FRV1-R9-011 | Administration chip rows | §5 capsule-pill recipe; delete inline margins | 2-line edit |
| 12 | FRV1-R9-012 | `<pre>` JSON blocks | §5 code/mono card recipe + `overflow-x: auto` | No |
| 13 | FRV1-R9-013 | `<code>` long tokens | §3 code chip + `overflow-wrap: anywhere` | No |
| 14 | FRV1-R9-014 | `index.html` head | §4 safe-area + §7.2 cross-platform bar | index.html only |
| 15 | FRV1-R9-015 | inspect radios (Dispatch/Audit) | §5 grouped-card list rows + checked highlight | No |
| 16 | FRV1-R9-016 | 23 table captions | §5 caption footnote recipe | No |

Also add to the queue (no finding id — contract gap): the `honesty-banner` class (7 render
sites, §3.1) has no recipe in design_spec_console_ios.md §4/§5; give it a line in the banner
family (same grouped-card recipe, gray accent) before wave 2 styles it ad hoc.
