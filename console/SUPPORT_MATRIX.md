# Neryva FlexCore — Operator Console: Browser Support Matrix (TARGETS, not verified support)

<!-- P10.14 — additive pointer/config doc. NO existing console source is
     modified. This file records the §21.4 browser TARGETS for the console
     bundle and the honest status of this directory; it records NO tested
     support. The authoritative record, procedure, and (empty) run record
     live in the ledger:
     ledger/phase-10-v1-release-hardening/14_browser_os_matrix.md -->

Status: **NO BROWSER HAS EVER LOADED THIS CONSOLE.** This directory is the
Phase 5 scaffold state (`README.md`): complete source, never installed, never
built, no lockfile, no node_modules. Every result cell in the matrix record
is EMPTY (`<EMPTY — UNRUN>`). Nothing here is a support claim — the support
matrix is published (P10.21) only from filled run results, per §22.3: no
untested cell is listed as supported.

## Browser targets (report §21.4, verbatim row)

The Operator Console (static React/TypeScript bundle served by the control
plane or a CDN) targets **current and previous major versions of Chrome,
Edge, Firefox, and Safari**. The concrete major-version numbers for any
matrix run are determined at run time by the executor and recorded per cell.

## Build toolchain (facts from `package.json` — declared pins, never installed)

- Build: `tsc -b && vite build` — the bundle a matrix run consumes.
- Declared pins: react/react-dom `19.3.1`, typescript `~7.0.0`,
  vite `^8.2.1`, `@vitejs/plugin-react ^5.0.0` — SECONDARY-SOURCE PINS
  (re-verify against the official release pages at the first install, per
  the `x-pin-policy` in `package.json`).
- No lockfile: the first install resolves the caret/tilde ranges; the
  resolved versions are recorded with the matrix results.

## Test procedure (per browser cell — see the ledger record §4)

Load the §20.1 pilot workflows; exercise an approval flow end-to-end (the
approver sees the command, its validation/feasibility evidence, and its
scope/limits BEFORE approving); verify the §20.2 state distinctions render
correctly (proposed setpoints never shown as executed; measured / estimated /
stale / simulated values distinguished; rejected commands carry the failing
constraint and evidence; high-impact actions confirmed explicitly). Record
the exact browser version, OS, and per-check results in the ledger record.
