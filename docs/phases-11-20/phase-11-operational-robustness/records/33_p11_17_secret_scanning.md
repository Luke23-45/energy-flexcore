# P11.17 — Secret scanning, pre-commit, and CI config

| Field | Value |
|---|---|
| Subtask | P11.17 — gitleaks-class scanning config, pre-commit hook config, and CI job config written and checked in, NEVER run here; findings are a human gate |
| Wave / agent | Phase 11 wave 2, Agent A |
| Status | WRITTEN-UNRUN: configuration only. NO SCAN WAS RUN, and therefore NO FINDINGS EXIST — there is no finding list to report, and none is invented. Every future scan result is a HUMAN GATE INPUT recorded UNRUN; no agent triages, dismisses, or auto-fixes findings. |
| Sources | gitleaks release index (latest tag v8.30.1, checked 2026-11-14 via the GitHub releases page); the repo's existing CI shape (`.github/workflows/ci.yml` already carried a `security-and-supply-chain` job with a gitleaks step since P1.27 — this batch adds the checked-in config, not a new CI system) |
| Files touched | `.gitleaks.toml` (new), `.pre-commit-config.yaml` (new), `.github/workflows/ci.yml` (additive comment on the existing secret-scan step; behavior unchanged) |

## 1. What was checked in

| File | Content | Honest status |
|---|---|---|
| `.gitleaks.toml` | `[extend] useDefault` (the tool's built-in generic rules) + two FlexCore-specific rules (Vault service-token pattern `hvs.`/`hvt.`; an env-style credential-assignment rule with a placeholder allowlist) + a narrowly scoped repo allowlist (the recorded dev-only documentation markers: the dev placeholder literal, the dev ROLE NAME, the compose shim's `$$` escapes; paths limited to the two compose READMEs, this config file, and the phase records) | UNRUN — never executed by any scanner |
| `.pre-commit-config.yaml` | One hook: gitleaks at the pinned rev `v8.30.1`, running with `--config .gitleaks.toml` so local and CI scans share one rule set. Deliberately minimal (no lint/format duplication — CI's `static-checks` already owns those gates) | UNRUN — the `pre-commit` framework and the binary were never installed or executed here |
| `.github/workflows/ci.yml` | Additive comment block on the existing `Secret scan (gitleaks)` step: records the checked-in config, the ASSUMPTION that the action reads `.gitleaks.toml` from the repository root (UNVERIFIED external behavior — the first human run confirms and, if not, wires the config explicitly), and the no-scan-ever-run status. The step's behavior is byte-identical | Config verified by inspection only |

## 2. What was deliberately NOT done

1. **No scan run** — by the loop bans, and because findings are a human
   gate: an agent-produced finding list would be an unsupervised security
   assertion, which the honesty rules forbid.
2. **No allowlist widening** — the allowlist is scoped as narrowly as the
   config shape allows; widening it is a human decision after a real scan.
3. **No tool install** — no gitleaks binary, no pre-commit framework.
4. **No CI behavior change** — the existing P1.27 job structure is
   untouched.

## 3. ASSUMPTION / UNVERIFIED register

1. **UNVERIFIED** — `[extend]`/`[allowlist]` semantics and the
   gitleaks-action's config discovery are third-party behaviors not
   executed here; both `.gitleaks.toml` and the CI comment record this,
   and the first human scan run is the confirmation gate.
2. **UNPINNED-REVERIFY** — the `v8.30.1` rev (release-index latest tag,
   checked 2026-11-14) is pinned in the pre-commit hook and named in the
   CI comment; re-verify at the first human run, the same discipline as
   the compose image tags.
3. **ASSUMPTION** — the custom `hvs.`/`hvt.` Vault-token pattern matches
   current Vault token formats (the P11.16 Vault DEFAULT makes those the
   most likely accidental secret class here); unverified against a live
   scanner.

## 4. Handoff notes

1. **Human gate**: run the scan (`gitleaks detect --config
   .gitleaks.toml` or via the CI job), record the findings UNRUN→filled,
   and triage by hand. If the action does not pick up the root config,
   wire it explicitly in the step.
2. **The dev placeholder**: the historical `flexcore_dev_password` literal
   is now allowlisted ONLY in the marked documentation paths; the compose
   file no longer contains it (P11.16), and a reintroduced copy outside
   the allowlist SHOULD fire the env-style rule — that rule's behavior is
   itself UNVERIFIED until the first scan run.
3. **Version drift**: if the first human run uses a different gitleaks
   version, re-check the config schema for that version before trusting
   the allowlist's scope.
