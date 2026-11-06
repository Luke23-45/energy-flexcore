# Source policy by domain

Where to look first for each class of claim in this repository.

## Power systems and standards

| Claim | Primary source |
|---|---|
| IEEE 1547 requirements | IEEE standard, or a national laboratory's clause-by-clause summary |
| Inverter interconnection profiles | The profile document and the certification body |
| Distribution test feeders | The official feeder data release |
| Power-flow behaviour | The solver's own documentation |

## Protocols

| Protocol | Source |
|---|---|
| OpenADR | OpenADR Alliance specification and conformance programme |
| OCPP | Open Charge Alliance protocol specification |
| IEEE 2030.5 / CSIP | The standard and the regional profile document |
| SunSpec Modbus | SunSpec Alliance model specifications |
| IEC 61850 | IEC standard (paid; cite the part, not a summary) |

## Software and libraries

Prefer, in order: the project repository, the released crate or package
documentation, the changelog. `docs.rs`, `readthedocs`, and the vendor's own
site are acceptable; a tutorial is not.

For Rust release timing, use the official release schedule, not a blog post.

## Regulation

- FERC: the order itself or the commission's own fact sheet.
- National grid codes: the regulator's or system operator's published code.

A regulator's summary is a secondary source for the legal text; the order is
primary.

## Platform and agent tooling

| Topic | Source |
|---|---|
| `AGENTS.md` format | agents.md |
| Agent Skills format | agentskills.io specification |
| Claude Code behavior | code.claude.com documentation |
| Gemini CLI behavior | google-gemini/gemini-cli documentation |
| OpenCode behavior | opencode.ai documentation |

## Red flags

- A URL that "looks right" but was never fetched.
- A version number with no date.
- A standard cited by number but not by clause.
- A capability table copied from a marketing page.
- A claim that a draft is final.
- A claim that a library solves a problem class it does not.
