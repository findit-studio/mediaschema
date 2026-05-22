# `WatchedLocation<Id>` — a monitored source root  *(rev 7 — LOCKED, user-approved)*

## Domain meaning

A filesystem root the **monitor watches for file events** (create/modify/
delete/move). On an event it **triggers (re)index of the affected file or
subfolder** — content-addressed, so re-index re-hashes and dedups. It is **not
a scanner and not a media owner**: no media count, no scan rollup. Pure
monitor config + monitor health. (Supersedes findit-proto's scan-rollup
framing.)

**WL ↔ file linkage (rev 6).** The content/copy split adds a per-copy
[`MediaFile`](media_file.md) record, and **each `MediaFile` *is* linked to
the `WatchedLocation` that discovered it** via the non-optional
`MediaFile.watched_location_id` FK. This is a *file-copy* link, not a
content link: a path is discovery provenance, not identity, so there is
still **no `Media` ↔ `WatchedLocation` link** — the content row is keyed by
hash. `WatchedLocation` itself stays config + health only (it stores no
back-list of files); the link lives on the `MediaFile` side.

## Volume-scoped — not folder-scoped *(rev 7 clarification)*

A `WatchedLocation` is **volume-scoped**: its `root` *is* a storage volume,
identified by that volume's **stable UUID** (the id written to
`<mount>/.findit_index/.id`, `Location::Local.volume`). It is **not**
folder-scoped — the application-layer monitor decides *which folder within
the volume* to actually watch, but the `WatchedLocation`'s **identity is the
volume**, not that folder.

Storage volumes are **disjoint and do not nest**: no volume is a subtree of
another, so two `WatchedLocation`s can never have overlapping roots. This is
precisely what makes each [`MediaFile`](media_file.md)'s **single**
`watched_location_id` FK unambiguous — a file copy lives on exactly one
volume, so exactly one `WatchedLocation` discovers it, and the
deletion-cascade (`ON DELETE CASCADE` on `MediaFile.watched_location_id`)
is safe with no overlapping-watch ambiguity to resolve. The single-FK +
cascade design depends on this volume-scoping; it would *not* hold for
nestable folder-level watches.

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
identity. The schema therefore models **no `Media` ↔ `WatchedLocation`
link, no media count, no scan rollup** — "what exists" (content) lives in
`Media`, keyed by hash. The *file copy* discovered through a watch **is**
recorded — as a [`MediaFile`](media_file.md) with a non-optional
`watched_location_id` FK back to this `WatchedLocation` (rev 6).
(Supersedes findit-proto's scan-rollup framing.)

**Cascade rule (rev 6).** Deleting a `WatchedLocation` **cascades** to its
`MediaFile`s: every file copy discovered through that watch is removed
(`ON DELETE CASCADE` on `MediaFile.watched_location_id`). This is what makes
the `MediaFile.watched_location_id` FK safely **non-optional** — it can
never dangle. A `Media` content row is *not* deleted by the cascade: it
persists as long as **≥1** `MediaFile` still references it, and becomes
reclaimable only once its last copy is gone.

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
- **WL-link:** *(updated rev 6)* still **no `Media` ↔ `WatchedLocation`
  link** (content is hash-keyed; a path is not content identity) — but the
  content/copy split *does* link the per-copy
  [`MediaFile`](media_file.md) to its discovering `WatchedLocation` via the
  non-optional `MediaFile.watched_location_id` FK, with a WL-deletion
  cascade. The link is on the file-copy side, not on `Media`.
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
  no count. The `media_file.watched_location_id` FK references this table
  **`ON DELETE CASCADE`** (rev 6 cascade rule) — deleting a watch drops its
  file copies.
- **mongodb**: `_id`=UUIDv7; single collection. The cascade is enforced by
  the indexer (delete the watch's `media_file` documents in the same op).
- **graphql**: management surface only (CRUD watch folders + monitor
  health/last-reconcile); a "files discovered here" resolver may join
  `media_file` on `watched_location_id`, but there is still no direct
  `Media` resolver (no `Media` link).

**Status: LOCKED (rev 7) — user-approved.** *(rev 7: documents that a
`WatchedLocation` is **volume-scoped** — its `root` is a storage volume
identified by the volume's stable UUID, and volumes are disjoint/non-nesting.
No schema change; this makes explicit the assumption the single-FK +
cascade design already relied on, so the "overlapping watched roots"
concern does not arise. No Rust code change for this revision.)*

*(rev 6: content/copy split.
The rev-5 "no `Media` ↔ `WatchedLocation` link, no sighting record"
position is **refined** — there is now a per-copy
[`MediaFile`](media_file.md) record, and `MediaFile.watched_location_id`
is the **non-optional** FK linking each file copy to the `WatchedLocation`
that discovered it. Deleting a `WatchedLocation` **cascades** to its
`MediaFile`s, so the FK never dangles; a `Media` content row persists as
long as ≥1 `MediaFile` references it. There is still **no `Media` ↔
`WatchedLocation` link** — content is hash-keyed; the link is on the
file-copy side.)* FS-event monitor (not scanner);
no `Media` link / count / globs (content-addressed); `Local`-only (WL-loc
deferred — `RemoteUrl`/object storage a later pass); WL-sched out; single
`is_ejectable` field (no `Volume` aggregate); `last_reconcile*` kept
(`ScanStatus`). `media.md` EXIF/`GeoLocation` re-review still separately
flagged (independent).
