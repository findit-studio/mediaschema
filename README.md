# mediaschema

Product-agnostic media-primitive schema. The architectural hub is the
hand-written `domain::*` layer (rust-type-conventions, validated
`try_new`-style constructors); every backend — protobuf wire (buffa),
sqlx (3 dialects), MongoDB — is a thin lossless conversion to/from
the domain.

Locked schema docs live under [`schema/*.md`](schema/); they are the
specification the implementation tracks.

## Architectural model

```
                              ┌──────────────────────┐
                              │   domain layer       │
                              │   (validated types)  │
                              └──────┬───────────────┘
                                     │
            ┌────────────────────────┼────────────────────────┐
            │                        │                        │
            ▼                        ▼                        ▼
   ┌──────────────────┐   ┌──────────────────────┐   ┌──────────────────┐
   │  buffa wire      │   │  sqlx row mappers    │   │  mongodb bson    │
   │  (proto3 msgs)   │   │  pg / mysql / sqlite │   │  documents +     │
   │  domain ⇄ wire   │   │  domain ⇄ Row*       │   │  IndexModel      │
   └──────────────────┘   └──────────────────────┘   └──────────────────┘
```

- Every backend's encode side (`From<&Domain> for Backend`) is
  infallible — the domain is the validated side.
- Every backend's decode side (`TryFrom<Backend> for Domain`) routes
  through the same `try_new` + `with_*` builders application code
  uses, so the same invariants are enforced at every wire/storage
  edge.
- All three SQL dialects share a single shape (`Pg* / MySql* /
  Sqlite*` row structs + borrowed `*RowRef<'r>` siblings); the column
  set is the same in all three.

## Aggregate clusters

| cluster | facet | per-track | leaf rows |
|---|---|---|---|
| **Media** | `Media` | — | — |
| **MediaFile** | `MediaFile` | — | — (1:N copies of a `Media`) |
| **WatchedLocation** | `WatchedLocation` | — | — (FS-event monitor) |
| **Audio** | `Audio` | `AudioTrack` | `AudioSegment` (+ `Word`) |
| **Video** | `Video` | `VideoTrack` | `Scene`, `Keyframe` |
| **Subtitle** | `Subtitle` | `SubtitleTrack` | `SubtitleCue<Id, D>` (polymorphic over format: `SrtData` / `VttData` / `AssData` / `LrcData` + sibling aggregates) |
| **Identity** | — | — | `Person`, `Speaker` (per-track diarized voice) |

## Feature flags

Three independent **capability tiers** plus medium-aggregate gates and a
set of optional **backend** features. Capability tiers are additive
(`std` is the default); medium-aggregate gates are independent on/off
flags layered on top.

| flag | tier / role | enables |
|---|---|---|
| _none_ (`--no-default-features`) | no-std + no-alloc | stack-only types (`Uuid7`, `FileChecksum`, `Rgba`, `ErrorCode`, every unit-variant enum + `bitflags!` companion, [`Identified`](crate::Identified) transport envelope). Wire layer **not** compiled. |
| `alloc` (no default) | no-std + alloc | cross-cutting heap-using domain types (`Location`, `ErrorInfo`, `Provenance`, `LocalizedText`, `Media`, `MediaFile`, `Person`, `Speaker`, `WatchedLocation`, `UserTag`, `SceneAnnotation`). |
| `std` (**default**) | std | adds `jiff`-using aggregates (`Speaker`, `WatchedLocation`, …) and `Uuid::now_v7`. |
| `video` (**default**) | medium gate | compiles the `Video` / `VideoTrack` / `Scene` / `Keyframe` aggregate tree + all its sqlx / mongodb backends. Pair with a heap tier (`std` or `alloc`). |
| `audio` (**default**) | medium gate | compiles the `Audio` / `AudioTrack` / `AudioSegment` / `Word` aggregate tree + all its sqlx / mongodb / buffa backends. Pair with a heap tier. |
| `subtitle` (**default**) | medium gate | compiles the `Subtitle` / `SubtitleTrack` / `SubtitleCue` aggregate tree + all its sqlx / mongodb / buffa backends. Pair with a heap tier. |
| `buffa` | wire layer | the buffa-generated `media.v1` messages + the `buffa` ⇄ domain bridge under `mediaschema::buffa::*`. Pair with `std` or `alloc`. |
| `json` | wire JSON | `serde` derives on the wire types (via buffa). Implies `std + buffa`. |
| `arbitrary` | property tests | `arbitrary::Arbitrary` on the wire types. Implies `std + buffa`. |
| `mongodb` | bson backend | bson `Document` ⇄ domain + per-collection `IndexModel` constructors. Implies `std + json`. |
| `sqlx-postgres` | sql backend | postgres `Pg*Row` types + `sqlx::FromRow` derives. Implies `std`. |
| `sqlx-mysql` | sql backend | mysql `MySql*Row` types + `sqlx::FromRow`. Implies `std`. |
| `sqlx-sqlite` | sql backend | sqlite `Sqlite*Row` types + `sqlx::FromRow`. Implies `std`. |

The three medium-aggregate gates (`video` / `audio` / `subtitle`) are
**all enabled in `default`** so out-of-the-box behaviour is unchanged.
Consumers that only need a subset of media — e.g. an analysis engine
that emits `FaceDetection`s but never touches audio or subtitle tracks
— can opt out via `default-features = false` plus a hand-picked subset:

```toml
mediaschema = { version = "0.1", default-features = false, features = ["std", "video"] }
```

Cross-cutting aggregates (`Media`, `MediaFile`, `Person`, `Speaker`,
`WatchedLocation`, `UserTag`, `SceneAnnotation`) plus the
[`Identified<Id, D>`](crate::Identified) transport envelope are
**always available** when a heap tier is on, regardless of which
medium features are selected.

## Quick start

```toml
[dependencies]
mediaschema = { version = "0.1", features = ["std", "buffa"] }
```

Build a domain aggregate, encode it through one of the backends:

```rust
use mediaschema::domain::{Media, MediaKind, Uuid7};
use mediaschema::domain::primitives::FileChecksum;

let m = Media::try_new(
    Uuid7::new(),
    FileChecksum::from_bytes([1u8; 32]),
    /* container */ mediaframe::container::Container::Mp4,
    /* duration */ 12_345,
    MediaKind::Video,
)?;
// ... pass `m` through any backend bridge.
```

See [`examples/`](examples/) for a complete end-to-end domain → backend
encoding round-trip.

## Regenerating wire code

The buffa-generated wire layer lives under `src/generated/` and is
produced from the `.proto` files in `proto/`. Regenerate with:

```bash
cargo run -p xtask -- gen
```

This is required after editing any `proto/**/*.proto` file. Do **not**
hand-edit the generated files.

## Versioning

mediaschema is currently **pre-1.0**. A single Cargo SemVer covers
every surface — Rust API, proto wire, sqlx DDL, mongodb document
shape — and the bump rule depends only on whether the change is
breaking on **any** of them:

- `0.x.y` **patch** — purely additive across **all** surfaces (new
  fields, new proto numbers, new sqlx columns / migrations, new
  mongodb keys, new public Rust items, new `#[non_exhaustive]`
  variants).
- `0.x.0` **minor** — any breaking change on any surface; bumps `x`.
- `1.0.0` — every surface stabilises. From then on, removing or
  renaming any field on any surface requires `2.0`, and proto
  reservation rules switch on (every removed proto field gets
  `reserved N;`, old sqlx migration files become immutable,
  mongodb keys are permanent with a one-major-version grace).

Pre-1.0 — the current state — the [`no-proto-reservations`][noproto]
policy allows free renumbering of proto fields between `0.x` bumps;
this is the trade-off for rapid iteration before any consumer pins
a stable version. See [issue #59][v59] for the full policy + the
three open decisions (cutover timing, mongodb grace period, and
schema-doc-rev formality).

[noproto]: docs/internal/conventions.md
[v59]: https://github.com/Findit-AI/mediaschema/issues/59

## Licence

MIT OR Apache-2.0.
