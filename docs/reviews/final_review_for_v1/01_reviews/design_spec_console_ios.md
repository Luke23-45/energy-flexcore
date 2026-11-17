# Design Spec — Console Visual System (Apple iOS-Inspired) — Iteration 2

**Status:** DESIGN CONTRACT for the console visual correction (FRV1-R9 wave).
**Date:** 2026-11-18.
**Hard constraints:** NO installs/builds/runs; **no external fonts, no CDN
links, no new npm dependencies** — system font stacks only; existing semantic
class names are NEVER renamed (42 written-unrun test files pin the structure);
TSX edits are additive-only and minimal; `prefers-color-scheme` drives
light/dark; `prefers-reduced-motion` is honored everywhere.

---

## 1. Diagnosis

The console ships **zero stylesheets**: every component renders with semantic
class names but the browser default look. The information architecture,
semantics, and a11y (roles, `aria-current`, skip link, captions) are strong —
the missing layer is purely visual. The fix is one design system keyed on the
existing classes, in the **iOS Settings-app idiom**: inset grouped cards,
hairline separators, tabular numerals, frosted chrome.

## 2. Design tokens (`:root` custom properties, light + dark)

CSS custom properties only; no preprocessor. Light values first, dark inside
`@media (prefers-color-scheme: dark)`.

| Token | Light | Dark |
|---|---|---|
| `--bg-grouped` (page) | `#F2F2F7` | `#000000` |
| `--bg-elevated` (cards) | `#FFFFFF` | `#1C1C1E` |
| `--bg-secondary-fill` | `rgba(120,120,128,.12)` | `rgba(120,120,128,.24)` |
| `--bg-chrome` (frosted header) | `rgba(242,242,247,.82)` | `rgba(22,22,24,.78)` |
| `--label` | `#000000` | `#FFFFFF` |
| `--label-secondary` | `#3C3C43` @ 60% | `#EBEBF5` @ 60% |
| `--label-tertiary` | `#3C3C43` @ 30% | `#EBEBF5` @ 30% |
| `--separator` | `#C6C6C8` @ ~65% | `#38383A` |
| `--blue` | `#007AFF` | `#0A84FF` |
| `--green` | `#34C759` | `#30D158` |
| `--red` | `#FF3B30` | `#FF453A` |
| `--orange` | `#FF9500` | `#FF9F0A` |
| `--yellow` | `#FFCC00` | `#FFD60A` |
| `--teal` | `#30B0C7` | `#40C8E0` |
| `--indigo` | `#5856D6` | `#5E5CE6` |
| `--gray` | `#8E8E93` | `#8E8E93` |

Derived tokens: `--radius-card: 16px`, `--radius-control: 12px`,
`--radius-pill: 999px`, `--hairline: 1px` (drawn at sub-pixel apparent weight
via alpha, never 0.5px borders), `--shadow-card` (very soft, near-invisible in
dark), `--content-max: 1080px`, `--gutter: 20px`.

## 3. Typography

- Font stack (no webfont): `-apple-system, BlinkMacSystemFont, "SF Pro Text",
  "SF Pro Display", "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif`.
  Mono: `"SF Mono", ui-monospace, "Cascadia Code", Menlo, Consolas, monospace`.
- `-webkit-font-smoothing: antialiased`; headings `letter-spacing: -0.015em`
  (SF-style tight tracking); body 15px/1.47 (slightly denser than iOS 17pt
  because this is a data console, per HIG content-first adaptation).
- Scale: h1 (large title) 32px/700; h2 (workspace title) 26px/700;
  h3 (section) 20px/600; h4 17px/600; body 15px; footnote 13px
  (`--label-secondary`); caption/overline 11px/600 uppercase
  `letter-spacing: .06em`.
- **Numbers: `font-variant-numeric: tabular-nums` on every table cell, dd,
  and metric** — alignment is non-negotiable in an operations console.
- `code`: SF Mono stack, 0.85em, `--bg-secondary-fill` rounded 6px chip,
  1px inner padding, never wraps mid-token (`overflow-wrap: anywhere` only on
  long IDs).

## 4. Shell recipes (existing classes)

- `.console`: min-height 100dvh; background `--bg-grouped`; color `--label`.
- `.skip-link`: visually hidden until `:focus` — then a blue pill top-left
  (16px inset), white text, `--radius-pill`, high z-index.
- `.console-header`: **frosted sticky chrome** — `position: sticky; top: 0;
  backdrop-filter: saturate(180%) blur(20px)` (+ `-webkit-` prefix),
  background `--bg-chrome`, hairline bottom border via box-shadow
  `0 0.5px 0 var(--separator)`, padding `12px var(--gutter) 10px`.
  Contains h1 (large title, 32/700, tight tracking), then the banner stack.
  **AMENDED (FRV1-R9-002, design wave D2):** the real header stack carries
  ~2.9k words of pinned honesty prose, so the sticky frosted element is the
  additive `.console-titlebar` (h1 + live-status row) and `.console-header`
  itself is non-sticky — the banners scroll away, the title and live
  connectivity never leave the viewport.
- `.mode-banner`: the safety-critical statement. iOS "sectioned footnote
  card": 13px, `--bg-secondary-fill`, `--radius-control`, 4px leading accent
  bar in `--orange`, icon-dot (pure CSS circle), max-width readable. NEVER
  decorative — it must remain the most visually distinct non-interactive
  element after the title.
- `.mode-gate`, `.tenant-indicator`, `.session-banner`: same grouped-card
  recipe at footnote scale, accent colors: mode-gate `--indigo`,
  tenant `--teal`, session `--gray`; each with its own 4px accent bar.
- `.live-status`: inline status row with a **pulsing status dot** (green =
  open, orange = connecting, red = paused/alert via `[role="alert"]`);
  `@media (prefers-reduced-motion: reduce)` stops the pulse animation;
  13px footnote text.
- `nav` (Workspaces): **iOS segmented-control language** — full-width
  horizontal scroll strip (no wrap, `overflow-x: auto`, scrollbar hidden but
  scrollable), items as pill chips: default transparent w/ `--label-secondary`;
  `[aria-current="page"]` = filled `--bg-elevated` chip (dark: elevated gray)
  with `--label` text, hairline border, subtle shadow — the iOS "selected
  segment" read. `:focus-visible` blue ring. 44px min height per chip.
- `main#workspace-main`: `max-width: var(--content-max)`, centered, gutter
  padding, `padding-bottom: max(24px, env(safe-area-inset-bottom))`.
- `h2` in main: workspace title 26/700 with 4px after; `h3` sections get an
  **iOS grouped-section header treatment**: 13px/600 uppercase overline in
  `--label-secondary`, `letter-spacing: .06em`, margin-top 28px — content
  beneath sits in a card.

## 5. Content recipes

- **Grouped cards**: `section`s and content blocks inside main render as
  inset-grouped cards — background `--bg-elevated`, `--radius-card`,
  `--shadow-card` (light: `0 1px 2px rgba(0,0,0,.06)`), padding 16–20px,
  separated by 12px. Apply via `main#workspace-main > *` card recipe with
  `main > h2, main > h3` excluded (headers stay on canvas).
- **Tables** (93 th/td — the console's core): card container; `caption` above
  as footnote (`--label-secondary`, text-align left, 13px); `thead th`
  sticky within card scroll contexts, 11px/600 uppercase overline style,
  `--label-secondary`, hairline bottom border; `tbody td` 15px tabular-nums,
  hairline row separators (`border-top` on `td` except first row),
  `padding: 10px 12px`; first column left, numeric-ish columns left-aligned
  by default (values carry units in text; no blind right-alignment without
  inspecting content — right-align ONLY via a `.num`-style helper if wave 2
  adds it); `tbody tr:last-child` no separator; row hover =
  `--bg-secondary-fill` at 50% (desktop only, `@media (hover: hover)`).
- **Definition lists** (`dt/dd`, 39 pairs): Settings-style key–value rows —
  `dt` footnote `--label-secondary` left, `dd` body `--label` right-aligned,
  hairline separators between rows, tabular-nums on dd.
- **Lists** (`ul/li`): custom markers — 6px round dot in `--blue` at
  `--label-tertiary` fill, 10px text gap, comfortable 6px row spacing.
- **Badges/status**: `.demo-data` and any inline status spans render as iOS
  capsule pills: 11px/600, `--radius-pill`, tinted fills (color @ 15% alpha
  over card, text in the full color), 1px border color @ 30%.
- `.freshness`, `.availability`, `.denial-diagnostics`, `.control-authority-mode`:
  footnote-row recipes consistent with the banner family (no new visual
  species — one system, many accents).
- **Links**: `--blue`, no underline by default, underline on `:hover`
  + `:focus-visible`; visited same (operations console, no history coloring).
- **Focus**: `:focus-visible` = `0 0 0 3px color-mix(in srgb, var(--blue) 35%,
  transparent)` ring + 1px solid `--blue` inner; never removed, never
  default-outline-only.
- **Scrollbars**: thin overlay style (`scrollbar-width: thin` +
  webkit), thumb `--separator`, track transparent.
- **Selection**: `::selection` blue @ 25%.
- **Reduced motion**: every `animation`/`transition` longer than 80ms gated
  behind `@media (prefers-reduced-motion: no-preference)`.
- **Print** (bonus, cheap): white background, chrome/banners unsticky,
  hairlines preserved.

## 6. Color-coding contract (wave 2)

Command/quality state vocabulary may be color-coded ONLY by adding classes in
TSX additively (e.g. `className="state-chip state-proposed"`), never by
renaming. Palette: PROPOSED/VALIDATED/FEASIBLE `--blue`; APPROVED `--indigo`;
ISSUED/ACKNOWLEDGED/TRACKING `--orange`; COMPLETED `--green`;
REJECTED/FAILED `--red`; CANCELLED/EXPIRED/SUPERSEDED/FALLBACK `--gray`.
Telemetry quality: GOOD `--green`, SUSPECT `--yellow`, BAD `--red`,
MISSING/STALE `--gray`, ESTIMATED `--teal`. Honesty rule: colors decorate the
frozen vocabulary, never reinterpret it.

## 7. Acceptance bar (what "not slop" means here)

1. Zero browser-default surfaces remain; one coherent system everywhere.
2. Both color schemes look native on macOS/iOS Safari AND readable on
   Windows/Chrome (the Segoe fallback path is deliberate).
3. Tabular numbers, hairlines, and grouped insets read as iOS at a glance.
4. No test file modified except where wave 2 adds a class; every structural
   pin in the 42 test files still holds by construction.
5. `prefers-reduced-motion` and `:focus-visible` fully honored.
