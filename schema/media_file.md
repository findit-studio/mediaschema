# `MediaFile<Id>` — one physical copy of a piece of content  *(rev 3 — LOCKED, user-approved)*

## Domain meaning

A `MediaFile` is **one physical file copy** on disk. The content/copy split
separates *what* the content is (a [`Media`](media.md) row, one per content
hash) from *where each copy of it sits* (a `MediaFile` row, one per physical
file). Many copies of the same bytes — the same file duplicated across
folders, drives, or watched locations — collapse to **one** `Media` but stay
**N** distinct `MediaFile`s, because each copy carries its own filesystem
identity: file name, path, creation time, and the watch that discovered it.

This resolves the codex review finding: file-level identity is *not*
content-intrinsic, so it cannot live on the content-hash-keyed `Media` row.

## N files ↔ 1 Media

```
WatchedLocation ──< MediaFile >── Media
   (discovers)        (copy)      (content, one per hash)
```

- Re-indexing re-hashes a file. Same hash ⇒ same `Media`; a second copy at a
  different path becomes a second `MediaFile` pointing at that same `Media`.
- `Media.files: Vec<Id>` is the reverse side of `MediaFile.media_id` —
  the set of copies of a given content.
- A `Media` content row persists as long as **≥1** `MediaFile` references it;
  it is reclaimable once its last copy is gone.

## Cross-cutting (locked rules)

Generic over `Id` (single **UUIDv7** key — Postgres `uuid` / Mongo `_id`; FKs
are the UUID). Wall-clock = `jiff::Timestamp` (ms). Strings = `SmolStr`
(`""` = absent — no `Option` for strings). Path = structured `Location`
(`Local { volume, components }`). Conversions deferred.

## Fields

| field | domain type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | the copy's key |
| `media_id` | `Id` (UUIDv7) | FK → the shared [`Media`](media.md) content row |
| `created_at` | `Option<jiff::Timestamp>` | **filesystem** creation time (Unix ms) — copy-specific. **Optional**: many filesystems lack a birth time, and the wire encodes `0` (Unix epoch, ms) as absent, so a 0-ms timestamp normalises to `None` (same treatment as `Media.capture_date`) |
| `location` | `Location` | structured `Local { volume, components }` — where this copy lives; volume-aware (removable drives), not a path string. The `WatchedLocation` is **volume-scoped** (its monitored target *is* a volume `Id`; volumes are disjoint/non-nesting — see [`watched_location.md`](watched_location.md)), so this copy's volume maps to exactly one watch and `watched_location_id` is unambiguous. **Volume consistency**: `location`'s volume must equal `watch_volume` — enforced at construction and on every location/watch change (see below) |
| `watched_location_id` | `Id` (UUIDv7) | FK → the [`WatchedLocation`](watched_location.md) that discovered it — **non-optional** (see below) |
| `watch_volume` | `Id` (UUIDv7) | cached volume identity of the discovering `WatchedLocation` (its `volume`). **Not a separate FK** — duplicates the watch's `volume` so the location setters can re-check the volume-consistency invariant without holding a `WatchedLocation` reference. Set once at construction from the `WatchedLocation` passed to `try_new` |

**`name` is derived, not stored.** The file name is the **last component of
`location`** (`Location::Local.components` is validated non-empty), exposed via
a `name()` accessor. It is *not* a standalone field — keeping a separate
`name: SmolStr` alongside `location` let the two desync via independent
setters. Renaming/moving a copy is therefore done by replacing `location`
(the single atomic rename/move API); the derived `name` follows.

## Why `watched_location_id` is non-optional

Every `MediaFile` enters the index through a `WatchedLocation` scan — a file
copy has no other way to be discovered. There is no "orphan copy" state. And
deleting a `WatchedLocation` **cascades** to its `MediaFile`s (see
[`watched_location.md`](watched_location.md)), so the FK can never dangle.
Modelling it as a plain `Id` rather than `Option<Id>` makes the always-present
invariant unrepresentable-otherwise — `try_new` rejects a nil
`watched_location_id` exactly as it rejects a nil `id` / `media_id`.

## Volume consistency *(rev 3 — codex PR #13 round-3 finding)*

A `MediaFile`'s `location` must sit on the **same volume** as the
`WatchedLocation` that discovered it (the watch is volume-scoped). Round 3
rejected a constructor that took `location` and a bare `watched_location_id`
independently and only nil-checked the id — nothing stopped a file on volume
A from pointing at a watch for volume B.

rev 3 enforces it:

- **`try_new` takes the `WatchedLocation` by reference**, not a bare id. It
  extracts the watch's `id` (still stored as `watched_location_id: Id` — the
  user's explicit decision) and `volume`, and rejects a
  `location.volume() != watch.volume()` pairing with the new
  `MediaFileError::VolumeMismatch` variant.
- **The watch volume is cached** in the `watch_volume` field so the location
  setters can re-validate without a `WatchedLocation` reference.
- **The location setters are fallible**: `set_location` / `with_location` are
  replaced by `try_set_location` / `try_with_location`, which reject a
  cross-volume move with `VolumeMismatch` (leaving `self` unchanged on the
  in-place form). Moving a copy to a different volume is a *new copy under a
  different watch*, not a mutation.
- **Re-pointing the watch is also fallible**: `set_watched_location_id` /
  `with_watched_location_id` (bare-id) are replaced by
  `try_set_watched_location` / `try_with_watched_location`, which take a
  `WatchedLocation` and reject a watch on a volume other than this copy's
  current `location` volume; they update both `watched_location_id` and
  `watch_volume` together so the two can never desync.

## Invariants

`id`, `media_id`, `watched_location_id` all non-nil (rejected by `try_new`);
`location.components` non-empty (valid `Local`, enforced by the `Location`
constructor); `location.volume() == watch_volume` (rejected by `try_new` and
re-checked by every location/watch mutator). No `Default` — a `MediaFile`
with nil ids is an orphan copy, same reasoning as `Media`'s "No Default".

## Projection notes

- **sqlx**: `media_file` table; `id` PK (uuid); `media_id` FK → `media(id)`;
  `watched_location_id` FK → `watched_location(id)` **`ON DELETE CASCADE`**
  (the cascade rule); a nullable `created_at` column; `location` flattened to
  `(volume_id, components)`. No `name` column — it is derived from the last
  path component on read. Index `media_id` (drives the `Media.files`
  reverse lookup) and `watched_location_id`. A `UNIQUE (volume_id, components)`
  constraint models "one copy per path".
- **mongodb**: `_id` = UUIDv7; single `media_file` collection; `media_id` /
  `watched_location_id` stored as `BinData`.
- **graphql**: expose the copy's path/name + a resolver to its `Media`
  content and discovering `WatchedLocation`; opaque external id.
- **volume consistency**: `watch_volume` is a denormalised copy of
  `watched_location(volume)`; it is *not* a separate FK column and is kept in
  lock-step with `watched_location_id` by the fallible domain mutators.

**Status: LOCKED (rev 3) — user-approved.** *(rev 3: codex PR #13 round-3
finding — `MediaFile` could point at a watch for a different volume.
`try_new` now takes the `WatchedLocation` (not a bare id) and rejects a
`location`/watch volume mismatch with the new `MediaFileError::VolumeMismatch`
variant; a cached `watch_volume: Id` field lets the location setters
re-validate; `set_location`/`with_location` and
`set_watched_location_id`/`with_watched_location_id` are replaced by the
fallible `try_set_location`/`try_with_location` and
`try_set_watched_location`/`try_with_watched_location`. `watched_location_id`
stays a stored `Id` field per the user's explicit decision.)*

*(rev 2: codex PR #13 round-2
findings — `name` is now **derived** from `location`'s last component
instead of a standalone field that could desync; `created_at` becomes
`Option<jiff::Timestamp>` with the 0-ms wire sentinel normalised to `None`,
mirroring `Media.capture_date`; and the volume-scoped nature of
`WatchedLocation` is made explicit so the single `watched_location_id` FK is
unambiguous.)*

*(rev 1)* New aggregate from the content/copy split (codex review finding on
PR #13): copy-specific metadata (`created_at`, `location`, discovering watch)
moves off the content-hash-keyed `Media` onto a per-copy `MediaFile`;
N files ↔ 1 `Media`; `watched_location_id` non-optional with a WL-deletion
cascade.
