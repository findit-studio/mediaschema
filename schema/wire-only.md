# WIRE-only types — no domain twin  *(rev 1 — drafted for review, NOT self-locked)*

The **63 WIRE-only** types from the type audit: RPC/transport envelopes that
exist only in `proto/media/v1/types.proto` (buffa-generated). They are **not**
mirrored in the domain layer and **not** projected to sqlx/mongodb; the
async-graphql API is curated from domain aggregates, not these. Listed by
category so the boundary is explicit (not enumerated one-by-one — they track
the proto, not the domain).

| category | what | why no domain twin |
|---|---|---|
| RPC request/response pairs | `*Request`/`*Response` for each service method | transport DTOs; the handler maps wire ⇄ domain aggregate |
| Streaming chunks | progress/event/chunk messages for streamed RPCs | per-call framing; ephemeral |
| Pagination | cursor/page/`*Page`/`*Connection` wrappers | a query concern; graphql defines its own pagination |
| Error envelopes | RPC status/error-detail wrappers | domain uses `ErrorInfo`; transport wraps it |
| Filter/sort inputs | list-query parameter messages | belong to the query/graphql layer, shaped there |
| Codegen smoke fixtures (3) | `Sp2CodegenSmoke`, `Sp3CodegenSmoke`, … | regression guards only — **excluded from domain** (locked) |

## Rule

- These remain buffa wire types. Domain conversions (`From`/`TryFrom`) are
  defined **only** at the RPC boundary (handler), not as domain mirrors.
- If a "WIRE-only" type later proves to carry genuine domain invariant, it is
  re-bucketed (audit is a hint, not authority) — flag it during review rather
  than mirroring speculatively (YAGNI).

## Open questions

- **W-verify:** confirm none of the 63 hides a domain aggregate (spot-check the
  request/response bodies that embed rich sub-messages — those sub-messages may
  be DOM-value, even though the envelope is WIRE-only).
- **W-graphql:** confirm graphql pagination/filters are defined freshly in the
  graphql projection (not derived from these wire wrappers).

**Status: in review (rev 1) — boundary catalogue; confirm W-verify. NOT self-locked.**
