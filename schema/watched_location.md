# `WatchedLocation<Id>` — a monitored source **volume**  *(rev 8 — LOCKED, user-approved)*

## Domain meaning

A storage **volume** the **monitor watches for file events** (create/modify/
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

## Volume-scoped — not folder-scoped *(rev 8 — enforced in the domain type)*

A `WatchedLocation` is **volume-scoped IN THE DOMAIN TYPE**: its monitored
target is a storage **volume**, carried as a bare volume **`Id`** (the volume's
**stable UUID** — the id written to `<mount>/.findit_index/.id`, the same
identity `Location::Local.volume` carries). It is **not** folder-scoped, and
— as of rev 8 — the type *cannot* express a folder scope: the rev-7 docs-only
clarification was rejected because the field was still a `root: Location`,
which carries arbitrary path `components`, leaving `Movies` and `Movies/2024`
watches on one volume representable.

rev 8 therefore **replaces `root: Location` with `volume: Id`**. The folder
path is **not part of this schema** — *which* folder(s) on the volume the
monitor actually walks is **application-layer configuration**; the
application layer ensures the monitor watches the correct folder. The
`WatchedLocation`'s **identity / uniqueness is the volume**: there is exactly
**one `WatchedLocation` per volume** (unique constraint on `volume`).

Storage volumes are **disjoint and do not nest**: no volume is a subtree of
another, so two `WatchedLocation`s can never overlap. This is precisely what
makes each [`MediaFile`](media_file.md)'s **single** `watched_location_id` FK
unambiguous — a file copy lives on exactly one volume, so exactly one
`WatchedLocation` discovers it, and the deletion-cascade (`ON DELETE CASCADE`
on `MediaFile.watched_location_id`) is safe with no overlapping-watch
ambiguity. The single-FK + cascade design depends on this volume-scoping; it
would *not* hold for nestable folder-level watches — which is why volume-
scoping is now enforced by the type, not merely documented.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Wall-clock = `jiff::Timestamp` (ms). Strings =
`SmolStr`. Conversions deferred.

## Fields

| field | domain type | wire origin | notes |
|---|---|---|---|
| `id` | `Id` (UUIDv7) | `*.id: bytes` | canonical identity |
| `volume` | `Id` (UUIDv7) | volume id | **stable volume identity** — the UUID written to `<mount>/.findit_index/.id`, the same id `Location::Local.volume` carries. **Not a folder path** — the watch is volume-scoped; which folders are walked is application-layer config. Uniqueness key: one `WatchedLocation` per volume |
| `recursive` | `bool` | recursive | descend subdirectories |
| `enabled` | `bool` | enabled | actively watched/scanned |
| `is_ejectable` | `bool` | `whichdisk` | the volume is removable/ejectable (Windows = `DRIVE_REMOVABLE`); set at resolve time. Drives monitor behavior: ejectable-absence = expected/transient, non-ejectable-absence = `VolumeNotAvailable` `last_error` |
| `added_at` | `jiff::Timestamp` | added_at: i64 | when this watch was configured |
| `last_reconciled_at` | `Option<jiff::Timestamp>` | last_scan: i64 | last full **reconcile sweep** (bootstrap / catch-up after monitor downtime or volume remount — events missed while offline). **Kept (your call)** |
| `last_reconcile_status` | `Option<ScanStatus>` (enum) | status | `Ok`/`Partial`/`Failed` of that sweep — locked `ScanStatus` |
| `last_error` | `Option<ErrorInfo>` | error | monitor-health failure (e.g. `FolderNotAvailable`/`VolumeNotAvailable`/`LocalPermissionDenied` — locked `ErrorCode`); `ErrorInfo {code, message}` |

*(`media_count` removed entirely — your call: no schema-derived truth, and a
monitor doesn't count/own media. "What exists" = `Media` by content hash.)*

## Invariants

`id` non-nil; `volume` non-nil (a watch with no volume identity cannot be
monitored — `try_new` rejects both with `WatchedLocationError::NilId` /
`NilVolume`). `volume` is unique across all `WatchedLocation`s (one watch per
volume). No `root`/path `components` — folder scope is application-layer, not
schema. No `include_globs`/`exclude_globs`, no `media_count`, no `Media`
back-link (your calls). The aggregate is monitor *config + health* only — it
owns no media facts.

## Monitor model (your clarification — not a scanner)

`WatchedLocation` configures the **file-event monitor**: watch the `volume`
(FS notifications); on a create/modify/delete/move event, **trigger (re)index
of the affected file or subfolder**. Content-addressed, so re-index re-hashes →
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

**Removable volumes (your point).** `volume` is the **stable UUID at
`<mount>/.findit_index/.id`** (survives eject/replug + mount-point changes —
old indexer: `whichdisk::resolve`). The single **`is_ejectable: bool`** field
(set from `whichdisk` at resolve time — no separate `Volume` aggregate, your
call) governs behavior: an ejectable volume going away is
**expected/transient** (monitor pauses it; remount → reconcile sweep), a
*non-ejectable* volume vanishing is the real `VolumeNotAvailable`
`last_error` case.

## Resolved (your calls)

- **WL-scope:** **keep** `WatchedLocation` in mediaschema.
- **WL-link:** *(updated rev 6)* still **no `Media` ↔ `WatchedLocation`
  link** (content is hash-keyed; a path is not content identity) — but the
  content/copy split *does* link the per-copy
  [`MediaFile`](media_file.md) to its discovering `WatchedLocation` via the
  non-optional `MediaFile.watched_location_id` FK, with a WL-deletion
  cascade. The link is on the file-copy side, not on `Media`.
- **WL-loc:** *(updated rev 8)* the monitored target is now a bare volume
  `Id`, not a `Location`. Object-storage support (a non-local "volume"
  identity) is a later pass; `WatchedLocation` is local-volume only for now.
- **WL-sched:** **out of scope now** — watch-vs-poll / debounce / event-type
  config is runtime, not schema; only `enabled` kept.
- **`media_count` DROPPED** (your call — no truth; not the monitor's purpose).

## Resolved (cont.)

- **WL-vol:** **no `Volume` aggregate** (your call — reversed). The volume is
  carried as a bare `volume: Id` on `WatchedLocation` itself (rev 8), plus a
  single **`is_ejectable: bool`** field set from `whichdisk` at resolve time
  (re-derivable, stable, so denormalization is harmless). No `volume.md`, no
  `label`/`last_seen_at`/`last_mount_point`. The same volume identity also
  appears in locked `Location::Local.volume` (unchanged).
- **WL-recon:** **keep** `last_reconciled_at` + `last_reconcile_status:
  ScanStatus` (bootstrap / after-downtime / volume-remount catch-up). Locked
  `enums.md` `ScanStatus` retained (still used).

## Projection notes

- **sqlx**: `watched_location` table; `id` PK; `volume` column with a
  **UNIQUE** constraint (one watch per volume); `is_ejectable`/
  `last_reconcile*`/`last_error` columns. No path/`components` column (folder
  scope is application-layer). No `Media` FK/join, no count. The
  `media_file.watched_location_id` FK references this table **`ON DELETE
  CASCADE`** (rev 6 cascade rule) — deleting a watch drops its file copies.
- **mongodb**: `_id`=UUIDv7; single collection. The cascade is enforced by
  the indexer (delete the watch's `media_file` documents in the same op).

**Status: LOCKED (rev 8) — user-approved.** *(rev 8: the rev-7 docs-only
"volume-scoped" clarification was **rejected** in Codex round 3 because the
domain type still carried `root: Location`, leaving folder-path components —
hence `Movies` vs `Movies/2024` watches on one volume — representable. rev 8
**enforces volume-scoping in the type**: the `root: Location` field is
replaced by a bare `volume: Id` (the stable volume UUID), `try_new` loses its
`path` argument and gains a `NilVolume` check, and the schema states one
`WatchedLocation` per volume (unique `volume`). The folder path is no longer
in the schema at all — it is application-layer config; the application layer
ensures the monitor watches the correct folder. `WatchedLocationError` is now
a unit-only `NilId`/`NilVolume` enum (the `Root(LocationError)` newtype
variant is gone).)*

*(rev 7: documented (docs-only) that a `WatchedLocation` is volume-scoped —
superseded by rev 8, which enforces it in the type.)*

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
