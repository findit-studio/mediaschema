# `WatchedLocation<Id>` — an indexed source root  *(rev 1 — drafted for review, NOT self-locked)*

## Domain meaning

A filesystem root (watch folder) the indexer monitors and scans for media. Not
a media artifact — an **indexing-source / configuration aggregate** (findit-proto's
watched-location). Owns scan configuration and a maintained scan rollup; every
`Media` discovered under it can back-reference it.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Wall-clock = `jiff::Timestamp` (ms). Strings =
`SmolStr`; path = `Location`. Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `root` | `Location` | path | watched directory root |
| `recursive` | `bool` | recursive | descend subdirectories |
| `enabled` | `bool` | enabled | actively watched/scanned |
| `include_globs` | `Vec<SmolStr>` | include | allowlist patterns (empty = all) |
| `exclude_globs` | `Vec<SmolStr>` | exclude | denylist patterns |
| `added_at` | `jiff::Timestamp` | added_at: i64 | Unix ms |
| `last_scanned_at` | `Option<jiff::Timestamp>` | last_scan: i64 | Unix ms; `0→None` |
| `last_scan_status` | `Option<ScanStatus>` (enum) | status | `Ok`/`Partial`/`Failed` |
| `media_count` | `u32` | — (rollup) | maintained count of `Media` under this root |
| `last_error` | `Option<ErrorInfo>` | error | last scan failure (the one non-track error case here) |

## Invariants

`id` non-empty; `root` non-empty/absolute; globs valid; `media_count` is a
denormalized cache (truth = `Media` rows referencing this id).

## Open questions

- **WL-scope:** does this belong in the *media* domain schema at all, or is it
  an indexing-service config concern kept out of mediaschema? *Lean: keep it —
  findit-proto models it and `Media → source` is a useful provenance/back-ref;
  but it is config-shaped, your call.*
- **WL-link:** add `Media.source: Option<Id>` (→ `WatchedLocation`) for
  provenance/"re-scan this folder's media"? *Lean: yes (Option, non-breaking).*
- **WL-sched:** scan scheduling (interval/cron, watch vs poll) — model here, or
  purely runtime config not persisted in the schema? *Lean: out of scope now
  (runtime), keep only `enabled` + last-scan rollup.*

## Projection notes

- **sqlx**: `watched_location` table; `id` PK; `root` UNIQUE; globs → `text[]`
  or side table; `media_count`/`last_*` columns.
- **mongodb**: `_id`=UUIDv7; globs embedded arrays.
- **graphql**: management surface (CRUD watch folders, scan status/errors).

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
