# Domain primitives — newtypes  *(rev 5 — LOCKED, user-approved; `LanguageCode`→`mediaframe::Language`)*

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
| `Language` | **`::mediaframe` extern** (your call — renamed from `LanguageCode`, moved to `mediaframe`; supersedes the `medialang`-crate plan) | `language: string` | a BCP-47 tag `{ language, script: Option, region: Option }` — wraps `icu_locid` subtags (validates ISO-639-**1/2/3** + optional script/region; `Copy`, no-alloc). Engines fill only what they emit: whisper/`asry` (`whispercpp::Lang`) = bare ISO-639-1 (+`yue`), script/region=`None`; subtitle/container fill the full tag. **`pt-BR`≠`pt-PT` are distinct `Language` values** (drives track selection). Engine `Lang`/`Region`/`Script` stay engine-internal, boundary-convert in |
| `Rgba` | `u32` (packed RGBA) | `tag/color: u32` | cheap-unambiguous redesign (`Tag→Rgba`); accessors `.r/.g/.b/.a` |
| `Location` | **oneof** (copied verbatim from `findit-proto::common::location`) | `Location` | `Local { volume: Id, components: Vec<SmolStr> }` (volume identity + platform-agnostic path components) · *(future)* `RemoteUrl`. **Not** an opaque string — structured so removable/remote volumes work (pairs with `ErrorCode::*Volume*` + [watched_location.md](watched_location.md)). `#[non_exhaustive]` |
| `ErrorInfo` (VO) | `{ code: ErrorCode, message: SmolStr }` | `ErrorInfo` | verified vs `findit-proto::common::error_info`; `code` = `ErrorCode` enum ([enums.md](enums.md)); the **sole error signal** (no `error_status`) — *which-stage/why* = `code`; "failed" iff `index_errors` non-empty |

**Strings:** all domain string fields are `SmolStr` (small-string-optimized),
not `String` (locked; upstream `buffa` SmolStr support requested — `anthropics/
buffa#127`; until then the domain↔wire conversion does the `String→SmolStr`).

**Externs (not redefined here):** media-time newtypes (`mediatime::{Timestamp,TimeRange,Timebase}`) are `::mediatime`; pixel/colour/
frame (`Dimensions`, `Rect`, `PixelFormat`, `ColorInfo`, `HdrStaticMetadata`,
`Rotation`, `SampleAspectRatio`, **`Rational`** (P-rat resolved — `{num:u32,
den:NonZeroU32}` lives in mediaframe `0.1.0`, *not* a mediaschema newtype), the
colour enums, the descriptor enums + `TrackDisposition`); **and EXIF/capture
metadata** — **`GeoLocation`** `{lat:f64, lon:f64, altitude:Option<f32>}`
(owned, decimal degrees, ISO-6709 parse/format; P-geo resolved → mediaframe,
your call), device make/model, capture-time, lens/exposure; **and
`Language`** (BCP-47, renamed from `LanguageCode`, moved to mediaframe —
supersedes the `medialang`-crate plan) — are `::mediaframe` (see the mediaframe
extern rule in [README.md](README.md) +
[mediaframe-candidates.md](mediaframe-candidates.md); FFmpeg-n8.1-numbered +
`Unknown(u32)` + `DOMAIN_EXT_BASE`; mediaframe = ex-`videoframe` PR #2, now
PR #3 `0.1.0`).

**Shared cross-cutting VOs (defined once in [README.md](README.md), not here):**
`Provenance {model_name,model_version,prompt_version,indexer_version}` and
`LocalizedText {src,translated}`. **No domain `Embedding` VO** — similarity
vectors live in **LanceDB** keyed by `id` (locked; the earlier shared-`Embedding`
idea is cancelled).

## Resolved

- **P-loc:** `Location` = the **structured oneof copied verbatim from
  `findit-proto::common::location`** (your directive): `Local {volume:Id,
  components:Vec<SmolStr>}` + future `RemoteUrl`. Not an opaque string.
- **P-rat:** `Rational` is a **`::mediaframe` extern** (`{num:u32,
  den:NonZeroU32}`, in `0.1.0`) — no mediaschema `Rational`.
- **P-geo:** `GeoLocation` → **`::mediaframe`** (your call: mediaframe also
  owns EXIF/capture-metadata vocab). Not a mediaschema type; supersedes the
  earlier "mediaschema owns `GeoLocation`" README bullet.
- **P-lang (rev 5, user-authorized reopen):** **`LanguageCode` → renamed
  `Language`, moved to `::mediaframe` extern** — supersedes the earlier
  "mediaschema-owned wrapper / future `medialang` crate" plan. `Language {
  language, script: Option, region: Option }` (a BCP-47 tag) wrapping
  `icu_locid` subtags (ISO-639-1/2/3 + optional script/region; `Copy`,
  no-alloc). Rationale: track/stream language *is* media-stream descriptor
  vocab (sits with codec/disposition already in mediaframe); one fewer crate;
  `whispercpp::Lang` (ISO-639-1, asry's source) stays engine-internal and
  boundary-converts in. Name `Language` (not "Code" → implies bare ISO-639-1;
  not `Locale` → implies collation/formatting); script/region optional so
  `pt-BR`≠`pt-PT` are distinct values.
- `ErrorInfo`/`ErrorCode` added (verified vs `findit-proto`); stale `Embedding`
  VO removed (→ LanceDB).

## Open questions

- *(none — all resolved; ready for your lock.)*

**Status: LOCKED (rev 5) — user-approved.** P-loc (structured `Location`
copied from old indexer) · P-rat (`Rational`→`::mediaframe`) · P-geo
(`GeoLocation`→`::mediaframe`, EXIF/capture charter) · **P-lang rev 5**:
`LanguageCode`→**`mediaframe::Language`** (renamed + moved to mediaframe,
supersedes the `medialang`-crate plan; BCP-47, wraps `icu_locid`;
`whispercpp::Lang` boundary-converts in) · `ErrorInfo`/`ErrorCode` added (verified vs
`findit-proto`) · `Embedding` removed (→ LanceDB). *(rev 5 = user-authorized
reopen of r4 for the LanguageTag move/rename.)*
