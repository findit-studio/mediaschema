# WIRE-only types — no domain twin  *(rev 1 — LOCKED, user-approved)*

The **63 WIRE-only** types from the type audit: RPC/transport envelopes that
exist only in `proto/media/v1/types.proto` (buffa-generated). They are **not**
mirrored in the domain layer and **not** projected to sqlx/mongodb. Listed by
category so the boundary is explicit (not enumerated one-by-one — they track
the proto, not the domain).

| category | what | why no domain twin |
|---|---|---|
| RPC request/response pairs | `*Request`/`*Response` for each service method | transport DTOs; the handler maps wire ⇄ domain aggregate |
| Streaming chunks | progress/event/chunk messages for streamed RPCs | per-call framing; ephemeral |
| Pagination | cursor/page/`*Page`/`*Connection` wrappers | a query concern; the query layer defines its own pagination |
| Error envelopes | RPC status/error-detail wrappers | domain uses `ErrorInfo`; transport wraps it |
| Filter/sort inputs | list-query parameter messages | belong to the query layer, shaped there |
| Codegen smoke fixtures (3) | `Sp2CodegenSmoke`, `Sp3CodegenSmoke`, … | regression guards only — **excluded from domain** (locked) |

## Rule

- These remain buffa wire types. Domain conversions (`From`/`TryFrom`) are
  defined **only** at the RPC boundary (handler), not as domain mirrors.
- If a "WIRE-only" type later proves to carry genuine domain invariant, it is
  re-bucketed (audit is a hint, not authority) — flag it during review rather
  than mirroring speculatively (YAGNI).

## Resolved

- **W-verify — PASSED** (spot-checked the rich envelopes in
  `generated/media.v1.types.rs`): `GetIndexedFileResponse` embeds
  `Video`+`Scene` = **already-locked domain aggregates**, correctly referenced
  (WIRE envelope, domain payload). `BrowseResponse`/`SearchRequest` embed
  `BrowseItem`/`Pagination`/`SearchFilter` = genuine **query/transport DTOs**,
  correctly no domain twin. `UpdateAnnotationRequest` embeds `Tag` = transport
  for the curation layer (domain home = `smart_folder.md`). **No domain
  aggregate is hidden inside a WIRE-only type — the audit bucketing holds.**
- **W-query — only `Pagination` confirmable now (your correction):**
  `Pagination` is unambiguously a wire transport wrapper with no domain twin —
  confirmed. **`SearchFilter`/`*Filter`/`Sort`/`BrowseItem` are NOT confirmed
  yet** — their query-layer disposition depends on the **storage layer** (sqlx/
  mongodb) being designed first; the query-layer shaping waits for that
  (task #68 / persistence sub-projects). They stay catalogued here as
  *not domain twins* (the boundary holds), but their final query-layer
  treatment is **deferred to the storage+query layer**, not asserted now.

**Status: LOCKED (rev 1) — user-approved.** Boundary policy: the 63 are
transport, no domain mirror, conversions only at the RPC boundary. W-verify
PASSED (spot-checked — no domain aggregate hidden; the audit bucketing holds).
W-query: `Pagination` confirmed; `SearchFilter`/`Sort`/`BrowseItem`
query-layer disposition **deferred** — the query layer is designed **only
after (a) all storage schema is finished AND (b) the indexer is finished**
(query is downstream of storage + indexer; user sequencing). They remain
catalogued here as not-domain-twins (boundary holds); their query-layer
treatment is settled in that later phase, not a reopen of this lock.
