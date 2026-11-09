<!-- P1.18 — Protobuf/Buf governance -->

# Protobuf governance (P1.18)

Internal contracts (gRPC/Protobuf) are governed by Buf from day one
(report §10.5, §15.1, ADR-008).

## Recorded decision: protos land in Phase 7

**No `.proto` files exist in Phase 1.** The internal worker RPC contracts
arrive with the first Python worker (Phase 7). `buf.yaml` and
`buf.gen.yaml` land NOW so the first schema is born under lint +
breaking-change discipline; CI (P1.27) runs `buf lint` and
`buf breaking` (against the previous commit) as required gates — a
breaking change fails CI unless a documented migration note is attached
(SemVer discipline per report §21.4; the 0.x line may break contracts
with a migration note; v1.0.0 freezes public contracts for 1.x).

## What is governed

- Lint: `STANDARD` rules on every schema.
- Breaking: `FILE` rules against the base branch; violations fail CI.
- Generation: `buf.gen.yaml` pins the prost (Rust) and Python plugins at
  Phase 7 activation — versions recorded then, never floated.
- No silent field repurposing: a semantic change is a new field/new
  version, never a reuse (mirrors the flexcore-events serialization rule).

## Layout

```
proto/            this module (the .proto files land here)
  buf.yaml        lint + breaking configuration
  buf.gen.yaml    generation plan (activates in Phase 7)
```
