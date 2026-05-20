# `WatchedLocation<Id>` — a monitored source root  *(rev 5 — LOCKED, user-approved)*

## Domain meaning

A filesystem root the **monitor watches for file events** (create/modify/
delete/move). On an event it **triggers (re)index of the affected file or
subfolder** — content-addressed, so re-index re-hashes and dedups. It is **not
a scanner and not a media owner**: no media count, no scan rollup, no
`Media` link (a path is not identity — your clarification). Pure monitor
config + monitor health. (Supersedes findit-proto's scan-rollup framing.)

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Wall-clock = `jiff::Timestamp` (ms). Strings =
`SmolStr`; path = `Location`. Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `root` | `Location` | path | watched root — **structured `Location`** (locked `primitives.md`: `Local {volume:Id, components}`); volume-aware (removable drives), not a path string |
| `recursive` | `bool` | recursive | descend subdirectories |
| `enabled` | `bool` | enabled | actively watched/scanned |
| `is_ejectable` | `bool` | `whichdisk` | root's volume is removable/ejectable (Windows = `DRIVE_REMOVABLE`); set at resolve time. Drives monitor behavior: ejectable-absence = expected/transient, non-ejectable-absence = `VolumeNotAvailable` `last_error` |
| `added_at` | `jiff::Timestamp` | added_at: i64 | when this watch was configured |
| `last_reconciled_at` | `Option<jiff::Timestamp>` | last_scan: i64 | last full **reconcile sweep** (bootstrap / catch-up after monitor downtime or volume remount — events missed while offline). **Kept (your call)** |
| `last_reconcile_status` | `Option<ScanStatus>` (enum) | status | `Ok`/`Partial`/`Failed` of that sweep — locked `ScanStatus` |
| `last_error` | `Option<ErrorInfo>` | error | monitor-health failure (e.g. `FolderNotAvailable`/`VolumeNotAvailable`/`LocalPermissionDenied` — locked `ErrorCode`); `ErrorInfo {code, message}` |

*(`media_count` removed entirely — your call: no schema-derived truth, and a
monitor doesn't count/own media. "What exists" = `Media` by content hash.)*

## Invariants

`id` non-empty; `root.components` non-empty (valid `Local`). No
`include_globs`/`exclude_globs`, no `media_count`, no `Media` back-link (your
calls). The aggregate is monitor *config + health* only — it owns no media
facts.

## Monitor model (your clarification — not a scanner)

`WatchedLocation` configures the **file-event monitor**: watch `root` (FS
notifications); on a create/modify/delete/move event, **trigger (re)index of
the affected file or subfolder**. Content-addressed, so re-index re-hashes →
same hash = same `Media` (dedup); a path is discovery provenance, not
identity. Therefore the schema models **no `Media`↔`WatchedLocation` link, no
media count, no scan rollup** — "what exists" lives in `Media` (by hash), not
here. (Supersedes findit-proto's scan-rollup framing.)

**Removable volumes (your point).** `root` is a `Local { volume: Id, … }`;
the volume Id is the **stable UUID at `<mount>/.findit_index/.id`** (survives
eject/replug + mount-point changes — old indexer: `whichdisk::resolve`). The
single **`is_ejectable: bool`** field (set from `whichdisk` at resolve time —
no separate `Volume` aggregate, your call) governs behavior: an ejectable
root going away is **expected/transient** (monitor pauses it; remount →
reconcile sweep), a *non-ejectable* root vanishing is the real
`VolumeNotAvailable`/`FolderNotAvailable` `last_error` case.

## Resolved (your calls)

- **WL-scope:** **keep** `WatchedLocation` in mediaschema.
- **WL-link:** **not added** — no `Media.source`, no `MediaLocation`
  aggregate. (Content-addressed; linkage out of schema scope.) Locked
  `media.md` untouched.
- **WL-loc:** **deferred** — `Location` stays `Local` + the thin future
  `RemoteUrl` stub; object storage (structured `Object`/`StorageProvider`) is
  a later pass; locked `primitives.md` **not** reopened. `WatchedLocation` is
  local-root only for now.
- **WL-sched:** **out of scope now** — watch-vs-poll / debounce / event-type
  config is runtime, not schema; only `enabled` kept.
- **`media_count` DROPPED** (your call — no truth; not the monitor's purpose).

## Resolved (cont.)

- **WL-vol:** **no `Volume` aggregate** (your call — reversed). Just a single
  **`is_ejectable: bool`** field on `WatchedLocation`, set from `whichdisk` at
  resolve time (re-derivable, stable, so denormalization is harmless). No
  `volume.md`, no `label`/`last_seen_at`/`last_mount_point`. The volume Id
  indirection still lives in locked `Location::Local.volume` (unchanged).
- **WL-recon:** **keep** `last_reconciled_at` + `last_reconcile_status:
  ScanStatus` (bootstrap / after-downtime / volume-remount catch-up). Locked
  `enums.md` `ScanStatus` retained (still used).

## Projection notes

- **sqlx**: `watched_location` table; `id` PK; `root` UNIQUE on
  `(volume_id, components)` (structured `Location`, `Local` only for now);
  `is_ejectable`/`last_reconcile*`/`last_error` columns. No `Media` FK/join,
  no count.
- **mongodb**: `_id`=UUIDv7; single collection.
- **graphql**: management surface only (CRUD watch folders + monitor
  health/last-reconcile); no "media here" resolver (no schema link).

**Status: LOCKED (rev 5) — user-approved.** FS-event monitor (not scanner);
no `Media` link / count / globs (content-addressed); `Local`-only (WL-loc
deferred — `RemoteUrl`/object storage a later pass); WL-sched out; single
`is_ejectable` field (no `Volume` aggregate); `last_reconcile*` kept
(`ScanStatus`). `media.md` EXIF/`GeoLocation` re-review still separately
flagged (independent).
