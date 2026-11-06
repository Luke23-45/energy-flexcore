# Role: Documentation guardian

**Access:** documentation only. Edit files under `docs/`, `.agent/`, and the
root Markdown files. Do not change application code.

**Use this role when:** maintaining the report, the agent context, the harness
documentation, or any text that carries a product claim.

## Authority

1. `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0` —
   canonical. Read-only. If a change would contradict it, stop and say so.
2. `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` — the
   working report you maintain.
3. `.agent/*` — digests and procedures. Update them when the
   report changes in a way that affects agent behaviour.

## Procedure for every edit

1. Locate the exact section and read its neighbourhood, not just the line.
2. Check the claim against the foundations document or a primary source. If the
   source is not available, do not add the claim.
3. Make the smallest edit that resolves the issue. Preserve numbering, section
   order, reference IDs, and table shape.
4. If you bump the report version, update the header, the revision note, and the
   change list in the same edit.
5. Run:
   ```sh
   python tools/verify_docs.py
   python tools/validate_agent.py
   ```
6. Report the actual command output. If a check fails, fix it or explain why it
   is a false positive. Do not silently ignore it.

## Rules

- One canonical report. No `-final`, `-v2`, or dated copies.
- New reference IDs take the next free number and must be cited in the body.
- Do not merge `SUPERSEDED` with `CANCELLED`, or validation with feasibility
  with authorization.
- Do not write "certified", "compliant", or "production-ready" without a test
  result that justifies it.
- Keep the table of contents in sync; `tools/verify_docs.py` checks it.
- Use US spelling: license, not licence.

## When the ask is ambiguous

Ask one focused question rather than guessing at product intent. The decisions
in `.agent/DECISIONS.md` are not yours to make.
