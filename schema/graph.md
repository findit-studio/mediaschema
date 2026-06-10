# `graph` module — standalone object-graph suite  *(rev 1 — matches the code in `src/graph/`)*

## Domain meaning

The **whole-record programming shape**: a second type suite in which every
media-tree aggregate has a standalone counterpart that owns its full field
set, with the relational plumbing replaced by the children themselves.
`graph::Media` is one self-contained value holding everything that hangs
off a content row. The flat aggregates stay the write/storage shape; graphs
are the read/transfer shape.

## Cross-cutting

Generic over `Id` (UUIDv7 default). Gated on `std` **and** all three medium
features (`video`, `audio`, `subtitle`) — a graph is a complete record;
partial-medium consumers use the flat aggregates. Types are immutable after
construction: lift, then read (no `set_*`/`with_*`).

## Shape rules

- **No embedded `domain::*` aggregates** — graph types are field owners,
  not wrappers.
- **No parent FK fields** (`media_id`, `video_track_id`, …): the parent is
  the node you are inside.
- **No id-vec / facet-link fields** (`files: Vec<Id>`, `scenes: Vec<Id>`,
  `video_id: Option<Id>`): replaced by `files: Vec<MediaFile>`,
  `scenes: Vec<Scene>`, `video: Option<Video>`.
- **Kept:** each node's own `id` (rows, blob keys, vector-store keys,
  targeted commands), and **cross-tree association ids**
  (`Speaker.person_id`, `AudioSegment.speaker_id`,
  `MediaFile.watched_location_id`) — nesting replaces joins, not identity.
- Scalar rollups (`nb_streams`, `nb_chapters`, `total_scenes`,
  `total_segments`, `cue_count`) are kept verbatim.

## Node inventory (13)

| graph type | owns scalars of | children |
|---|---|---|
| `Media` | `domain::Media` | `files: Vec<MediaFile>`, `chapters: Vec<Chapter>`, `video/audio/subtitle: Option<facet>` |
| `MediaFile` | `domain::MediaFile` | — (leaf) |
| `Chapter` | `domain::Chapter` | — (leaf) |
| `Video` | `domain::Video` | `tracks: Vec<VideoTrack>` |
| `VideoTrack` | `domain::VideoTrack` | `scenes: Vec<Scene>` |
| `Scene` | `domain::Scene` | `keyframes: Vec<Keyframe>` |
| `Keyframe` | `domain::Keyframe` | — (leaf) |
| `Audio` | `domain::Audio` | `tracks: Vec<AudioTrack>` |
| `AudioTrack` | `domain::AudioTrack` | `segments: Vec<AudioSegment>`, `speakers: Vec<Speaker>` |
| `AudioSegment` | `domain::AudioSegment` | — (leaf; `speaker_id` association kept) |
| `Speaker` | `domain::Speaker` | — (leaf; `person_id` association kept) |
| `Subtitle` | `domain::Subtitle` | `tracks: Vec<SubtitleTrack>` |
| `SubtitleTrack` | `domain::SubtitleTrack` | `cues: Vec<SubtitleCue>` |
| `SubtitleCue` | `domain::SubtitleCue<Id, SubtitleCueDetails<Id>>` | — (leaf; type-erased payload form) |

## Construction: lifting (flat → graph)

`X::try_from_flat(expected_parent, flat, children…) -> Result<X, GraphError>`
(trait form: `TryFrom<(parent_id, flat[, children])>`; the root takes the
`MediaFlat` tuple alias). Every lift validates the flat child's parent FK
against the parent it is nested under (`GraphError::ParentMismatch`, with
`NodeKind` naming the relation); `Media` additionally checks its stored
facet links against the embedded facets, both directions
(`GraphError::FacetLinkMismatch`). The FKs are consumed by that validation
and do not exist in the graph shape. Leaf children lift inside their
parent's lift; mid-tree children arrive pre-lifted.

**Totality:** a graph is assembled fully or not at all — "everything that
currently exists in storage". Empty child vecs are unambiguous because
per-track `index_status` says which stages have run. No partial-hydration
states.

## Decomposition (graph → flat)

`From<graph::Media> for domain::Media` is the root-row projection (facet
links + reverse-lookup id vecs re-derived from the embedded children);
`From<(parent_id, graph::X)> for domain::X` re-attaches each child node.
Child-id vecs are re-derived from the nested children before they drop —
convert children first when persisting a tree. Reconstruction goes through
`pub(crate)` `*Parts::rehydrate`-style constructors (invariant-carrying;
graph values are lift-validated), so there is no public validation bypass.

## Drift guard (compile-time)

Every flat aggregate has a public-field `XParts` data-transfer struct (the
conversion-boundary exception to the encapsulation rule) and an exhaustive
`into_parts()`. Lifts destructure the `Parts` exhaustively (no `..`) and
move every field. Adding a domain field is a compile error in three places
(`into_parts`, `XParts`, the lift) until the graph mirrors it.

## Deferred (tracked follow-ups)

- Bridge tree fetch/store (`fetch_media_graph` / `store_media_graph`) —
  query-layer work in the sqlx/mongo bridges.
- Nested-document MongoDB encoding for graphs (schema decision: one nested
  document per media vs the current flat collections).
- Wire (`media.v2`) nested messages — blocked on the wire regeneration
  against the locked schema docs.
- `BlobRef` externalization of `Keyframe.data` before scene-and-below
  graphs are assembled at scale.
