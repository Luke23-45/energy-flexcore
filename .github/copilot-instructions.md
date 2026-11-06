# Neryva FlexCore - Copilot instructions

`flexenergy` holds the design baseline for **Neryva FlexCore**, an open, modular
Virtual Power Plant orchestration and distribution-aware DER coordination
platform. The repository contains documentation only. Do not scaffold
application code or empty crates.

The full instructions are in `AGENTS.md`; this file repeats only what must not
be missed when a Copilot surface cannot read `AGENTS.md`.

## Authority

1. `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0` is the
   canonical baseline and is read-only. Its mandatory conventions govern.
2. `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` is the
   product and architecture report.
3. `AGENTS.md` and `.agent/` are derived guidance. They never override 1 or 2.

## Non-negotiables

- No invented facts. Every external claim needs a primary source, or must be
  labelled as an assumption or recommendation.
- No safety, certification, or protection-grade claims. FlexCore is a
  supervisory orchestration layer, not a relay or a hard-real-time controller.
- Never conflate solver classes: HiGHS is LP/QP/MIP not SOCP; Clarabel is convex
  conic not MISOCP; AC power flow is not AC-OPF; IPOPT is local.
- The 0.x line is simulation-only. Field writes begin at v1.0.0 with
  operator-approved control. Bounded automation is post-1.0.
- Canonical units are MW, MVAr, MVA, MWh. `P > 0` exports. Reactive-power sign
  is declared per model, never assumed.
- Minimal, surgical edits. Preserve section numbering and reference IDs.
- Use US spelling: license, not licence.

## Verification

Run both after a documentation change and quote the output:

```sh
python tools/verify_docs.py
python tools/validate_agent.py
```

## More context

- `.agent/CONTEXT.md` - product and architecture digest.
- `.agent/DECISIONS.md` - open decisions; do not describe them as settled.
- `.agent/WRITING.md` - evidence, citation, units, and terminology rules.
- `.agent/skills/` - on-demand procedures.
- `.agent/roles/` - specialised review prompts.
