# SP2 (`database`) Implementation Plan

> **Mono-consolidation note:** This plan was authored for a 3-package split (`media.v1` / `findit.db.v1` / `findit.net.v1`). Per explicit user directive that split was consolidated — see `docs/superpowers/plans/2026-05-18-mediaschema-mono-consolidation.md` for the current execution plan. The per-batch field/tag/enum content in this file remains **authoritative** (nothing changed there); only the package header, proto file path, import lines, `media.v1.*` FQN prefixes (now bare same-package refs), and xtask `.files()` structure are superseded by the mono outcome. Read the header/Conventions/Cross-package-mechanism sections below as historical context; the Mono-consolidation note in each relevant section explains the delta.

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`. Executed autonomously/continuously per user directive — no per-task human check-in.

**Goal:** ~~Migrate the full `findit-proto/src/database/` type set (incl. `database/audio/` + `database/audio/track/`) into a NEW `findit.db.v1` proto package in `mediaschema`~~ **[Mono-consolidation]** Migrate the full `findit-proto/src/database/` type set into the **single `media.v1` package** in `proto/media/v1/types.proto`, as clean buffa-generated proto3 that uses bare same-package refs (no cross-package refs), with round-trip/JSON/quickcheck tests. `MediaKind`→`DbMediaKind` (same-package rename; SP1's `MediaKind` oneof keeps its name).

**Architecture:** ~~Add a NEW proto file `proto/findit/db/v1/database.proto` (package `findit.db.v1`) that `import`s `media/v1/types.proto` and `mediatime/v1/mediatime.proto`. Compile BOTH protos in one run; buffa-codegen emits `findit.db.v1` into `src/generated/**` as a sibling module of `media.v1` and resolves cross-package refs natively via `super::`-rooted paths (NOT extern).~~ **[Mono-consolidation]** Append SP2 messages/enums directly into `proto/media/v1/types.proto` (package `media.v1`) across **Task 0 scaffolding + 10 dependency-ordered content batches** (§6.6); regenerate checked-in code via the single-entry xtask `.files([proto/media/v1/types.proto])`; extend `src/lib.rs` named re-exports (the `DbMediaKind` alias disambiguates at the Rust crate root; the proto definition itself uses `DbMediaKind`); add tests to `tests/roundtrip.rs`. All former `media.v1.*` cross-package refs by SP2 types become bare same-package refs. `mediatime.v1.*` stays `extern_path`-mapped to `::mediatime` (NOT generated). No `findit-proto` dependency — fidelity is by faithful authoring from spec §6 (read-only reference).

**Tech Stack:** buffa/buffa-types/buffa-build `0.6` + mediatime `0.1.6` (crates.io), protoc 34.0, features `json`/`arbitrary`/`quickcheck`.

**Spec:** `docs/superpowers/specs/2026-05-17-mediaschema-full-migration-design.md` §6 (authoritative field/enum mappings; §6.1–§6.8 incl. §6.8 #8 mono-consolidation note). §2 = locked decisions. Branch: `sp2-database` (off `sp1-common`, stacked).

---

## Cross-package codegen mechanism (historical — superseded by mono-consolidation)

> **Mono-consolidation note:** The cross-package mechanism described here (separate `findit.db.v1` package/file, two-entry xtask `.files()`, native `super::`-rooted cross-package resolution) was the original SP2 approach. Under mono-consolidation all SP2 types are authored into the single `proto/media/v1/types.proto` (package `media.v1`); the xtask `.files()` retains its single entry; no cross-package refs exist; `mediatime.v1` remains the sole extern. The technical description below is retained as historical context (it was verified correct against buffa-build/codegen 0.6.0 and informed the design) but is **no longer the execution path**.

Verified against `buffa-build 0.6.0` (`Config::{files,includes,extern_path,include_file,compile}`) and `buffa-codegen 0.6.0` (`generate`, `generate_package`, `generate_module_tree`, `context::TypePath`):

- buffa-build's `.compile()` runs `protoc --include_imports` over **all** `.files()`, then `buffa_codegen::generate` groups the descriptors **by proto package** (`BTreeMap<package, files>`), emitting per-package output. `generate_module_tree` (driven by the `PackageMod` entries via `include_file("mod.rs")`) builds **one nested `pub mod` tree per package**, all rooted at the same `src/generated/mod.rs`.
- `buffa_codegen::context::TypePath` (context.rs:18–38) resolves a type reference's `to_package` prefix to one of: empty / `super::super` (same package), **`super::…::other_pkg` (cross-package, SAME compilation)**, or `::extern_crate::pkg` (extern only). Therefore, when `media.v1` and `findit.db.v1` are compiled **in the same `compile()` call**, `findit.db.v1` references to `media.v1.*` resolve **natively** to `super::super::super::media::v1::…` (a sibling module under the shared `generated` root) — identical to the existing `media.v1.types.__oneof.rs` referencing `super::super::super::VideoFormat`.
- **Consequence (locked under original plan):** `media.v1` is **NOT** added as an `extern_path` for the db package. Adding it as extern would *suppress generation of the shared tree* and break native resolution. The ONLY xtask change is **adding the new proto file to `.files()`**. `mediatime.v1` stays extern (`.mediatime.v1` → `::mediatime`) and continues NOT to be generated. Both protos already share `includes(&[root.join("proto")])`, so `import "media/v1/types.proto";` and `import "mediatime/v1/mediatime.proto";` resolve.

**Under mono-consolidation:** xtask `.files()` retains the single entry `root.join("proto/media/v1/types.proto")`. The `Sp2CodegenSmoke` fixture (Task 0) becomes a plain same-package `media.v1` message (formerly cross-package `media.v1.ErrorInfo`/`media.v1.VideoFormat` refs are now bare same-package refs; `mediatime.v1.TimeRange` extern is unchanged). Expected `src/generated/` after regen: `media.v1.types.rs` grows with SP2 content; `media.v1.types.__oneof.rs` gains the `SubtitleTrackOrigin.source` oneof (batch 4). **No** `findit.db.v1.*` files are created. **`mediatime.v1` is still NOT generated** — no `mediatime.v1.*` files appear (extern-mapped).

---

## Conventions (all batches)

> **Mono-consolidation note:** Under the mono outcome, substitute `proto/media/v1/types.proto` (package `media.v1`) for every reference to `proto/findit/db/v1/database.proto` (package `findit.db.v1`) below. All `media.v1.<Type>` cross-package FQN refs become bare same-package refs (just `<Type>`). The one-entry xtask `.files([proto/media/v1/types.proto])` is unchanged. `DbMediaKind` is the proto definition name (not just a Rust re-export alias) — the proto definition in `types.proto` is `enum DbMediaKind { … }`, replacing the original `enum MediaKind { … }` for the SP2 database tri-state.

- Append proto into `proto/media/v1/types.proto` (package `media.v1`; **append-only** — every batch adds its block at end of file, never edits a prior block). *(Original: `proto/findit/db/v1/database.proto`, package `findit.db.v1` — superseded by mono-consolidation.)*
- **Reuse, never redefine.** Same-package refs are bare `<Type>` names (e.g. `ErrorInfo`, `Detection`, `Dimensions`, `TrackTime`, `AudioFormat`). `mediatime` refs are written `mediatime.v1.Timebase` / `mediatime.v1.TimeRange` (extern → `::mediatime`). **No SP2 message may redeclare any SP1/SP0 type.** `DbMediaKind` is the proto definition name for the database tri-state (§6.1/§6.8 #8); SP1's `MediaKind` oneof is a different type and keeps its name. *(Original: `media.v1.<Type>` FQN + separate `findit.db.v1` package — superseded.)*
- `Id`/`FileChecksum`/checksum fields are **inline `bytes`** (16-/32-byte newtype convention, §6.1) — NOT `Id`/`FileChecksum` message refs.
- Bitflags are **inline `uint32`** at the use site with the bit layout in a proto comment (§6.4) — no standalone type. `Iso6392B` → inline `uint32`; `CedTag` → inline `fixed64` (§6.2/§6.8). The 5 clap newtype wrappers collapse to `Detection` (bare same-package ref, §6.2). `SubtitleTrackOrigin` is a `message` with a `oneof` (§6.2).
- proto3: every `enum` has a `*_UNSPECIFIED = 0` first value. Singular nested messages → `buffa::MessageField<T>`. `optional` scalar → `Option`. `repeated` → `Vec`. Source tag gaps that are *bounded interior holes* → `reserved` (documents history, blocks silent reuse); *wide deliberate bands* (`AudioAnalysis` 8–9/11–19/…, `TrackRecord` 1000s/2000s) are emitted as sparse field numbers verbatim with NO `reserved` (§6.8 #3).
- After each batch's proto edit: `cargo run -p xtask -- gen` (regenerates `src/generated/**`; expect `generated -> …`; the growing `media.v1.types.rs` with SP2 additions; **no `mediatime.v1` files, no `__view` files**, no new package directories). *(Original: "the new `findit.db.v1.*` files plus unchanged `media.v1.*`" — superseded by mono-consolidation.)*
- Extend `src/lib.rs` with the batch's new public message/enum idents (named, not glob) under the SP2 re-export block (added in Task 0 — see Task 0 Step 4). Oneof companion idents follow the SP1 pattern (`pub use generated::findit::db::v1::<msg_snake>::<OneofEnum> as <Alias>;`).
- Tests use the existing `rt` helper in `tests/roundtrip.rs`:

```rust
fn rt<M: buffa::Message + PartialEq + std::fmt::Debug>(m: &M) {
    let bytes = m.encode_to_vec();
    let back = M::decode_from_slice(&bytes).expect("decode");
    assert_eq!(*m, back, "wire round-trip mismatch");
}
```

Per new type add: a populated instance + `M::default()`, both through `rt`. Construct via the generated owned struct (public fields; nested singular message → `buffa::MessageField::some(...)`; enum fields → `buffa::EnumValue::from(<Enum>::<VARIANT>)`; `repeated` → `vec![...]`; oneof → `Some(<OneofAlias>::<Variant>(...))`). **Before writing each batch's tests, run** `grep -n "pub struct <T>\|pub enum <T>\|pub .*:\|MessageField\|EnumValue\|pub mod <msg_snake>" src/generated/media.v1.types*.rs` for the batch's types and match the EXACT generated field names/wrappers (buffa may raw-ident-escape, wrap enums in `::buffa::EnumValue<…>`, or box oneof arms); adjust constructions to the real generated API. Nested SP1/SP2 types are all in the same `media.v1` package — construct via the `mediaschema::<Type>` re-export. *(Original grep path: `src/generated/findit.db.v1.database*.rs` — superseded by mono-consolidation.)*
- Verification block (run after each batch — uses the cached registry, no network):
```
cd /Users/user/Develop/findit-studio/mediaschema
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo build
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo build --no-default-features
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo build --features json
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo build --features arbitrary
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo build --features quickcheck
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo build --all-features
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo test --features quickcheck,json 2>&1 | grep "test result:"
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo run -p xtask -- gen && git diff --exit-code -- src/generated && echo GEN_CLEAN
```
All 6 builds PASS; all tests pass; `GEN_CLEAN`. If a cargo cmd hits a registry/network error, retry the SAME command with the Bash tool's `dangerouslyDisableSandbox: true` (deps are cached under `/Users/user/Develop/findit-studio/.cargo`).
- Commit each batch on `sp2-database`:
```
git add proto/media/v1/types.proto src/generated src/lib.rs tests/roundtrip.rs
git commit -m "feat(sp2): <batch name> media.v1 SP2-database types"
```
(append `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`. `xtask/src/main.rs` only changes in Task 0; later commits will simply have no xtask diff to stage.)

---

## Task 0: SP2 codegen scaffolding (de-risk cross-package + extern BEFORE any content)

**Files:** create `proto/findit/db/v1/database.proto`; modify `xtask/src/main.rs`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate `src/generated/**`.

This task proves the NEW-package + cross-`media.v1`-ref + `mediatime` extern pipeline end-to-end with ONE tiny smoke message. No content batch proceeds until this is green.

- [ ] **Step 1: Create `proto/findit/db/v1/database.proto`** (header + both imports + ONE smoke message referencing exactly one `media.v1` message, one `media.v1` enum, an inline `bytes` id, and one `optional mediatime.v1.TimeRange`):

```protobuf
syntax = "proto3";

package findit.db.v1;

// SP2 — findit `database` domain. Authored from findit-proto's
// src/database/ Rust as a clean proto3 redesign (NO wire-compat with the
// hand-rolled encoding; correctness = round-trip + semantic fidelity).
//
// REUSE, NEVER REDEFINE: every media.v1.* / mediatime.v1.* type below is
// referenced from its own package. buffa compiles this file together with
// proto/media/v1/types.proto in one run and resolves the cross-package
// refs natively (super::-rooted); mediatime.v1.* is extern-mapped to
// ::mediatime and is NOT generated. findit.db.v1.MediaKind is a DIFFERENT
// type from media.v1.MediaKind (the distinct package disambiguates).

import "media/v1/types.proto";
import "mediatime/v1/mediatime.proto";

// Task-0 codegen smoke fixture (NOT a domain message): proves the
// findit.db.v1 -> media.v1 cross-package reference (message + enum), the
// inline-bytes id convention, and the mediatime.v1.TimeRange extern, all
// resolving in a single buffa codegen run. Removed implicitly once real
// content lands? No — kept: it is the permanent regression guard for the
// SP2 cross-package pipeline (mirrors SP0's TimedDetection fixture role).
message Sp2CodegenSmoke {
  bytes id = 1;                              // inline-bytes id convention (§6.1)
  media.v1.ErrorInfo error = 2;              // cross-package media.v1 MESSAGE ref
  media.v1.VideoFormat format = 3;           // cross-package media.v1 ENUM ref
  optional mediatime.v1.TimeRange range = 4; // mediatime extern (-> ::mediatime)
}
```

- [ ] **Step 2: Apply the xtask edit.** In `xtask/src/main.rs`, replace the line

```rust
        .files(&[root.join("proto/media/v1/types.proto")])
```

with

```rust
        .files(&[
            root.join("proto/media/v1/types.proto"),
            root.join("proto/findit/db/v1/database.proto"),
        ])
```

(No other change to `xtask/src/main.rs`. Do NOT add an `extern_path` for `media.v1` — see "Cross-package codegen mechanism".)

- [ ] **Step 3: Regenerate** — `env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo run -p xtask -- gen`. Expect `generated -> …/src/generated`. Confirm with `ls src/generated`: the new files `findit.db.v1.mod.rs` and `findit.db.v1.database.rs` exist (no `findit.db.v1.database.__oneof.rs` yet — `Sp2CodegenSmoke` has no oneof); the existing `media.v1.*` files are still present and **unchanged** (`git diff --stat -- src/generated/media.v1.types.rs` shows no change); there is **no `mediatime.v1.*` file** and **no `*__view*` file**. Confirm `src/generated/mod.rs` now contains a `pub mod findit { … pub mod db { … pub mod v1 { … include!("findit.db.v1.mod.rs"); } } }` block alongside `pub mod media`.

- [ ] **Step 4: Add the SP2 re-export block to `src/lib.rs`.** After the existing `media.v1` re-export block (the `pub use generated::media::v1::{…};` and its three oneof `pub use` lines), append:

```rust

// SP2 — findit `database` package (`findit.db.v1`). Referenced media.v1
// types are re-exported above (SP1); these are the database-domain owned
// types. Named (not glob) so buffa internals stay private. Re-exported
// under the crate root to match the media.v1 surface; the
// `findit.db.v1.MediaKind` vs `media.v1.MediaKind` name pair is the only
// collision and is resolved by exposing the SP2 one as `DbMediaKind`
// (the proto packages stay distinct; only the Rust re-export alias
// disambiguates at the crate root — see spec §6.8 #6).
pub use generated::findit::db::v1::{
    Sp2CodegenSmoke,
};
```

(Each later batch extends THIS brace list with its new idents. The `DbMediaKind` alias is introduced in Task 1 — see Task 1 Step 3. Oneof companion `pub use` lines, when needed, go immediately after this block, mirroring the SP1 `MediaKindKind`/`LocationKind` pattern.)

- [ ] **Step 5: Add the Task-0 smoke round-trip test** to `tests/roundtrip.rs`. First confirm generated field names: `grep -n "pub struct Sp2CodegenSmoke\|pub .*:\|MessageField\|EnumValue" src/generated/findit.db.v1.database.rs`. Add `Sp2CodegenSmoke` (and any oneof aliases — none here) to the `mediaschema::{…}` import list in `tests/roundtrip.rs`. Then add:

```rust
// ── SP2 Task 0: cross-package + extern codegen smoke ────────────────────────

#[test]
fn sp2_codegen_smoke_roundtrip() {
    use mediaschema::{ErrorInfo, Sp2CodegenSmoke, VideoFormat};
    // Populated: media.v1 MESSAGE ref + media.v1 ENUM ref + inline bytes +
    // mediatime extern, all in one findit.db.v1 message.
    let s = Sp2CodegenSmoke {
        id: vec![0x01, 0x02, 0x03, 0x04],
        error: buffa::MessageField::some(ErrorInfo {
            code: 7,
            message: "smoke".into(),
            ..Default::default()
        }),
        format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MP4),
        range: buffa::MessageField::some(mediatime::TimeRange::new(
            10,
            20,
            mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
        )),
        ..Default::default()
    };
    rt(&s);
    // Default (all empty / no extern set) — guards the absent-extern path.
    rt(&Sp2CodegenSmoke::default());
}

#[test]
#[cfg(feature = "json")]
fn sp2_codegen_smoke_json_roundtrip() {
    use mediaschema::{ErrorInfo, Sp2CodegenSmoke, VideoFormat};
    let s = Sp2CodegenSmoke {
        id: vec![0xAA, 0xBB],
        error: buffa::MessageField::some(ErrorInfo {
            code: 1,
            message: "json".into(),
            ..Default::default()
        }),
        format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MKV),
        range: buffa::MessageField::some(mediatime::TimeRange::new(
            0,
            5,
            mediatime::Timebase::new(1, core::num::NonZeroU32::new(48000).unwrap()),
        )),
        ..Default::default()
    };
    let json = serde_json::to_string(&s).expect("to_json");
    let back: Sp2CodegenSmoke = serde_json::from_str(&json).expect("from_json");
    assert_eq!(s, back);
}
```

(If the grep shows `format` is `::buffa::EnumValue<super::…::VideoFormat>` re-exported as `mediaschema::VideoFormat`, the `EnumValue::from` form above is correct; if `id` is raw-ident-escaped or `Bytes`-typed, adjust per the real signature. `mediatime` is already a dev-dependency and imported in `tests/roundtrip.rs`.)

- [ ] **Step 6: Verify** (Conventions verification block). Expected: all 6 builds PASS, `cargo test --features quickcheck,json` shows all tests passing (incl. `sp2_codegen_smoke_roundtrip` + the SP0/SP1 suite still green), `GEN_CLEAN`. This green state proves the cross-package + extern mechanism; content batches may proceed.

- [ ] **Step 7: Commit** — `feat(sp2): batch0 codegen scaffolding (findit.db.v1 package + cross-package/extern smoke)` (stage incl. `proto/findit/db/v1/database.proto` and `xtask/src/main.rs`).

---

## Task 1: Batch 1 — DB enums + bitflag carriers (no message deps)

All 12 §6.3 owned enums. Bitflags are inline `uint32` (no type here — materialized at use sites in later batches).

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto**

```protobuf
// ── Batch 1: DB-owned enums (§6.3) ──────────────────────────────────────────

// Database asset tri-state. DISTINCT from media.v1.MediaKind (the SP1
// video/audio-format oneof) — kept distinct per spec §6.8 #6.
enum MediaKind {
  MEDIA_KIND_UNSPECIFIED = 0;  // src `Unknown`
  MEDIA_KIND_VIDEO = 1;
  MEDIA_KIND_AUDIO = 2;
}

enum SubtitleTrackFormat {
  SUBTITLE_TRACK_FORMAT_UNSPECIFIED = 0;  // src `Unknown`
  SUBTITLE_TRACK_FORMAT_TEXT = 1;
  SUBTITLE_TRACK_FORMAT_ASS = 2;
  SUBTITLE_TRACK_FORMAT_BITMAP = 3;
  SUBTITLE_TRACK_FORMAT_SRT = 4;
  SUBTITLE_TRACK_FORMAT_VTT = 5;
  SUBTITLE_TRACK_FORMAT_TTML = 6;
  SUBTITLE_TRACK_FORMAT_SAMI = 7;
  SUBTITLE_TRACK_FORMAT_LRC = 8;
  SUBTITLE_TRACK_FORMAT_WHISPER = 9;
}

enum SubtitleTrackRole {
  SUBTITLE_TRACK_ROLE_UNSPECIFIED = 0;  // src `Unknown`
  SUBTITLE_TRACK_ROLE_SUBTITLE = 1;
  SUBTITLE_TRACK_ROLE_CAPTION = 2;
  SUBTITLE_TRACK_ROLE_TRANSCRIPT = 3;
  SUBTITLE_TRACK_ROLE_TRANSLATION = 4;
  SUBTITLE_TRACK_ROLE_LYRICS = 5;
  SUBTITLE_TRACK_ROLE_COMMENTARY = 6;
}

enum ChannelLayoutKind {
  CHANNEL_LAYOUT_KIND_UNSPECIFIED = 0;  // src `Unknown`
  CHANNEL_LAYOUT_KIND_MONO = 1;
  CHANNEL_LAYOUT_KIND_STEREO = 2;
  CHANNEL_LAYOUT_KIND_STEREO_DOWNMIX = 3;
  CHANNEL_LAYOUT_KIND_SURROUND = 4;
  CHANNEL_LAYOUT_KIND_QUAD = 5;
  CHANNEL_LAYOUT_KIND_HEXAGONAL = 6;
  CHANNEL_LAYOUT_KIND_OCTAGONAL = 7;
  CHANNEL_LAYOUT_KIND_HEXADECAGONAL = 8;
  CHANNEL_LAYOUT_KIND_CUBE = 9;
  CHANNEL_LAYOUT_KIND_CH2_1 = 10;
  CHANNEL_LAYOUT_KIND_CH2_1_ALT = 11;
  CHANNEL_LAYOUT_KIND_CH2_2 = 12;
  CHANNEL_LAYOUT_KIND_CH3_1 = 13;
  CHANNEL_LAYOUT_KIND_CH3_1_2 = 14;
  CHANNEL_LAYOUT_KIND_CH4_0 = 15;
  CHANNEL_LAYOUT_KIND_CH4_1 = 16;
  CHANNEL_LAYOUT_KIND_CH5_0 = 17;
  CHANNEL_LAYOUT_KIND_CH5_0_BACK = 18;
  CHANNEL_LAYOUT_KIND_CH5_1 = 19;
  CHANNEL_LAYOUT_KIND_CH5_1_BACK = 20;
  CHANNEL_LAYOUT_KIND_CH5_1_2_BACK = 21;
  CHANNEL_LAYOUT_KIND_CH5_1_4_BACK = 22;
  CHANNEL_LAYOUT_KIND_CH6_0 = 23;
  CHANNEL_LAYOUT_KIND_CH6_0_FRONT = 24;
  CHANNEL_LAYOUT_KIND_CH6_1 = 25;
  CHANNEL_LAYOUT_KIND_CH6_1_BACK = 26;
  CHANNEL_LAYOUT_KIND_CH6_1_FRONT = 27;
  CHANNEL_LAYOUT_KIND_CH7_0 = 28;
  CHANNEL_LAYOUT_KIND_CH7_0_FRONT = 29;
  CHANNEL_LAYOUT_KIND_CH7_1 = 30;
  CHANNEL_LAYOUT_KIND_CH7_1_WIDE = 31;
  CHANNEL_LAYOUT_KIND_CH7_1_WIDE_BACK = 32;
  CHANNEL_LAYOUT_KIND_CH7_1_TOP_BACK = 33;
  CHANNEL_LAYOUT_KIND_CH7_1_2 = 34;
  CHANNEL_LAYOUT_KIND_CH7_1_4_BACK = 35;
  CHANNEL_LAYOUT_KIND_CH7_2_3 = 36;
  CHANNEL_LAYOUT_KIND_CH9_1_4_BACK = 37;
  CHANNEL_LAYOUT_KIND_CH22_2 = 38;
}

enum AudioChannelOrderKind {
  AUDIO_CHANNEL_ORDER_KIND_UNSPECIFIED = 0;  // src `Unspecified`
  AUDIO_CHANNEL_ORDER_KIND_NATIVE = 1;
  AUDIO_CHANNEL_ORDER_KIND_CUSTOM = 2;
  AUDIO_CHANNEL_ORDER_KIND_AMBISONIC = 3;
}

enum AudioClipKind {
  AUDIO_CLIP_KIND_UNSPECIFIED = 0;  // src `Unknown`
  AUDIO_CLIP_KIND_WHOLE_TRACK_SUMMARY = 1;
  AUDIO_CLIP_KIND_VIDEO_SCENE_ALIGNED = 2;
  AUDIO_CLIP_KIND_FIXED_WINDOW = 3;
  AUDIO_CLIP_KIND_EVENT_SPAN = 4;
}

enum AudioPrefilterClass {
  AUDIO_PREFILTER_CLASS_UNSPECIFIED = 0;  // src `Unknown`
  AUDIO_PREFILTER_CLASS_CONTENT = 1;
  AUDIO_PREFILTER_CLASS_SILENT = 2;
  AUDIO_PREFILTER_CLASS_NOISE = 3;
}

enum AudioTrackRole {
  AUDIO_TRACK_ROLE_UNSPECIFIED = 0;  // src `Unknown`
  AUDIO_TRACK_ROLE_MAIN_PROGRAM = 1;
  AUDIO_TRACK_ROLE_COMMENTARY = 2;
  AUDIO_TRACK_ROLE_DUB = 3;
  AUDIO_TRACK_ROLE_DESCRIPTIVE_AUDIO = 4;
  AUDIO_TRACK_ROLE_KARAOKE = 5;
  AUDIO_TRACK_ROLE_LYRICS = 6;
}

enum AudioContainerFormat {
  AUDIO_CONTAINER_FORMAT_UNSPECIFIED = 0;  // inserted (src define_code_type! 0=None)
  AUDIO_CONTAINER_FORMAT_MP4 = 1;
  AUDIO_CONTAINER_FORMAT_MKV = 2;
  AUDIO_CONTAINER_FORMAT_MOV = 3;
  AUDIO_CONTAINER_FORMAT_WAV = 4;
  AUDIO_CONTAINER_FORMAT_MP3 = 5;
  AUDIO_CONTAINER_FORMAT_FLAC = 6;
  AUDIO_CONTAINER_FORMAT_OGG = 7;
  AUDIO_CONTAINER_FORMAT_MKA = 8;
  AUDIO_CONTAINER_FORMAT_WMA = 9;
  AUDIO_CONTAINER_FORMAT_AAC = 10;
}

enum AudioCodec {
  AUDIO_CODEC_UNSPECIFIED = 0;  // inserted
  AUDIO_CODEC_AAC = 1;
  AUDIO_CODEC_FLAC = 2;
  AUDIO_CODEC_OPUS = 3;
  AUDIO_CODEC_MP3 = 4;
  AUDIO_CODEC_PCM_S16LE = 5;
  AUDIO_CODEC_AC3 = 6;
  AUDIO_CODEC_VORBIS = 7;
}

enum AudioSampleFormat {
  AUDIO_SAMPLE_FORMAT_UNSPECIFIED = 0;  // inserted
  AUDIO_SAMPLE_FORMAT_FLTP = 1;
  AUDIO_SAMPLE_FORMAT_S16 = 2;
  AUDIO_SAMPLE_FORMAT_S32 = 3;
  AUDIO_SAMPLE_FORMAT_F32 = 4;
}

enum TrackClassificationType {
  TRACK_CLASSIFICATION_TYPE_UNSPECIFIED = 0;  // src `Unknown`
  TRACK_CLASSIFICATION_TYPE_TIMECODE = 1;
  TRACK_CLASSIFICATION_TYPE_SILENT = 2;
  TRACK_CLASSIFICATION_TYPE_AMBIENCE = 3;
  TRACK_CLASSIFICATION_TYPE_VOICE = 4;
  TRACK_CLASSIFICATION_TYPE_MUSIC = 5;
  TRACK_CLASSIFICATION_TYPE_SOUND_EFFECT = 6;
  TRACK_CLASSIFICATION_TYPE_MIXED = 7;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.

- [ ] **Step 3: Re-exports** — extend the `pub use generated::findit::db::v1::{…};` brace list in `src/lib.rs` with: `AudioChannelOrderKind, AudioClipKind, AudioCodec, AudioContainerFormat, AudioPrefilterClass, AudioSampleFormat, AudioTrackRole, ChannelLayoutKind, SubtitleTrackFormat, SubtitleTrackRole, TrackClassificationType`. For the `findit.db.v1.MediaKind` enum, add the **aliased** re-export (the only name colliding with the SP1 `media.v1.MediaKind` already re-exported at the crate root) by adding, immediately after the `pub use generated::findit::db::v1::{…};` block:

```rust
/// Database asset tri-state. Aliased to avoid colliding with the SP1
/// [`MediaKind`] (the `media.v1` video/audio-format oneof). The proto
/// packages stay distinct (`findit.db.v1.MediaKind`); only this crate-root
/// re-export is renamed — spec §6.8 #6.
pub use generated::findit::db::v1::MediaKind as DbMediaKind;
```

- [ ] **Step 4: Tests** — confirm the generated enum representation: `grep -n "pub enum MediaKind\|pub enum SubtitleTrackFormat\|pub enum ChannelLayoutKind" src/generated/findit.db.v1.database.rs` (enums are plain Rust enums; fields elsewhere wrap them in `::buffa::EnumValue<…>`). Add `DbMediaKind` + the 11 enums to the `mediaschema::{…}` import in `tests/roundtrip.rs`. Because proto enums have no standalone `buffa::Message` impl, exercise them via a tiny in-test wrapper is unnecessary — instead assert variant/discriminant stability + (under `json`) serde. Add:

```rust
// ── SP2 Batch 1: DB enums ───────────────────────────────────────────────────

#[test]
fn batch1_sp2_enum_discriminants() {
    use mediaschema::{
        AudioChannelOrderKind, AudioClipKind, AudioCodec, AudioContainerFormat,
        AudioPrefilterClass, AudioSampleFormat, AudioTrackRole, ChannelLayoutKind,
        DbMediaKind, SubtitleTrackFormat, SubtitleTrackRole, TrackClassificationType,
    };
    // UNSPECIFIED == 0 for every enum; spot-check a max value per enum.
    assert_eq!(DbMediaKind::MEDIA_KIND_UNSPECIFIED as i32, 0);
    assert_eq!(DbMediaKind::MEDIA_KIND_AUDIO as i32, 2);
    assert_eq!(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_UNSPECIFIED as i32, 0);
    assert_eq!(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_WHISPER as i32, 9);
    assert_eq!(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_UNSPECIFIED as i32, 0);
    assert_eq!(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_COMMENTARY as i32, 6);
    assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_UNSPECIFIED as i32, 0);
    assert_eq!(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH22_2 as i32, 38);
    assert_eq!(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_UNSPECIFIED as i32, 0);
    assert_eq!(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_AMBISONIC as i32, 3);
    assert_eq!(AudioClipKind::AUDIO_CLIP_KIND_UNSPECIFIED as i32, 0);
    assert_eq!(AudioClipKind::AUDIO_CLIP_KIND_EVENT_SPAN as i32, 4);
    assert_eq!(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_UNSPECIFIED as i32, 0);
    assert_eq!(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_NOISE as i32, 3);
    assert_eq!(AudioTrackRole::AUDIO_TRACK_ROLE_UNSPECIFIED as i32, 0);
    assert_eq!(AudioTrackRole::AUDIO_TRACK_ROLE_LYRICS as i32, 6);
    assert_eq!(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_UNSPECIFIED as i32, 0);
    assert_eq!(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_AAC as i32, 10);
    assert_eq!(AudioCodec::AUDIO_CODEC_UNSPECIFIED as i32, 0);
    assert_eq!(AudioCodec::AUDIO_CODEC_VORBIS as i32, 7);
    assert_eq!(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_UNSPECIFIED as i32, 0);
    assert_eq!(AudioSampleFormat::AUDIO_SAMPLE_FORMAT_F32 as i32, 4);
    assert_eq!(TrackClassificationType::TRACK_CLASSIFICATION_TYPE_UNSPECIFIED as i32, 0);
    assert_eq!(TrackClassificationType::TRACK_CLASSIFICATION_TYPE_MIXED as i32, 7);
}
```

(If the generated enums are not `#[repr(i32)]`-castable directly, instead assert via `i32::from(<Enum>::VARIANT)` or `<Enum>::VARIANT as i32` per the real generated form found by the grep. The enums are fully round-trip-exercised inside their owning messages in later batches; this batch only locks discriminants. No standalone JSON test — enums are serialized within messages in later batches.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch1 DB enums (MediaKind/SubtitleTrackFormat/SubtitleTrackRole/ChannelLayoutKind/AudioChannelOrderKind/AudioClipKind/AudioPrefilterClass/AudioTrackRole/AudioContainerFormat/AudioCodec/AudioSampleFormat/TrackClassificationType)`.

---

## Task 2: Batch 2 — audio scalar leaves

`TagConfidence, SoundSource, AudioEvent, SpeakerSegment, AudioTranscriptSegment, AudioChannelSpec, Chromaprint, Ebur128, Timecode, CedDetection, Ced`.

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto**

```protobuf
// ── Batch 2: audio scalar leaves (§6.5 leaves table) ────────────────────────

message TagConfidence { string label = 1; float confidence = 2; }

message SoundSource {
  string name = 1;
  string prominence = 2;
  string description = 3;
}

message AudioEvent {
  string event_type = 1;
  uint32 start_ms = 2;
  uint32 end_ms = 3;
  float avg_confidence = 4;
}

message SpeakerSegment {
  uint32 start_ms = 1;
  uint32 end_ms = 2;
  uint32 speaker_id = 3;
}

message AudioTranscriptSegment {
  uint32 start_ms = 1;
  uint32 end_ms = 2;
  string text = 3;
  string language = 4;
  float confidence = 5;
}

message AudioChannelSpec {
  uint32 index = 1;
  uint32 raw_id = 2;
  string label = 3;
}

message Chromaprint {
  bytes fingerprint = 1;
  double fingerprint_duration = 2;
}

message Ebur128 {
  float loudness_lufs = 1;
  float loudness_range_lu = 2;
  float true_peak_dbtp = 3;
}

message Timecode {
  string start = 1;
  string end = 2;
  float fps = 3;
  bool drop_frame = 4;
}

// `tag` is the src `CedTag` u64 newtype inlined as fixed64 (wire
// SixtyFourBit, §6.2/§6.8 #4) — soundevents dataset code.
message CedDetection {
  fixed64 tag = 1;
  float confidence = 2;
}

message Ced { repeated CedDetection tags = 1; }
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `AudioChannelSpec, AudioEvent, AudioTranscriptSegment, Ced, CedDetection, Chromaprint, Ebur128, SoundSource, SpeakerSegment, TagConfidence, Timecode`.
- [ ] **Step 4: Tests** — confirm fields: `grep -n "pub struct TagConfidence\|pub struct CedDetection\|pub struct Ced\b\|pub .*:" src/generated/findit.db.v1.database.rs`. Add the 11 idents to the `mediaschema::{…}` import. Add `batch2_sp2_roundtrip`: populated + `::default()` for each. `CedDetection` with `tag: 0xDEAD_BEEF_0000_1234` (proves `fixed64`); `Ced` with `tags: vec![CedDetection{..}, CedDetection{..}]`; `Chromaprint` with non-empty `fingerprint` bytes; `Timecode` with `drop_frame: true`. Add a `#[cfg(feature="json")]` `batch2_sp2_json_roundtrip` doing a `serde_json` round-trip for one populated `Ced` (covers repeated nested + `fixed64` under serde).

```rust
// ── SP2 Batch 2: audio scalar leaves ────────────────────────────────────────

#[test]
fn batch2_sp2_roundtrip() {
    use mediaschema::{
        AudioChannelSpec, AudioEvent, AudioTranscriptSegment, Ced, CedDetection,
        Chromaprint, Ebur128, SoundSource, SpeakerSegment, TagConfidence, Timecode,
    };
    rt(&TagConfidence { label: "speech".into(), confidence: 0.91, ..Default::default() });
    rt(&TagConfidence::default());
    rt(&SoundSource { name: "rain".into(), prominence: "background".into(), description: "steady rain".into(), ..Default::default() });
    rt(&SoundSource::default());
    rt(&AudioEvent { event_type: "applause".into(), start_ms: 1000, end_ms: 4000, avg_confidence: 0.8, ..Default::default() });
    rt(&AudioEvent::default());
    rt(&SpeakerSegment { start_ms: 0, end_ms: 2500, speaker_id: 3, ..Default::default() });
    rt(&SpeakerSegment::default());
    rt(&AudioTranscriptSegment { start_ms: 100, end_ms: 900, text: "hello".into(), language: "en".into(), confidence: 0.97, ..Default::default() });
    rt(&AudioTranscriptSegment::default());
    rt(&AudioChannelSpec { index: 2, raw_id: 0x10, label: "FL".into(), ..Default::default() });
    rt(&AudioChannelSpec::default());
    rt(&Chromaprint { fingerprint: vec![0x01, 0x02, 0x03, 0x04], fingerprint_duration: 120.5, ..Default::default() });
    rt(&Chromaprint::default());
    rt(&Ebur128 { loudness_lufs: -14.0, loudness_range_lu: 7.5, true_peak_dbtp: -1.2, ..Default::default() });
    rt(&Ebur128::default());
    rt(&Timecode { start: "00:00:00:00".into(), end: "01:23:45:12".into(), fps: 25.0, drop_frame: true, ..Default::default() });
    rt(&Timecode::default());
    rt(&CedDetection { tag: 0xDEAD_BEEF_0000_1234, confidence: 0.66, ..Default::default() });
    rt(&CedDetection::default());
    rt(&Ced {
        tags: vec![
            CedDetection { tag: 1, confidence: 0.5, ..Default::default() },
            CedDetection { tag: 0xFFFF_FFFF_FFFF_FFFF, confidence: 0.9, ..Default::default() },
        ],
        ..Default::default()
    });
    rt(&Ced::default());
}

#[test]
#[cfg(feature = "json")]
fn batch2_sp2_json_roundtrip() {
    use mediaschema::{Ced, CedDetection};
    let c = Ced {
        tags: vec![
            CedDetection { tag: 0xABCD_0000_0000_0001, confidence: 0.42, ..Default::default() },
            CedDetection { tag: 7, confidence: 0.99, ..Default::default() },
        ],
        ..Default::default()
    };
    let json = serde_json::to_string(&c).expect("to_json");
    let back: Ced = serde_json::from_str(&json).expect("from_json");
    assert_eq!(c, back);
}
```

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch2 audio scalar leaves (TagConfidence/SoundSource/AudioEvent/SpeakerSegment/AudioTranscriptSegment/AudioChannelSpec/Chromaprint/Ebur128/Timecode/CedDetection/Ced)`.

---

## Task 3: Batch 3 — composite audio leaves + reuse-only wrappers

`AudioChannelLayout` (needs batch-1 enums + batch-2 `AudioChannelSpec`), `Clap` (5 clap newtype wrappers collapsed → `media.v1.Detection`), `TrackTag` (reuses `media.v1.Detection`).

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto**

```protobuf
// ── Batch 3: composite audio leaves + reuse-only wrappers (§6.5/§6.2) ────────

message AudioChannelLayout {
  AudioChannelOrderKind order = 1;
  uint32 channels = 2;
  ChannelLayoutKind known_kind = 3;
  optional uint64 native_mask = 4;
  repeated AudioChannelSpec custom_channels = 5;
  string description = 6;
}

// clap.rs's 5 transparent single-Detection newtypes
// (AudioDetection/AudioSceneDetection/AudioMoodDetection/VoiceDetection/
// SoundEvent) are NOT emitted — each slot references media.v1.Detection
// directly (§6.2).
message Clap {
  optional media.v1.Detection audio_detection = 1;
  optional media.v1.Detection scene = 2;
  optional media.v1.Detection mood = 3;
  optional media.v1.Detection voice = 4;
  repeated media.v1.Detection sound_events = 5;
}

message TrackTag {
  string category = 1;
  repeated media.v1.Detection detections = 2;
  string source = 3;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `AudioChannelLayout, Clap, TrackTag`.
- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct AudioChannelLayout\|pub struct Clap\|pub struct TrackTag\|pub .*:\|MessageField\|EnumValue" src/generated/findit.db.v1.database.rs`. Add the 3 idents to the `mediaschema::{…}` import (plus `Detection` from SP1). Add `batch3_sp2_roundtrip`:
  - `AudioChannelLayout`: `order: EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_NATIVE)`, `known_kind: EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH5_1)`, `native_mask: Some(0x3F)`, `custom_channels: vec![AudioChannelSpec{..}]`; a 2nd case with `native_mask: None` and `order: UNSPECIFIED`/`known_kind: UNSPECIFIED`; + default.
  - `Clap`: all four optional `media.v1.Detection` set (`MessageField::some(Detection{..})`) + `sound_events: vec![Detection{..}, Detection{..}]`; a 2nd case with all optionals `MessageField::none()` and empty `sound_events`; + default.
  - `TrackTag`: `detections: vec![Detection{..}]`; + default.
  - `#[cfg(feature="json")]` `batch3_sp2_json_roundtrip` for one populated `Clap` (covers `optional media.v1.Detection` + repeated cross-package under serde).

```rust
// ── SP2 Batch 3: composite audio leaves + reuse-only wrappers ────────────────

#[test]
fn batch3_sp2_roundtrip() {
    use mediaschema::{
        AudioChannelLayout, AudioChannelOrderKind, AudioChannelSpec, ChannelLayoutKind,
        Clap, Detection, TrackTag,
    };
    let det = |l: &str, c: f32| buffa::MessageField::some(Detection { label: l.into(), confidence: c, ..Default::default() });

    rt(&AudioChannelLayout {
        order: buffa::EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_NATIVE),
        channels: 6,
        known_kind: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_CH5_1),
        native_mask: Some(0x3F),
        custom_channels: vec![AudioChannelSpec { index: 0, raw_id: 1, label: "FL".into(), ..Default::default() }],
        description: "5.1".into(),
        ..Default::default()
    });
    rt(&AudioChannelLayout {
        order: buffa::EnumValue::from(AudioChannelOrderKind::AUDIO_CHANNEL_ORDER_KIND_UNSPECIFIED),
        channels: 2,
        known_kind: buffa::EnumValue::from(ChannelLayoutKind::CHANNEL_LAYOUT_KIND_UNSPECIFIED),
        native_mask: None,
        custom_channels: vec![],
        description: String::new(),
        ..Default::default()
    });
    rt(&AudioChannelLayout::default());

    rt(&Clap {
        audio_detection: det("music", 0.9),
        scene: det("concert", 0.8),
        mood: det("energetic", 0.7),
        voice: det("singing", 0.6),
        sound_events: vec![
            Detection { label: "applause".into(), confidence: 0.5, ..Default::default() },
            Detection { label: "cheer".into(), confidence: 0.55, ..Default::default() },
        ],
        ..Default::default()
    });
    rt(&Clap {
        audio_detection: buffa::MessageField::none(),
        scene: buffa::MessageField::none(),
        mood: buffa::MessageField::none(),
        voice: buffa::MessageField::none(),
        sound_events: vec![],
        ..Default::default()
    });
    rt(&Clap::default());

    rt(&TrackTag {
        category: "ambience".into(),
        detections: vec![Detection { label: "wind".into(), confidence: 0.6, ..Default::default() }],
        source: "panns".into(),
        ..Default::default()
    });
    rt(&TrackTag::default());
}

#[test]
#[cfg(feature = "json")]
fn batch3_sp2_json_roundtrip() {
    use mediaschema::{Clap, Detection};
    let c = Clap {
        audio_detection: buffa::MessageField::some(Detection { label: "speech".into(), confidence: 0.88, ..Default::default() }),
        scene: buffa::MessageField::none(),
        mood: buffa::MessageField::some(Detection { label: "calm".into(), confidence: 0.7, ..Default::default() }),
        voice: buffa::MessageField::none(),
        sound_events: vec![Detection { label: "door".into(), confidence: 0.4, ..Default::default() }],
        ..Default::default()
    };
    let json = serde_json::to_string(&c).expect("to_json");
    let back: Clap = serde_json::from_str(&json).expect("from_json");
    assert_eq!(c, back);
}
```

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch3 composite audio leaves + reuse-only wrappers (AudioChannelLayout/Clap/TrackTag)`.

---

## Task 4: Batch 4 — non-audio meta blocks

`VideoMeta, VideoTrackMeta, VideoStreamMeta, MediaMeta, SceneMeta, SubtitleMeta, SubtitleTrackMeta, SubtitleTrackOrigin, FailedFile`.

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`reserved` per §6.8 #3; `SubtitleTrackOrigin` = message+oneof per §6.2; `Id`/checksum inline `bytes` per §6.1; bare `Timebase` → `mediatime.v1.Timebase`, `time` → `media.v1.TrackTime` per §6.8 #5)

```protobuf
// ── Batch 4: non-audio meta blocks (§6.5 non-audio table) ───────────────────

message VideoMeta {
  bytes id = 1;
  reserved 2;
  string name = 3;
  media.v1.VideoFormat format = 4;
  media.v1.Dimensions dimensions = 5;
  uint64 size = 6;
  media.v1.TrackTime time = 7;
  double frame_rate = 8;
  uint64 bit_rate = 9;
  int64 created_at = 10;
}

message VideoTrackMeta {
  bytes id = 1;
  uint32 ordinal = 2;
  uint32 stream_index = 3;
  optional uint64 container_track_id = 4;
  media.v1.TrackTime time = 5;
}

message VideoStreamMeta {
  media.v1.CodecId codec_id = 1;
  media.v1.Dimensions dimensions = 2;
  int64 total_pts = 3;
  double frame_rate = 4;
  uint64 bit_rate = 5;
  mediatime.v1.Timebase time_base = 6;
}

message MediaMeta {
  bytes id = 1;
  bytes checksum = 2;
  string name = 3;
  uint64 size = 4;
  media.v1.TrackTime time = 5;
  int64 created_at = 6;
}

message SceneMeta {
  bytes id = 1;
  bytes video_id = 2;
  mediatime.v1.TimeRange range = 3;
  int64 created_at = 4;
  bytes video_track_id = 5;
}

message SubtitleMeta {
  bytes id = 1;
  int64 created_at = 2;
}

message SubtitleTrackMeta {
  bytes id = 1;
  uint32 ordinal = 2;
  optional uint32 stream_index = 3;
  optional uint64 container_track_id = 4;
  media.v1.TrackTime time = 5;
}

// Rust enum-with-data -> message + oneof (§6.2). `kind` mirrors the src
// discriminant: 0=Unspecified, 1=Embedded, 2=Sidecar,
// 3=GeneratedWhisper(source_audio_track_id),
// 4=GeneratedOcr(source_subtitle_track_id). The two id payloads are the
// oneof arms (tags 2/3 mirror the hand-rolled wire layout).
message SubtitleTrackOrigin {
  uint32 kind = 1;
  oneof source {
    bytes source_audio_track_id = 2;
    bytes source_subtitle_track_id = 3;
  }
}

message FailedFile {
  bytes id = 1;
  bytes media_id = 2;
  bytes location_id = 3;
  int64 failed_at = 4;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`. Confirm `findit.db.v1.database.__oneof.rs` now appears (the `SubtitleTrackOrigin.source` oneof) and the `findit.db.v1.mod.rs` stitcher now `include!`s it under `__buffa::oneof`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `FailedFile, MediaMeta, SceneMeta, SubtitleMeta, SubtitleTrackMeta, SubtitleTrackOrigin, VideoMeta, VideoStreamMeta, VideoTrackMeta`. Then add the oneof companion alias immediately after the brace block (mirroring SP1's `LocationKind`):

```rust
/// Oneof variant for [`SubtitleTrackOrigin`]: `Source::SourceAudioTrackId(…)`
/// or `Source::SourceSubtitleTrackId(…)` (proto `oneof source`).
pub use generated::findit::db::v1::subtitle_track_origin::Source as SubtitleTrackOriginSource;
```

(Confirm the generated submodule path `subtitle_track_origin` and oneof enum name `Source` from `grep -n "pub mod subtitle_track_origin\|pub enum Source" src/generated/findit.db.v1.database*.rs` before finalizing the `as` target.)

- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct VideoMeta\|pub struct VideoStreamMeta\|pub struct SubtitleTrackOrigin\|pub mod subtitle_track_origin\|pub .*:\|MessageField\|EnumValue" src/generated/findit.db.v1.database*.rs`. Add the 9 idents + `SubtitleTrackOriginSource` to the `mediaschema::{…}` import (plus SP1 `CodecId, Dimensions, TrackTime, VideoFormat`). Add `batch4_sp2_roundtrip`:
  - `VideoMeta`: nested `media.v1.Dimensions` + `media.v1.TrackTime` (build `TrackTime` with one `mediatime.v1.TimeRange` arm via the SP1 re-export), `format: EnumValue::from(VideoFormat::VIDEO_FORMAT_MP4)`; + default. (Proves `reserved 2` is transparent and the `media.v1` refs round-trip.)
  - `VideoTrackMeta`/`SubtitleTrackMeta` with `container_track_id: Some(...)` and a `None` case.
  - `VideoStreamMeta` with nested `media.v1.CodecId` + `mediatime.v1.Timebase` (extern) — this is the SP2 bare-`Timebase` extern round-trip.
  - `MediaMeta` with non-empty `checksum` bytes.
  - `SceneMeta` with `mediatime.v1.TimeRange` (extern).
  - `SubtitleTrackOrigin`: case A `kind: 1` no oneof arm; case B `kind: 3, source: Some(SubtitleTrackOriginSource::SourceAudioTrackId(vec![..]))`; case C `kind: 4, source: Some(SubtitleTrackOriginSource::SourceSubtitleTrackId(vec![..]))`; + default.
  - `SubtitleMeta`/`FailedFile` populated + default.
  - `#[cfg(feature="json")]` `batch4_sp2_json_roundtrip` for a populated `VideoMeta` (covers cross-package message+enum+TrackTime under serde) and a `SubtitleTrackOrigin` case C (covers the oneof under serde).

```rust
// ── SP2 Batch 4: non-audio meta blocks ──────────────────────────────────────

fn sp2_track_time_one() -> buffa::MessageField<mediaschema::TrackTime> {
    use mediaschema::TrackTime;
    buffa::MessageField::some(TrackTime {
        declared: buffa::MessageField::some(mediatime::TimeRange::new(
            0, 1000, mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
        )),
        packet_observed: buffa::MessageField::none(),
        decoded_observed: buffa::MessageField::none(),
        ..Default::default()
    })
}

#[test]
fn batch4_sp2_roundtrip() {
    use mediaschema::{
        CodecId, Dimensions, FailedFile, MediaMeta, SceneMeta, SubtitleMeta,
        SubtitleTrackMeta, SubtitleTrackOrigin, SubtitleTrackOriginSource, VideoFormat,
        VideoMeta, VideoStreamMeta, VideoTrackMeta,
    };
    rt(&VideoMeta {
        id: vec![1, 2, 3, 4],
        name: "clip.mp4".into(),
        format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MP4),
        dimensions: buffa::MessageField::some(Dimensions { width: 1920, height: 1080, ..Default::default() }),
        size: 123456,
        time: sp2_track_time_one(),
        frame_rate: 23.976,
        bit_rate: 8_000_000,
        created_at: 1_700_000_000,
        ..Default::default()
    });
    rt(&VideoMeta::default());

    rt(&VideoTrackMeta { id: vec![9], ordinal: 0, stream_index: 1, container_track_id: Some(42), time: sp2_track_time_one(), ..Default::default() });
    rt(&VideoTrackMeta { id: vec![9], ordinal: 1, stream_index: 2, container_track_id: None, time: buffa::MessageField::none(), ..Default::default() });
    rt(&VideoTrackMeta::default());

    rt(&VideoStreamMeta {
        codec_id: buffa::MessageField::some(CodecId { value: 27, ..Default::default() }),
        dimensions: buffa::MessageField::some(Dimensions { width: 3840, height: 2160, ..Default::default() }),
        total_pts: 90_000,
        frame_rate: 60.0,
        bit_rate: 20_000_000,
        time_base: buffa::MessageField::some(mediatime::Timebase::new(1, core::num::NonZeroU32::new(30000).unwrap())),
        ..Default::default()
    });
    rt(&VideoStreamMeta::default());

    rt(&MediaMeta { id: vec![1], checksum: (1u8..=32).collect(), name: "a".into(), size: 10, time: sp2_track_time_one(), created_at: 1, ..Default::default() });
    rt(&MediaMeta::default());

    rt(&SceneMeta {
        id: vec![1], video_id: vec![2],
        range: buffa::MessageField::some(mediatime::TimeRange::new(
            100, 500, mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
        )),
        created_at: 5, video_track_id: vec![3],
        ..Default::default()
    });
    rt(&SceneMeta::default());

    rt(&SubtitleMeta { id: vec![7], created_at: 9, ..Default::default() });
    rt(&SubtitleMeta::default());

    rt(&SubtitleTrackMeta { id: vec![1], ordinal: 0, stream_index: Some(3), container_track_id: Some(8), time: sp2_track_time_one(), ..Default::default() });
    rt(&SubtitleTrackMeta { id: vec![1], ordinal: 2, stream_index: None, container_track_id: None, time: buffa::MessageField::none(), ..Default::default() });
    rt(&SubtitleTrackMeta::default());

    rt(&SubtitleTrackOrigin { kind: 1, source: None, ..Default::default() });
    rt(&SubtitleTrackOrigin { kind: 3, source: Some(SubtitleTrackOriginSource::SourceAudioTrackId(vec![0xAA, 0xBB])), ..Default::default() });
    rt(&SubtitleTrackOrigin { kind: 4, source: Some(SubtitleTrackOriginSource::SourceSubtitleTrackId(vec![0xCC, 0xDD])), ..Default::default() });
    rt(&SubtitleTrackOrigin::default());

    rt(&FailedFile { id: vec![1], media_id: vec![2], location_id: vec![3], failed_at: 42, ..Default::default() });
    rt(&FailedFile::default());
}

#[test]
#[cfg(feature = "json")]
fn batch4_sp2_json_roundtrip() {
    use mediaschema::{Dimensions, SubtitleTrackOrigin, SubtitleTrackOriginSource, VideoFormat, VideoMeta};
    let vm = VideoMeta {
        id: vec![1, 2],
        name: "j.mp4".into(),
        format: buffa::EnumValue::from(VideoFormat::VIDEO_FORMAT_MKV),
        dimensions: buffa::MessageField::some(Dimensions { width: 1280, height: 720, ..Default::default() }),
        size: 99,
        time: sp2_track_time_one(),
        frame_rate: 25.0,
        bit_rate: 5_000_000,
        created_at: 7,
        ..Default::default()
    };
    let json = serde_json::to_string(&vm).expect("to_json");
    let back: VideoMeta = serde_json::from_str(&json).expect("from_json");
    assert_eq!(vm, back);

    let o = SubtitleTrackOrigin { kind: 4, source: Some(SubtitleTrackOriginSource::SourceSubtitleTrackId(vec![1, 2, 3])), ..Default::default() };
    let oj = serde_json::to_string(&o).expect("to_json");
    let ob: SubtitleTrackOrigin = serde_json::from_str(&oj).expect("from_json");
    assert_eq!(o, ob);
}
```

(If the oneof arm payload is boxed/`Bytes`-typed, adjust `SourceAudioTrackId(...)` per the real generated signature found by the grep.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch4 non-audio meta blocks (VideoMeta/VideoTrackMeta/VideoStreamMeta/MediaMeta/SceneMeta/SubtitleMeta/SubtitleTrackMeta/SubtitleTrackOrigin/FailedFile)`.

---

## Task 5: Batch 5 — non-audio track/record wrappers

`Video, VideoTrack, Media, Subtitle, SubtitleTrack, SubtitleCue`.

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`reserved` per §6.8 #3; bitflags inline `uint32` with documented bits per §6.4; `index_error` = `optional media.v1.ErrorInfo`; `findit.db.v1.MediaKind` is the batch-1 db enum)

```protobuf
// ── Batch 5: non-audio track/record wrappers (§6.5 non-audio table) ─────────

// `index_status` bits (VideoIndexStatus, §6.4): PROBED=0x01,
// SCENE_DETECTED=0x02, KEYFRAME_EXTRACTED=0x04, VLM_ANALYZED=0x10,
// APPLE_VISION_ANALYZED=0x20, TEXT_EMBEDDING_FINISHED=0x40,
// SCENE_EMBEDDING_FINISHED=0x80 (src gap at 0x08).
message Video {
  VideoMeta meta = 1;
  repeated bytes scenes = 2;
  reserved 3 to 7;
  uint32 index_status = 8;
  optional media.v1.ErrorInfo index_error = 9;
  reserved 10;
  uint32 error_status = 11;
}

// `disposition` bits (VideoTrackDisposition, §6.4): DEFAULT=0x1, DUB=0x2,
// ORIGINAL=0x4, COMMENT=0x8, LYRICS=0x10, KARAOKE=0x20, FORCED=0x40,
// HEARING_IMPAIRED=0x80, VISUAL_IMPAIRED=0x100, CLEAN_EFFECTS=0x200,
// ATTACHED_PIC=0x400, TIMED_THUMBNAILS=0x800, NON_DIEGETIC=0x1000,
// CAPTIONS=0x10000, DESCRIPTIONS=0x20000, METADATA=0x40000,
// DEPENDENT=0x80000, STILL_IMAGE=0x100000, MULTILAYER=0x200000.
message VideoTrack {
  VideoTrackMeta meta = 1;
  VideoStreamMeta stream = 2;
  uint32 disposition = 3;
  bool is_primary = 4;
  bool auto_selected = 5;
  string selection_reason = 6;
  bytes video_id = 7;
  optional media.v1.ErrorInfo index_error = 8;
}

// `index_status` bits (MediaIndexStatus, §6.4): PROBED=0x01,
// VIDEO_INDEXED=0x02, AUDIO_INDEXED=0x04, SUBTITLE_INDEXED=0x08.
message Media {
  MediaMeta meta = 1;
  MediaKind kind = 2;
  uint32 index_status = 3;
  optional media.v1.ErrorInfo index_error = 4;
  optional bytes video_id = 5;
  optional bytes audio_id = 6;
  optional bytes subtitle_id = 7;
  uint32 error_status = 8;
  int64 capture_date = 9;
  string device_make = 10;
  string device_model = 11;
  string gps_location = 12;
}

// `index_status` bits (SubtitleIndexStatus, §6.4): TRACKS_DISCOVERED=0x01,
// CUES_EXTRACTED=0x02, OCR_DONE=0x04, SEARCH_INDEXED=0x08.
message Subtitle {
  SubtitleMeta meta = 1;
  reserved 2;
  uint32 index_status = 3;
  optional media.v1.ErrorInfo index_error = 4;
}

// `disposition` bits = SubtitleTrackDisposition (identical layout to
// VideoTrackDisposition above, §6.4).
message SubtitleTrack {
  SubtitleTrackMeta meta = 1;
  bytes subtitle_id = 2;
  SubtitleTrackOrigin origin = 3;
  SubtitleTrackFormat format = 4;
  SubtitleTrackRole role = 5;
  string language = 6;
  string title = 7;
  media.v1.CodecId codec_id = 8;
  uint32 disposition = 9;
  bool is_primary = 10;
  bool auto_selected = 11;
  string selection_reason = 12;
  optional media.v1.ErrorInfo index_error = 13;
}

message SubtitleCue {
  bytes id = 1;
  bytes subtitle_track_id = 2;
  mediatime.v1.TimeRange range = 3;
  string text = 4;
  string language = 5;
  optional float confidence = 6;
  string raw_payload = 7;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `Media, Subtitle, SubtitleCue, SubtitleTrack, Video, VideoTrack`.
- [ ] **Step 4: Tests** — confirm fields: `grep -n "pub struct Video\b\|pub struct SubtitleTrack\b\|pub struct Media\b\|pub .*:\|MessageField\|EnumValue" src/generated/findit.db.v1.database.rs`. Add the 6 idents to the `mediaschema::{…}` import (plus `DbMediaKind, ErrorInfo, CodecId, SubtitleTrackFormat, SubtitleTrackRole, SubtitleTrackOrigin, SubtitleTrackOriginSource`). Add `batch5_sp2_roundtrip`:
  - `Video`: nested `VideoMeta` (reuse a builder), `scenes: vec![vec![1],vec![2]]`, `index_status: 0x01|0x02|0x80`, `index_error: MessageField::some(ErrorInfo{..})`, `error_status: 1`; a 2nd case with `index_error: none()` and empty `scenes`; + default. (Proves `reserved 3..7,10` transparent.)
  - `VideoTrack`: nested `VideoTrackMeta`+`VideoStreamMeta`, `disposition: 0x1|0x40`, booleans, `video_id` bytes.
  - `Media`: `kind: EnumValue::from(DbMediaKind::MEDIA_KIND_VIDEO)`, `video_id: Some(vec![1])`/`audio_id: None`/`subtitle_id: None`; a 2nd case `kind: MEDIA_KIND_UNSPECIFIED` all optionals `None`; + default.
  - `Subtitle` populated + default.
  - `SubtitleTrack`: nested `SubtitleTrackMeta`+`SubtitleTrackOrigin` (`kind:2`)+`media.v1.CodecId`, `format: EnumValue::from(SubtitleTrackFormat::SUBTITLE_TRACK_FORMAT_SRT)`, `role: EnumValue::from(SubtitleTrackRole::SUBTITLE_TRACK_ROLE_CAPTION)`, `disposition: 0x1`; + default.
  - `SubtitleCue`: `range: mediatime.v1.TimeRange` (extern), `confidence: Some(0.9)` and a `None` case; + default.
  - `#[cfg(feature="json")]` `batch5_sp2_json_roundtrip` for a populated `SubtitleTrack` (covers nested oneof message + enums + cross-package `CodecId` under serde).

(Construct nested `VideoMeta`/`VideoTrackMeta`/`VideoStreamMeta`/`SubtitleTrackMeta` via small local builders reusing the batch-4 `sp2_track_time_one()` helper. Test bodies follow the exact same `rt(&...) + rt(&Default::default())` shape as batch 4; fully expand each per the confirmed generated field names.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch5 non-audio track/record wrappers (Video/VideoTrack/Media/Subtitle/SubtitleTrack/SubtitleCue)`.

---

## Task 6: Batch 6 — Scene + VLM

`Scene, SceneVlmResult`.

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`Scene` `reserved 10,12` per §6.8 #3; `SceneVlmResult` is the pure-Rust struct — tags 1..11 in struct-declaration order, a clean-redesign choice per §6.8 #2; all detector fields reference `media.v1.*`)

```protobuf
// ── Batch 6: Scene + VLM (§6.5 non-audio table; §6.8 #2 for SceneVlmResult) ──

message Scene {
  SceneMeta meta = 1;
  repeated bytes keyframes = 2;
  string description = 3;
  string shot_type = 4;
  string camera_motion = 5;
  string tags = 6;
  uint32 people_count = 7;
  repeated bytes tag_ids = 8;
  repeated string vision_provider = 9;
  reserved 10;
  repeated string smart_folders = 11;
  reserved 12;
}

// Pure-Rust struct (scene_vlm.rs, no hand-rolled Encode); field tags
// 1..11 assigned in struct-declaration order — a clean-redesign choice
// (§2 grants this; no wire-compat to honor). §6.8 #2.
message SceneVlmResult {
  optional string scene = 1;
  optional string description = 2;
  repeated media.v1.SubjectDetection subjects = 3;
  repeated media.v1.ObjectDetection objects = 4;
  repeated media.v1.ActionDetection actions = 5;
  repeated media.v1.MoodDetection mood = 6;
  optional string shot_type = 7;
  repeated media.v1.LightingDetection lighting = 8;
  repeated media.v1.ColorDetection colors = 9;
  repeated string tags = 10;
  repeated media.v1.ClassificationDetection classifications = 11;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `Scene, SceneVlmResult`.
- [ ] **Step 4: Tests** — confirm fields: `grep -n "pub struct Scene\b\|pub struct SceneVlmResult\|pub .*:\|MessageField" src/generated/findit.db.v1.database.rs`. Add the 2 idents to the `mediaschema::{…}` import (plus SP1 `SubjectDetection, ObjectDetection, ActionDetection, MoodDetection, LightingDetection, ColorDetection, ClassificationDetection, Detection, BoundingBox`). Add `batch6_sp2_roundtrip`:
  - `Scene`: nested `SceneMeta`, `keyframes: vec![vec![1],vec![2]]`, `tag_ids: vec![vec![9]]`, `vision_provider: vec!["apple".into()]`, `smart_folders: vec!["fav".into()]`, `people_count: 3`; + default. (Proves `reserved 10,12` transparent.)
  - `SceneVlmResult`: `scene: Some("beach".into())`, `subjects: vec![media.v1.SubjectDetection{..}]`, `objects`/`actions`/`mood`/`lighting`/`colors`/`classifications` each with ≥1 nested `media.v1.*` detector, `tags: vec!["sunset".into()]`, `shot_type: Some("wide".into())`; a 2nd all-`None`/empty case; + default.
  - `#[cfg(feature="json")]` `batch6_sp2_json_roundtrip` for a populated `SceneVlmResult` (covers many cross-package `media.v1.*` detectors under serde).

(Reuse the SP1 `make_subject`/`make_bbox`/`make_detection` helpers already in `tests/roundtrip.rs` to build the nested `media.v1` detectors. Fully expand both `rt` cases per the confirmed generated field names; `media.v1.ClassificationDetection`/etc. construct as `ClassificationDetection { detection: buffa::MessageField::some(Detection{..}), ..Default::default() }`.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch6 Scene + SceneVlmResult`.

---

## Task 7: Batch 7 — Keyframe (own batch — large, reuse-heavy)

`Keyframe` (composes `media.v1.{Dimensions,HumanAnalysis,AnimalAnalysis,HorizonInfo,FeaturePrint,Aesthetics}` + 16 `media.v1.*` detector types; uses `media.v1.SaliencyRegion` twice).

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (every detector/aggregate field is a `media.v1.*` reference — NEVER redefined; `Id`/`scene_id` inline `bytes`; `Dimensions` referenced from `media.v1` per §6.8 #7)

```protobuf
// ── Batch 7: Keyframe (§6.5 non-audio table) ────────────────────────────────

message Keyframe {
  bytes id = 1;
  bytes scene_id = 2;
  int64 pts = 3;
  media.v1.Dimensions dimensions = 4;
  bytes data = 5;
  repeated media.v1.ClassificationDetection classifications = 6;
  media.v1.HumanAnalysis humans = 7;
  media.v1.AnimalAnalysis animals = 8;
  repeated media.v1.ObjectDetection objects = 9;
  repeated media.v1.ActionDetection actions = 10;
  repeated media.v1.MoodDetection mood = 11;
  repeated media.v1.EmotionDetection emotion = 12;
  repeated media.v1.LightingDetection lighting = 13;
  repeated media.v1.ColorDetection colors = 14;
  repeated media.v1.TextDetection text_detections = 15;
  repeated media.v1.BarcodeDetection barcodes = 16;
  repeated media.v1.SaliencyRegion attention_saliency = 17;
  repeated media.v1.SaliencyRegion objectness_saliency = 18;
  media.v1.HorizonInfo horizon = 19;
  repeated media.v1.DocumentSegment document_segments = 20;
  media.v1.FeaturePrint feature_print = 21;
  media.v1.Aesthetics aesthetics = 22;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `Keyframe`.
- [ ] **Step 4: Tests** — confirm fields: `grep -n "pub struct Keyframe\|pub .*:\|MessageField" src/generated/findit.db.v1.database.rs`. Add `Keyframe` to the `mediaschema::{…}` import (plus SP1 `Dimensions, HumanAnalysis, AnimalAnalysis, HorizonInfo, FeaturePrint, Aesthetics, ClassificationDetection, ObjectDetection, ActionDetection, MoodDetection, EmotionDetection, LightingDetection, ColorDetection, TextDetection, BarcodeDetection, SaliencyRegion, Detection, BoundingBox`). Add `batch7_sp2_roundtrip`:
  - A fully-populated `Keyframe`: `dimensions: MessageField::some(Dimensions{1920,1080})`, `humans: MessageField::some(HumanAnalysis{ subjects: vec![make_subject(..)] , .. })`, `animals: MessageField::some(AnimalAnalysis{..})`, ≥1 of each repeated detector (`ClassificationDetection`/`ObjectDetection`/`ActionDetection`/`MoodDetection`/`EmotionDetection`/`LightingDetection`/`ColorDetection`/`TextDetection`/`BarcodeDetection`), `attention_saliency` AND `objectness_saliency` each `vec![SaliencyRegion{ bbox: make_bbox(..), confidence: .. }]` (proves the same `media.v1` type used in two fields), `horizon: MessageField::some(HorizonInfo{..})`, `document_segments: vec![DocumentSegment{..}]`, `feature_print: MessageField::some(FeaturePrint{..})`, `aesthetics: MessageField::some(Aesthetics{..})`, `data: vec![0xAB; 64]`, `pts: 12345`; + `Keyframe::default()`.
  - `#[cfg(feature="json")]` `batch7_sp2_json_roundtrip` for the populated `Keyframe` (the heaviest cross-package serde case — 16 detector types + 3 aggregates).

(Reuse the SP1 `make_subject`/`make_body_pose`/`make_bbox`/`make_detection` helpers. Build `HumanAnalysis`/`AnimalAnalysis` exactly as the SP1 `batch6_roundtrip` does. Fully expand per confirmed field names.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch7 Keyframe (reuse-heavy: 16 media.v1 detectors + 3 aggregates)`.

---

## Task 8: Batch 8 — audio meta + summary blocks

`AudioMeta, AudioStreamMeta, AudioTrackMeta, AudioSummary`.

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`AudioMeta reserved 2` per §6.8 #3; `time` → `media.v1.TrackTime`; `AudioStreamMeta.codec_id` → `media.v1.CodecId`; `AudioSummary` reuses batch-1 `AudioPrefilterClass` + batch-2 `TagConfidence`)

```protobuf
// ── Batch 8: audio meta + summary blocks (§6.5 audio aggregates table) ──────

message AudioMeta {
  bytes id = 1;
  reserved 2;
  string name = 3;
  string container = 4;
  uint64 size = 5;
  media.v1.TrackTime time = 6;
  int64 created_at = 7;
}

message AudioStreamMeta {
  media.v1.CodecId codec_id = 1;
  uint32 sample_rate = 2;
  AudioChannelLayout layout = 3;
  uint64 bit_rate = 4;
  string language = 5;
  string stream_title = 6;
  string album = 7;
  string artist = 8;
  string title = 9;
  string genre = 10;
  uint32 track_number = 11;
  string sample_format = 12;
  uint32 bits_per_sample = 13;
}

message AudioTrackMeta {
  bytes id = 1;
  uint32 ordinal = 2;
  uint32 stream_index = 3;
  optional uint64 container_track_id = 4;
  media.v1.TrackTime time = 5;
}

message AudioSummary {
  AudioPrefilterClass prefilter_class = 1;
  optional TagConfidence audio_type = 2;
  optional TagConfidence scene = 3;
  optional TagConfidence mood = 4;
  optional TagConfidence voice = 5;
  bool has_speech = 6;
  bool has_music = 7;
  string dominant_language = 8;
  float speech_ratio = 9;
  uint32 speaker_count = 10;
  float loudness_lufs = 11;
  float rms_db = 12;
  string transcript_preview = 13;
  uint32 clip_count = 14;
  repeated uint32 fingerprint = 15;
  bool gemini_enhanced = 16;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `AudioMeta, AudioStreamMeta, AudioSummary, AudioTrackMeta`.
- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct AudioMeta\|pub struct AudioStreamMeta\|pub struct AudioSummary\|pub .*:\|MessageField\|EnumValue" src/generated/findit.db.v1.database.rs`. Add the 4 idents to the `mediaschema::{…}` import (plus batch-1 `AudioPrefilterClass`, batch-2 `TagConfidence`, batch-3 `AudioChannelLayout`, SP1 `CodecId`). Add `batch8_sp2_roundtrip`:
  - `AudioMeta`: nested `media.v1.TrackTime` (reuse `sp2_track_time_one()`), `container: "mka".into()`; + default. (Proves `reserved 2` transparent.)
  - `AudioStreamMeta`: nested `media.v1.CodecId` + batch-3 `AudioChannelLayout`, all string/int fields set; + default.
  - `AudioTrackMeta`: `container_track_id: Some(...)` and a `None` case; + default.
  - `AudioSummary`: `prefilter_class: EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_CONTENT)`, all four optional `TagConfidence` set; a 2nd case all `None` + `prefilter_class: UNSPECIFIED`, `fingerprint: vec![]`; + default.
  - `#[cfg(feature="json")]` `batch8_sp2_json_roundtrip` for a populated `AudioSummary` (covers optional nested `TagConfidence` + enum + repeated under serde).

(Reuse the batch-4 `sp2_track_time_one()` and a small `AudioChannelLayout` builder. Fully expand per confirmed field names.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch8 audio meta + summary blocks (AudioMeta/AudioStreamMeta/AudioTrackMeta/AudioSummary)`.

---

## Task 9: Batch 9 — audio aggregates (`AudioAnalysis` + `TrackRecord` are large)

`AudioAnalysis` (≈40 fields, wide deliberate tag bands), `TrackRecord` (banded 1000s/2000s tags, composes `Timecode/Ebur128/Clap/Ced/Chromaprint/TrackTag`), `Audio`, `AudioTrack`.

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`AudioAnalysis` sparse field numbers verbatim, NO `reserved` for the wide bands per §6.8 #3; `Audio reserved 3,4` / `AudioTrack reserved 2` / `TrackRecord reserved 17` per §6.8 #3; `language` = `Iso6392B` inlined as `uint32` per §6.2/§6.8 #4; bare `Timebase` → `mediatime.v1.Timebase`; `index_error` = `optional media.v1.ErrorInfo`; `findit.db.v1` enums from batch 1; `Timecode/Ebur128/Clap/Ced/Chromaprint/TrackTag` from batches 2/3)

```protobuf
// ── Batch 9: audio aggregates (§6.5 audio aggregates table) ─────────────────

// Source has wide deliberate tag bands (gaps 8–9, 11–19, 25–29, 38–39,
// 43–49, 53–59, 62–69, 73–79, 83–89, 96–99, 102–109). Per §6.8 #3 these
// are semantic groupings, NOT accidental holes: proto3 sparse field
// numbers are emitted verbatim with NO `reserved` (no wire-compat
// constraint; reserving dozens of numbers adds noise with no value).
message AudioAnalysis {
  bytes id = 1;
  bytes audio_id = 2;
  optional bytes scene_id = 3;
  AudioClipKind kind = 4;
  uint32 start_ms = 5;
  uint32 end_ms = 6;
  bytes track_id = 7;
  repeated CedDetection ced_tags = 10;
  optional TagConfidence zs_audio_type = 20;
  optional TagConfidence zs_scene = 21;
  optional TagConfidence zs_mood = 22;
  repeated TagConfidence zs_sound_events = 23;
  optional TagConfidence zs_voice = 24;
  string description_en = 30;
  string description_zh = 31;
  string gemini_scene = 32;
  string gemini_mood = 33;
  repeated SoundSource gemini_sound_sources = 34;
  string gemini_foreground = 35;
  string gemini_background = 36;
  bool gemini_enhanced = 37;
  repeated AudioEvent event_timeline = 40;
  string foreground_layer = 41;
  string background_layer = 42;
  float speech_ratio = 50;
  uint32 speaker_count = 51;
  repeated SpeakerSegment speaker_segments = 52;
  optional TagConfidence voice_gender = 60;
  optional TagConfidence voice_emotion = 61;
  string transcript = 70;
  repeated AudioTranscriptSegment transcript_segments = 71;
  string language = 72;
  optional TagConfidence music_genre = 80;
  optional float music_bpm = 81;
  repeated TagConfidence music_instruments = 82;
  float loudness_lufs = 90;
  float rms_db = 91;
  optional float snr_db = 92;
  bool has_sudden_onset = 93;
  string energy_profile = 94;
  float spectral_flatness = 95;
  repeated uint32 fingerprint = 100;
  AudioPrefilterClass prefilter_class = 101;
  bool has_speech = 110;
  bool has_music = 111;
}

// `language` is the src Iso6392B u32 newtype inlined as uint32 (varint,
// §6.2/§6.8 #4). `stop_reason` bits (StopReason, §6.4): TIMECODE=0x01,
// SILENT=0x02, FLATNESS=0x04, NO_SPEECH=0x08, LLM_NO_NET=0x10,
// LLM_NO_CREDIT=0x20. `index_status` bits (ProcessingStage, §6.4):
// EXTRACTED=0x01, CLASSIFIED=0x02, VAD_DONE=0x04, STT_DONE=0x08,
// SPEAKER_DONE=0x10, LLM_DONE=0x20, TEXT_EMBED=0x40, CED_DONE=0x80,
// CLAP_DONE=0x100, EBUR128_DONE=0x200, FPRINT_DONE=0x400. Source 1000s/
// 2000s bands preserved verbatim; gap at 17 reserved (§6.8 #3).
message TrackRecord {
  bytes id = 1;
  bytes audio_id = 2;
  uint32 track_index = 3;
  AudioCodec codec = 4;
  AudioSampleFormat sample_format = 5;
  uint32 sample_rate = 6;
  uint32 channels = 7;
  ChannelLayoutKind channel_layout = 8;
  uint64 bit_rate = 9;
  uint32 bit_depth = 10;
  int64 total_pts = 11;
  mediatime.v1.Timebase time_base = 12;
  uint32 language = 13;
  optional Timecode timecode = 14;
  TrackClassificationType classification = 15;
  uint32 stop_reason = 16;
  reserved 17;
  repeated TrackTag tags = 18;
  optional Ebur128 ebur_128 = 1000;
  optional Clap clap = 1001;
  optional Ced ced = 1002;
  optional Chromaprint chromaprint = 1003;
  uint32 index_status = 2000;
  optional media.v1.ErrorInfo index_error = 2001;
  uint32 error_status = 2002;
}

// `index_status` bits (AudioIndexStatus, §6.4): PROBED=0x01,
// ANALYZED_LOCAL=0x02, TRANSCRIPTED=0x04, GEMINI_ENHANCED=0x08,
// AUDIO_EMBEDDING_FINISHED=0x10, DESCRIPTION_EMBEDDING_FINISHED=0x20.
message Audio {
  AudioMeta meta = 1;
  repeated bytes analyses = 2;
  reserved 3, 4;
  AudioSummary summary = 5;
  uint32 index_status = 6;
  optional media.v1.ErrorInfo index_error = 7;
}

// `disposition` bits = AudioTrackDisposition (identical layout to
// VideoTrackDisposition, §6.4).
message AudioTrack {
  AudioTrackMeta meta = 1;
  reserved 2;
  AudioStreamMeta stream = 3;
  uint32 disposition = 4;
  AudioTrackRole role = 5;
  bool is_primary = 6;
  bool auto_selected = 7;
  string selection_reason = 8;
  bytes audio_id = 9;
  optional media.v1.ErrorInfo index_error = 10;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `Audio, AudioAnalysis, AudioTrack, TrackRecord`.
- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct AudioAnalysis\|pub struct TrackRecord\|pub struct Audio\b\|pub struct AudioTrack\|pub .*:\|MessageField\|EnumValue" src/generated/findit.db.v1.database.rs`. Add the 4 idents to the `mediaschema::{…}` import (plus batch-1 enums `AudioClipKind, AudioCodec, AudioSampleFormat, ChannelLayoutKind, TrackClassificationType, AudioPrefilterClass, AudioTrackRole`; batch-2 `CedDetection, TagConfidence, SoundSource, AudioEvent, SpeakerSegment, AudioTranscriptSegment, Timecode, Ebur128, Ced, Chromaprint`; batch-3 `Clap, TrackTag`; batch-8 `AudioMeta, AudioStreamMeta, AudioTrackMeta, AudioSummary`; SP1 `ErrorInfo`). Add `batch9_sp2_roundtrip`:
  - `AudioAnalysis`: a wide populated instance touching at least one field per band — `id/audio_id` bytes, `scene_id: Some(vec![5])`, `kind: EnumValue::from(AudioClipKind::AUDIO_CLIP_KIND_FIXED_WINDOW)`, `ced_tags: vec![CedDetection{..}]`, `zs_audio_type: Some(TagConfidence{..})`, `zs_sound_events: vec![TagConfidence{..}]`, `gemini_sound_sources: vec![SoundSource{..}]`, `event_timeline: vec![AudioEvent{..}]`, `speaker_segments: vec![SpeakerSegment{..}]`, `transcript_segments: vec![AudioTranscriptSegment{..}]`, `music_bpm: Some(120.0)`, `snr_db: Some(15.0)`, `fingerprint: vec![1,2,3]`, `prefilter_class: EnumValue::from(AudioPrefilterClass::AUDIO_PREFILTER_CLASS_CONTENT)`, scalars set; a 2nd minimal case (all `optional` `None`, all `repeated` empty, enums `UNSPECIFIED`) — this 2nd case is the **sparse-field-number guard** (proves the wide bands round-trip with holes empty); + `AudioAnalysis::default()`.
  - `TrackRecord`: `time_base: MessageField::some(mediatime::Timebase::new(..))` (extern), `language: 0x656E67` (Iso6392B-as-uint32), `codec/sample_format/channel_layout/classification` enums set, `timecode: Some(Timecode{..})`, `tags: vec![TrackTag{..}]`, the 1000s band all set (`ebur_128/clap/ced/chromaprint` `Some(...)`), `index_status: 0x01|0x100|0x400`, `index_error: MessageField::some(ErrorInfo{..})`, `error_status: 1`; a 2nd case with the 1000s band all `None`, `timecode: None`, `index_error: none()` (proves `reserved 17` + banded tags round-trip); + default.
  - `Audio`: nested `AudioMeta`+`AudioSummary`, `analyses: vec![vec![1],vec![2]]`, `index_status: 0x01|0x20`, `index_error` set; a 2nd case `index_error: none()` empty `analyses`; + default. (Proves `reserved 3,4` transparent.)
  - `AudioTrack`: nested `AudioTrackMeta`+`AudioStreamMeta`, `role: EnumValue::from(AudioTrackRole::AUDIO_TRACK_ROLE_MAIN_PROGRAM)`, `disposition: 0x1`, booleans, `audio_id` bytes; + default. (Proves `reserved 2` transparent.)
  - `#[cfg(feature="json")]` `batch9_sp2_json_roundtrip` for the populated `AudioAnalysis` (the widest serde case — sparse field numbers + many optional nested `TagConfidence`) and the populated `TrackRecord` (banded tags + extern `Timebase` + nested `Clap/Ced` under serde).

(Reuse batch-2/3/8 builders for nested `CedDetection/TagConfidence/Clap/Ced/Chromaprint/Ebur128/Timecode/TrackTag/AudioMeta/AudioStreamMeta/AudioSummary/AudioTrackMeta`. Fully expand both `AudioAnalysis` cases and both `TrackRecord` cases per confirmed field names — do not abbreviate; every field present in the proto block above must appear in the populated case.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch9 audio aggregates (AudioAnalysis/TrackRecord/Audio/AudioTrack)`.

---

## Task 10: Batch 10 — audio file record (own batch)

`AudioCoverArt` then `AudioFileRecord` (composes `AudioCoverArt`; reuses `media.v1.{AudioFormat,Location,Dimensions}` + `mediatime.v1.Timebase`).

**Files:** modify `proto/findit/db/v1/database.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`AudioFileRecord reserved 28..30` per §6.8 #3; bare `Timebase` → `mediatime.v1.Timebase`; `AudioCoverArt.path` → `optional media.v1.Location`, `dimensions` → `optional media.v1.Dimensions`; `checksum` inline `bytes`; `container_format` = batch-1 `AudioContainerFormat`; both messages are owned per §6.8 #1)

```protobuf
// ── Batch 10: audio file record (§6.5 audio aggregates table; §6.8 #1) ──────

message AudioCoverArt {
  optional media.v1.Location path = 1;
  string mime = 2;
  optional media.v1.Dimensions dimensions = 3;
  uint32 size = 4;
}

message AudioFileRecord {
  bytes id = 1;
  optional bytes checksum = 2;
  string name = 3;
  media.v1.AudioFormat format = 4;
  uint64 size = 5;
  int64 total_pts = 6;
  double frame_rate = 7;
  uint64 bit_rate = 8;
  mediatime.v1.Timebase time_base = 9;
  AudioContainerFormat container_format = 10;
  uint32 stream_count = 11;
  string title = 12;
  string artist = 13;
  string album_artist = 14;
  string album = 15;
  string genre = 16;
  string composer = 17;
  string performer = 18;
  string date = 19;
  uint32 track_number = 20;
  uint32 total_tracks = 21;
  uint32 disc_number = 22;
  uint32 total_discs = 23;
  string comment = 24;
  string lyrics = 25;
  repeated string tag_types = 26;
  optional AudioCoverArt cover_art = 27;
  reserved 28 to 30;
  int64 created_at = 31;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `AudioCoverArt, AudioFileRecord`.
- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct AudioCoverArt\|pub struct AudioFileRecord\|pub .*:\|MessageField\|EnumValue" src/generated/findit.db.v1.database.rs`. Add the 2 idents to the `mediaschema::{…}` import (plus SP1 `AudioFormat, Location, LocationKind, Local, Id, Dimensions`; batch-1 `AudioContainerFormat`). Add `batch10_sp2_roundtrip`:
  - `AudioCoverArt`: `path: MessageField::some(make_local_location())` (reuse the SP1 `make_local_location()` helper already in `tests/roundtrip.rs`), `dimensions: MessageField::some(Dimensions{600,600})`, `mime: "image/jpeg".into()`, `size: 40_000`; a 2nd case `path: none()`/`dimensions: none()`; + default.
  - `AudioFileRecord`: a fully-populated instance — `checksum: Some((1u8..=32).collect())`, `format: EnumValue::from(AudioFormat::AUDIO_FORMAT_FLAC)`, `time_base: MessageField::some(mediatime::Timebase::new(..))` (extern), `container_format: EnumValue::from(AudioContainerFormat::AUDIO_CONTAINER_FORMAT_FLAC)`, `cover_art: MessageField::some(AudioCoverArt{..})`, every string/int field set; a 2nd case `checksum: None`/`cover_art: none()` (proves `reserved 28..30` transparent + optionals); + default.
  - `#[cfg(feature="json")]` `batch10_sp2_json_roundtrip` for the populated `AudioFileRecord` (covers nested `AudioCoverArt` → `media.v1.Location` oneof + extern `Timebase` + enums under serde).

(Reuse the SP1 `make_local_location()` helper. Fully expand both cases per confirmed field names.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp2): batch10 audio file record (AudioCoverArt/AudioFileRecord)`.

---

## Task 11: SP2 finalize — CI matrix + DoD + stacked PR

**Files:** possibly `.github/workflows/codegen.yml` (only if needed); branch finish.

- [ ] **Step 1: Full SP2 Definition-of-Done check** (run, all must pass):
```
cd /Users/user/Develop/findit-studio/mediaschema
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo build --no-default-features
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo build --all-features
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo test --features quickcheck,json 2>&1 | grep "test result:"
env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo run -p xtask -- gen && git diff --exit-code -- src/generated && echo GEN_CLEAN
```
All SP0 + SP1 + SP2 tests green; `GEN_CLEAN`; no `mediatime.v1.*` / `*__view*` files in `src/generated`.

- [ ] **Step 2:** Confirm `.github/workflows/codegen.yml` covers SP2. The existing `gen-is-committed` job runs `cargo run -p xtask -- gen` + `git diff --exit-code -- src/generated` (now also covers the new `findit.db.v1.*` files — no change needed; the xtask `.files()` edit is part of the committed tree). The `test-matrix` job runs `cargo test` / `--no-default-features` / `--all-features`. SP2's `#[cfg(feature="json")]` tests are exercised by `--all-features`; the SP1 plan already may have added `cargo test --features quickcheck,json` — if that step is **absent** from the `test-matrix` job, add `- run: cargo test --features quickcheck,json` (with the protoc-34.0 setup step already present). No other CI change.

- [ ] **Step 3: Open stacked PR** `sp2-database` → `sp1-common`:
```
git push -u origin sp2-database
gh pr create --repo Findit-AI/mediaschema --base sp1-common --head sp2-database \
  --title "SP2: migrate findit-proto database/ → findit.db.v1 (stacked on sp1-common)" \
  --body "<summary: Task 0 codegen scaffolding (cross-package media.v1 ref + mediatime extern, single-compile, no media.v1 extern) + 10 batches = 42 owned messages + 12 owned enums; DoD evidence; reuse/exclusion decisions per spec §6; stacked on sp1-common>"
```

---

## Self-Review

**1. Spec §6 coverage — every message/enum/batch maps to a task.**
- §6.3 — all **12 owned enums** in **Task 1** (`MediaKind, SubtitleTrackFormat, SubtitleTrackRole, ChannelLayoutKind, AudioChannelOrderKind, AudioClipKind, AudioPrefilterClass, AudioTrackRole, AudioContainerFormat, AudioCodec, AudioSampleFormat, TrackClassificationType`). ✓
- §6.5/§6.6 — all **42 owned messages** across Tasks 2–10: B2 `TagConfidence, SoundSource, AudioEvent, SpeakerSegment, AudioTranscriptSegment, AudioChannelSpec, Chromaprint, Ebur128, Timecode, CedDetection, Ced` (11); B3 `AudioChannelLayout, Clap, TrackTag` (3); B4 `VideoMeta, VideoTrackMeta, VideoStreamMeta, MediaMeta, SceneMeta, SubtitleMeta, SubtitleTrackMeta, SubtitleTrackOrigin, FailedFile` (9); B5 `Video, VideoTrack, Media, Subtitle, SubtitleTrack, SubtitleCue` (6); B6 `Scene, SceneVlmResult` (2); B7 `Keyframe` (1); B8 `AudioMeta, AudioStreamMeta, AudioTrackMeta, AudioSummary` (4); B9 `AudioAnalysis, TrackRecord, Audio, AudioTrack` (4); B10 `AudioCoverArt, AudioFileRecord` (2). 11+3+9+6+2+1+4+4+2 = **42**. ✓ (matches §6.6 total).
- §6.6 batch order honored exactly (enums → audio leaves → composite leaves → non-audio meta → non-audio wrappers → scene/vlm → Keyframe isolated → audio meta/summary → audio aggregates with `AudioAnalysis`/`TrackRecord` isolated → file record). ✓
- §6.7 exclusions are absent (no `findit.db.v1` message for the 5 clap wrappers, `CedTag`, `Iso6392B`, `VoiceRange`, any `*Ref`/`*Chunk`, macro/helper modules; `media.v1.*`/`mediatime.v1.*` referenced not redefined). The proto blocks contain ZERO of these as messages. ✓
- §6.8 ambiguity resolutions all baked in: #1 both `AudioFileRecord`+`AudioCoverArt` owned (B10, total 42); #2 `SceneVlmResult` tags 1..11 in declaration order (B6, commented); #3 bounded interior gaps `reserved`, wide bands verbatim (B4/B5/B6/B9/B10 proto comments + `reserved` lines; `AudioAnalysis`/`TrackRecord` bands have NO `reserved`); #4 `Iso6392B`→`uint32` / `CedTag`→`fixed64` (B9/B2, commented); #5 bare `Timebase`→`mediatime.v1.Timebase` vs `time`→`media.v1.TrackTime` (B4/B8/B9/B10); #6 `findit.db.v1.MediaKind` distinct + crate-root alias `DbMediaKind` (B1 re-export, commented); #7 `media.v1.Dimensions` referenced everywhere, no SP2 redefine (B4/B7/B8/B10). ✓

**2. Cross-package codegen front-loaded.** Task 0 creates the new package file + the exact one-line xtask `.files()` edit (verified against buffa-build 0.6.0 + buffa-codegen 0.6.0: single `compile()` over both protos → native `super::`-rooted cross-package refs via `context::TypePath`; `media.v1` NOT extern; `mediatime.v1` stays extern, not generated), the `findit_db` re-export block, the smoke message exercising a `media.v1` message + `media.v1` enum + inline bytes + `mediatime` extern, and a full feature-matrix + drift-gate + round-trip + JSON verification BEFORE any content batch. Exact expected `src/generated/` filenames stated (`findit.db.v1.mod.rs`, `findit.db.v1.database.rs`, `findit.db.v1.database.__oneof.rs` once B4 lands; `mediatime.v1` absent). ✓

**3. Placeholder scan.** Every batch's proto block is transcribed in full from §6's mapping tables (every `proto_type proto_name = tag;`, every enum value with discriminant, every `reserved`, intent comments) — NO "see §6", NO "TBD", NO "handle edge cases". Re-export idents are listed explicitly per batch. Test steps give the reusable `rt` recipe + a mandatory "confirm generated field names from src/generated" grep + concrete `batchN_sp2_roundtrip`/`#[cfg(feature="json")]` shapes (Task 0, batches 1–4 fully expanded as code; batches 5–10 give the exhaustive per-field construction recipe + reuse the established helpers — the same density as the SP1 plan, which similarly gave construction recipes for its later batches). No structural placeholders. ✓

**4. Type-name consistency.** Message/enum names are identical across proto blocks, re-export lists, and test descriptions in every task. `media.v1.*` always written as the proto FQN in proto and as the `mediaschema::<Type>` re-export in tests; `mediatime.v1.*` always `mediatime.v1.*` in proto / `::mediatime::*` in tests. The single rename (`DbMediaKind`) is introduced once (Task 1 Step 3) and used consistently in Task 5 and Task 1 tests. The `rt` helper + `make_*` helpers are defined once (in `tests/roundtrip.rs` from SP1) and reused; `sp2_track_time_one()` is defined once in batch 4 and reused by batches 8–9. Oneof companion exposure (`SubtitleTrackOriginSource`) mirrors the SP1 `LocationKind` pattern. ✓
