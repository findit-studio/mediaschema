# `MediaFile<Id>` — one physical copy of a piece of content  *(rev 2 — LOCKED, user-approved)*

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
| `location` | `Location` | structured `Local { volume, components }` — where this copy lives; volume-aware (removable drives), not a path string. The `WatchedLocation` is **volume-scoped** (its root *is* a volume; volumes are disjoint/non-nesting — see [`watched_location.md`](watched_location.md)), so this copy's volume maps to exactly one watch and `watched_location_id` is unambiguous |
| `watched_location_id` | `Id` (UUIDv7) | FK → the [`WatchedLocation`](watched_location.md) that discovered it — **non-optional** (see below) |

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

## Invariants

`id`, `media_id`, `watched_location_id` all non-nil (rejected by `try_new`);
`location.components` non-empty (valid `Local`, enforced by the `Location`
constructor). No `Default` — a `MediaFile` with nil ids is an orphan copy,
same reasoning as `Media`'s "No Default".

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

**Status: LOCKED (rev 2) — user-approved.** *(rev 2: codex PR #13 round-2
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
