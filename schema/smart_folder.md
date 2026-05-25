# Smart-folder / user-curation layer  *(rev 1 — drafted for review, NOT self-locked)*

## Domain meaning

A **separate, mutable, user-owned curation layer** over the immutable
detected+analysed aggregates. Keeps `Scene`/`Keyframe` clean (machine output)
while users add their own organisation: **user tags** (distinct from the VLM
`tags`/`labels`), **favorites**, **ratings/notes**, and **smart folders**
(saved dynamic filters). Targets **`Scene`** now; designed to generalise.

## Cross-cutting (locked)

Generic over `Id` (UUIDv7). Wall-clock = `jiff::Timestamp` (ms). Strings =
`SmolStr` (`""`=absent). User curation is **never** mixed into the analysis
aggregates. Conversions deferred.

## Aggregates

### `UserTag<Id>` — a user-defined tag
| field | type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | |
| `name` | `SmolStr` | unique (case-folded); the label users type |
| `color` | `Option<Rgba>` | optional swatch (UI) |
| `created_at` | `jiff::Timestamp` | |

Distinct from VLM `Keyframe.tags` / `Scene.labels` (machine, per-aggregate
strings). User tags are first-class, renameable, recolourable, deduped.

### `SceneAnnotation<Id>` — user curation of one `Scene`
| field | type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | |
| `scene` | `Id` | FK → `Scene.id` (1:1 per scope — see SF-user) |
| `favorite` | `bool` | |
| `user_tags` | `Vec<Id>` | refs → `UserTag` (not inline strings: rename/dedupe) |
| `rating` | `Option<u8>` | e.g. 0–5; `None` = unrated |
| `note` | `SmolStr` | free text; `""` = none |
| `updated_at` | `jiff::Timestamp` | |

Absent annotation = default (not favorite, no tags). Scene stays immutable.

### `SmartFolder<Id>` — a saved dynamic filter
| field | type | notes |
|---|---|---|
| `id` | `Id` (UUIDv7) | |
| `name` | `SmolStr` | |
| `filter` | `SmartFilter` | the predicate (see SF-filter) — membership is **computed**, never stored |
| `sort` | `Option<SortSpec>` | result ordering |
| `pinned` | `bool` | sidebar pin |
| `created_at` · `updated_at` | `jiff::Timestamp` | |

`SmartFilter` (proposed — structured AST, not a DSL string): a boolean tree
`And/Or/Not` of typed predicates over scene + its keyframes + annotation:
VLM `tags`/`subjects`/`objects`/`actions`/`mood` contains; `description`
matches; `detector ==`; `span`/date range; `favorite`; `user_tags` contains;
`rating >=`; quality (`sharpness`/`blur`/…) threshold. Typed AST = validatable
+ projectable to SQL/Mongo/LanceDB; a query-string DSL is the alternative.

### `Collection<Id>` *(optional — "etc.")* — manual static grouping
`{ id, name, members: Vec<Id> (→Scene, ordered), created_at }`. The manual
counterpart to `SmartFolder` (dynamic). Include only if you want manual
playlists/albums too.

## Open questions

- **SF-filter (the crux):** structured **`SmartFilter` AST** (recommended —
  typed, validatable, compiles to each projection's query) vs a stored
  query-DSL string (flexible, but parsing/validation/injection burden).
- **SF-user:** single-user (findit local) → no owner field, `SceneAnnotation`
  1:1 with `Scene`. Multi-user later → add `owner: Id` and key
  curation/smart-folders per user. *Lean: single-user now; design the FK so
  adding `owner` is additive.*
- **SF-target: RESOLVED — scene-level** (you confirmed: "scene level
  smart_folder instead of keyframe level"). `SceneAnnotation`/`SmartFilter`/
  `Collection` operate on `Scene.id`; not keyframe-level. Generalising to a
  second target (e.g. `Media`) is a future, additive change — not modelled now.
- **SF-collection:** include `Collection` (manual) now, or smart-folders +
  favorites only? *Lean: defer `Collection` unless you want manual albums.*
- **Favorite vs tag:** keep `favorite: bool` as a first-class field
  (recommended — it's the highest-traffic filter) vs a reserved system
  `UserTag`. *Lean: first-class bool.*

## Projection notes

- **sqlx**: `user_tag`, `scene_annotation` (FK `scene_id`, `user_tags` →
  `scene_annotation_tag` join), `smart_folder` (`filter` → JSONB AST),
  `collection`/`collection_member`. Smart-folder query = AST compiled to SQL.
- **mongodb**: collections per aggregate; `filter` embedded; smart-folder = AST
  → Mongo query.

**Status: in review (rev 1) — new layer; SF-target RESOLVED (scene-level);
open: SF-filter / SF-user / SF-collection. NOT self-locked.**
