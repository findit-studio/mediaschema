# Domain primitives — newtypes  *(rev 1 — drafted for review, NOT self-locked)*

Reference doc for the **DOM-primitive** bucket (~10): the small newtypes every
aggregate is built from. Not aggregates (no doc each) — catalogued here so the
rules are stated once.

## Identity model (locked recap)

Structs are **generic over `Id`** (`Media<Id>`, `VideoTrack<Id>`, …) — the
*struct* is generic; **not** a phantom `Id<Video>`. The id value is a
**UUIDv7** (time-ordered, 16 bytes). Single key, no surrogate; the same value is
the physical key in every projection (PG `uuid` · SQLite `BLOB(16)` · MySQL
`BINARY(16)` · Mongo `_id` = UUIDv7 `BinData`, never `ObjectId`). FKs are the
UUIDv7.

| newtype | repr | wire origin | invariant / notes |
|---|---|---|---|
| `Id` (the type param) | UUIDv7 (16 B) | `*.id: bytes` | non-nil; time-ordered; the single key & all FKs |
| `FileChecksum` | `[u8; 32]` | `checksum: bytes` | content hash ≠ identity; **unique index, not PK**; distinct newtype (never interchangeable with `Id`) |
| `LanguageCode` | `SmolStr` | `language: string` | BCP-47 / ISO-639; validated/normalized on construction (open: strictness) |
| `Rgba` | `u32` (packed RGBA) | `tag/color: u32` | cheap-unambiguous redesign (`Tag→Rgba`); accessors `.r/.g/.b/.a` |
| `Location` | `SmolStr` | `path: string` | file path / object-store key / URI; not parsed in the domain (DB stores text) |
| `Rational` | `{ num: i64, den: NonZeroU64 }` | num/den | exact ratios (`frame_rate`, where not VF); `den ≠ 0` invariant |
| `Embedding` (VO) | `{ model: SmolStr, dim: u32, vector: Vec<f32> }` | vector | shared similarity-vector VO (defined in `scene.md`; reused by keyframe/segment) |

**Strings:** all domain string fields are `SmolStr` (small-string-optimized),
not `String` (locked; upstream `buffa` SmolStr support requested — `anthropics/
buffa#127`; until then the domain↔wire conversion does the `String→SmolStr`).

**Externs (not redefined here):** media-time newtypes (`TrackTime`,
`mediatime::{Timestamp,TimeRange,Timebase}`) are `::mediatime`; pixel/colour/
frame (`Dimensions`, `Rect`, `PixelFormat`, `ColorInfo`, `HdrStaticMetadata`,
`Rotation`, `SampleAspectRatio`, the colour enums) are `::videoframe` (see the
videoframe extern rule in [README.md](README.md); FFmpeg-n8.1-numbered +
`Unknown(u32)` + `DOMAIN_EXT_BASE` model per videoframe PR #2).

## Open questions

- **P-lang:** `LanguageCode` strictness — validate to BCP-47 on construction
  (reject junk) vs store-as-seen `SmolStr` newtype with lenient parse. *Lean:
  lenient newtype + `.normalized()`; don't reject unknown tags (real files are
  messy).*
- **P-loc:** `Location` as one opaque `SmolStr` (recommended — fs path / S3 key
  / URL all flow through) vs a typed enum (`Fs`/`Object`/`Url`). *Lean: opaque
  now; enum later if a consumer needs to branch.*
- **P-rat:** is a shared `Rational` needed in mediaschema given SAR/frame-rate
  largely moved to `::videoframe`? *Lean: keep for `frame_rate` VFR pairing if
  it stays MS; else drop.*

**Status: in review (rev 1) — drafted for your one-by-one review. NOT self-locked.**
