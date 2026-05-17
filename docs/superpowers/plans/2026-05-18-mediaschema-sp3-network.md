# SP3 (`network`) Implementation Plan

> **Mono-consolidation note:** This plan was authored for a 3-package split (`media.v1` / `findit.db.v1` / `findit.net.v1`). Per explicit user directive that split was consolidated — see `docs/superpowers/plans/2026-05-18-mediaschema-mono-consolidation.md` for the current execution plan. The per-batch field/tag/enum content in this file remains **authoritative** (nothing changed there); only the package header, proto file path, import lines, `media.v1.*`/`findit.db.v1.*` FQN prefixes (now bare same-package refs), and xtask `.files()` structure are superseded by the mono outcome. `findit.net.v1.FailedFile`→`NetFailedFile` (same-package rename; SP2's `FailedFile` keeps its name). Read the header/Conventions/Cross-package-mechanism sections below as historical context.

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`. Executed autonomously/continuously per user directive — no per-task human check-in.

**Goal:** ~~Migrate the full `findit-proto/src/network/` type set into a NEW `findit.net.v1` proto package~~ **[Mono-consolidation]** Migrate the full `findit-proto/src/network/` type set into the **single `media.v1` package** in `proto/media/v1/types.proto`, as clean buffa-generated proto3 with bare same-package refs (no cross-package refs), with round-trip/JSON/quickcheck tests. `FailedFile`→`NetFailedFile` (same-package rename; SP2's `FailedFile` keeps its name).

**Architecture:** ~~Add a NEW proto file `proto/findit/net/v1/network.proto` (package `findit.net.v1`) that `import`s `media/v1/types.proto`, `findit/db/v1/database.proto`, and `mediatime/v1/mediatime.proto`. Compile ALL THREE protos in one run; buffa-codegen emits `findit.net.v1` into `src/generated/**` as a sibling module of `media.v1` and `findit.db.v1` and resolves the three-way cross-package refs natively via `super::`-rooted paths (NOT extern).~~ **[Mono-consolidation]** Append SP3 messages directly into `proto/media/v1/types.proto` (package `media.v1`) across **Task 0 scaffolding + 9 dependency-ordered content batches** (§7.6); regenerate checked-in code via the single-entry xtask `.files([proto/media/v1/types.proto])`; extend `src/lib.rs` named re-exports under an SP3 block (the `NetFailedFile` alias disambiguates at the Rust crate root; the proto definition itself uses `NetFailedFile`); add tests to `tests/roundtrip.rs`. All former cross-package `media.v1.*` and `findit.db.v1.*` refs become bare same-package refs. `mediatime.v1.*` stays `extern_path`-mapped to `::mediatime` (NOT generated). No `findit-proto` dependency — fidelity is by faithful authoring from spec §7 (read-only reference).

**Tech Stack:** buffa/buffa-types/buffa-build `0.6` + mediatime `0.1.6` (crates.io), protoc 34.0, features `json`/`arbitrary`/`quickcheck`.

**Spec:** `docs/superpowers/specs/2026-05-17-mediaschema-full-migration-design.md` §7 (authoritative field mappings; §7.1–§7.9 incl. §7.8 #13–#14 mono-consolidation notes). §2 = locked decisions. §6 = SP2 context. Branch: `sp3-network` (off `sp2-database`, stacked).

---

## Cross-package codegen mechanism (historical — superseded by mono-consolidation)

> **Mono-consolidation note:** The three-way cross-package mechanism described here (separate `findit.net.v1` package/file, three-entry xtask `.files()`, native `super::`-rooted three-way cross-package resolution for `media.v1.*` + `findit.db.v1.*` + `mediatime.v1.*`) was the original SP3 approach. Under mono-consolidation all SP3 types are authored into the single `proto/media/v1/types.proto` (package `media.v1`); the xtask `.files()` retains its single entry; no cross-package refs exist (all same-package); `mediatime.v1` remains the sole extern. The technical description below is retained as historical context but is **no longer the execution path**.

Verified against `buffa-build 0.6.0` (`Config::{files,includes,extern_path,include_file,compile}`) and `buffa-codegen 0.6.0` (`generate`, `generate_package`, `generate_module_tree`, `context::TypePath`), and confirmed live by the already-merged SP2 two-way precedent:

- buffa-build's `.compile()` runs `protoc --include_imports` over **all** `.files()`, then `buffa_codegen::generate` groups the descriptors **by proto package** (`BTreeMap<package, files>`), emitting per-package output. `generate_module_tree` (driven by the `PackageMod` entries via `include_file("mod.rs")`) builds **one nested `pub mod` tree per package**, all rooted at the same `src/generated/mod.rs`.
- `buffa_codegen::context::TypePath` (context.rs:18–38) resolves a type reference's `to_package` prefix to one of: empty / `super::super` (same package), **`super::…::other_pkg` (cross-package, SAME compilation)**, or `::extern_crate::pkg` (extern only). Therefore, when `media.v1`, `findit.db.v1` AND `findit.net.v1` are compiled **in the same `compile()` call**, `findit.net.v1` references to **both** `media.v1.*` **and** `findit.db.v1.*` resolve **natively** to `super::super::super::media::v1::…` / `super::super::super::findit::db::v1::…` (sibling modules under the shared `generated` root) — identical to how the already-merged SP2 `findit.db.v1.database.rs` references `super::super::super::media::v1::…`. SP3 is simply a strictly larger resolution surface (the FIRST three-way consumer: `media.v1` + `findit.db.v1` + `mediatime.v1`) — the SAME mechanism SP2 already proved, just with one more in-crate package referenced.
- **Consequence (locked under original plan):** neither `media.v1` nor `findit.db.v1` is added as an `extern_path` for the net package. Adding either as extern would *suppress generation of the shared tree* and break native resolution. The ONLY xtask change is **adding the new proto file to `.files()`** (a third entry). `mediatime.v1` stays extern (`.mediatime.v1` → `::mediatime`) and continues NOT to be generated. All three protos already share `includes(&[root.join("proto")])`.

**Under mono-consolidation:** xtask `.files()` retains the single entry `root.join("proto/media/v1/types.proto")`. The `Sp3CodegenSmoke` fixture (Task 0) becomes a plain same-package `media.v1` message (the `media.v1.ErrorInfo`/`findit.db.v1.VideoMeta` cross-package refs become bare same-package refs; `mediatime.v1.TimeRange` extern is unchanged). Expected `src/generated/` after regen: `media.v1.types.rs` grows with SP3 content; `media.v1.types.__oneof.rs` gains `Request.kind`/`Response.kind`/`Event.kind` oneofs (batches 6–8). **No** `findit.net.v1.*` files are created. **`mediatime.v1` is still NOT generated** — no `mediatime.v1.*` files appear (extern-mapped); no `*__view*` files appear.

---

## Conventions (all batches)

> **Mono-consolidation note:** Under the mono outcome, substitute `proto/media/v1/types.proto` (package `media.v1`) for every reference to `proto/findit/net/v1/network.proto` (package `findit.net.v1`) below. All `media.v1.<Type>` and `findit.db.v1.<Type>` cross-package FQN refs become bare same-package refs (just `<Type>`). The one-entry xtask `.files([proto/media/v1/types.proto])` is unchanged. `NetFailedFile` is the proto definition name for the SP3 network-wire-type (not just a Rust re-export alias) — the proto definition in `types.proto` is `message NetFailedFile { … }`, replacing the original `message FailedFile { … }` for the SP3 type. SP2's `FailedFile` DB record keeps its name.

- Append proto into `proto/media/v1/types.proto` (package `media.v1`; **append-only** — every batch adds its block at end of file, never edits a prior block). *(Original: `proto/findit/net/v1/network.proto`, package `findit.net.v1` — superseded by mono-consolidation.)*
- **Reuse, never redefine.** Same-package refs are bare `<Type>` names (e.g. `Location`, `LocationTarget`, `Dimensions`, `ErrorInfo`, `VolumeMeta`, `WatchedLocation`, `Tag`, `VideoMeta`, `Video`, `Scene`). `mediatime` refs are written `mediatime.v1.TimeRange` (extern → `::mediatime`). **No SP3 message may redeclare any SP1/SP0/SP2 type.** `NetFailedFile` is intentionally a DIFFERENT type from SP2's `FailedFile` (same-package rename disambiguates; §7.1/§7.8 #13 — SP3's `NetFailedFile` is the frontend wire type `{kind, location, error, error_status, index_status}`; SP3 does NOT reference SP2's `FailedFile`). *(Original: `media.v1.<Type>` + `findit.db.v1.<Type>` FQN + separate `findit.net.v1` package — superseded.)*
- `Id`/`FileChecksum`/`checksum`/`*_id`/`scene_ids`/`location_id`/`search_id`/`volume_id`/`thumbnail` fields are **inline `bytes`** (or `repeated bytes`) — the 16-/32-byte newtype convention (§7.1) — NOT `Id`/`FileChecksum` message refs.
- **SP3 owns ZERO proto `enum`s** (§7.3). The 5 candidate "enums" (`ModelInfo.status`, `ModelDownloadProgress.status`, `FailedFile.kind`, `VolumeStateChangedEvent.event`, `FolderUpdatedEvent.event`) are unconstrained source `u32` → plain `uint32` with the value table reproduced verbatim in a self-contained proto field comment (NOT closed proto enums — protects round-trip of out-of-table values; §7.3/§7.8 #2/#6). `VideoIndexStatus` is the SP2 bitflag, NOT an SP3 type — emitted as an inline `uint32` at each use site with the full bit listing in a self-contained comment (§7.4). `MessageFlags` has no carrier (its only container `Header` is excluded) — no field emitted (§7.4/§7.7).
- `Header`/`MessageType`/`MessageFlags`/`RequestId`/`framing.rs`/`async_framing.rs`/`error.rs::ErrorResponse`/all `*Ref`/`*Chunk`/`RequestMessage`/`ResponseMessage` aliases/all `Encode`/`Decode`/`Arbitrary`/`define_oneof!` macro/`*_IDENTIFIER` modules are **EXCLUDED** (§7.2/§7.7). `RequestId` → inline `uint64` on the two envelopes. `ErrorResponse` → the `Response` error arm references `ErrorInfo` directly (bare same-package ref; NO `NetErrorResponse`).
- `Request`/`Response`/`Event` (Rust enums-with-data) → a proto3 `message` each with a single `oneof kind` (owned; mirrors SP2 `SubtitleTrackOrigin`, §6.2/§7.8 #1). The source `MessageType` discriminant integer of each variant is that arm's `oneof` field tag, **verbatim** — sparse, deliberately-banded values up to 20002, emitted **with NO `reserved`** (per §2 wide-band rule + SP2 §6.8 #3 `AudioAnalysis`/`TrackRecord` band precedent) — **except the single `Response.error` arm: 19999→20000, protobuf-reserved, §7.8 #12**. `Payload<T>` → two concrete messages `RequestEnvelope`/`ResponseEnvelope` (proto3 has no generics; §7.8 #3); `Event` has NO envelope (uncorrelated push).
- proto3: no `enum` is defined by SP3. Singular nested messages → `buffa::MessageField<T>`. `optional` scalar → `Option`. `repeated` → `Vec`. Non-zero Rust defaults (`Pagination.limit=50`, `SearchFilter.weight=1.0`) → plain `uint32`/`float`, **NOT** `optional` (the value round-trips exactly; the default is a documented client-side convention recorded in the field comment — §7.8 #9). **No source tag gap occurs in any owned SP3 message** (all leaf/request/response/event messages are contiguous from 1) → **no `reserved` anywhere in SP3** (§7.5); the only sparse numbering is the three oneof arm-tag sets, emitted verbatim with NO `reserved`.
- After each batch's proto edit: `cargo run -p xtask -- gen` (regenerates `src/generated/**`; expect `generated -> …`; the growing `media.v1.types.rs` with SP3 additions; **no `mediatime.v1` files, no `__view` files**, no new package directories). *(Original: "the new/updated `findit.net.v1.*` files plus unchanged `media.v1.*`/`findit.db.v1.*` bodies" — superseded by mono-consolidation.)*
- Extend `src/lib.rs` with the batch's new public message idents (named, not glob) under the SP3 re-export block (added in Task 0 — see Task 0 Step 4). Oneof companion idents follow the SP1/SP2 pattern (`pub use generated::media::v1::<msg_snake>::<OneofEnum> as <Alias>;` — exactly mirroring SP2's `subtitle_track_origin::Source as SubtitleTrackOriginSource`). *(Original path: `pub use generated::findit::net::v1::…` — superseded by mono-consolidation; the single `media.v1` package path is used.)*
- Tests use the existing `rt` helper in `tests/roundtrip.rs`:

```rust
fn rt<M: buffa::Message + PartialEq + std::fmt::Debug>(m: &M) {
    let bytes = m.encode_to_vec();
    let back = M::decode_from_slice(&bytes).expect("decode");
    assert_eq!(*m, back, "wire round-trip mismatch");
}
```

Per new type add: a populated instance + `M::default()`, both through `rt`. Empty messages (`ListLocationsRequest`, `RemoveLocationResponse`, `RetryFailedResponse`, `EjectVolumeResponse`, `GetModelStatusRequest`, `GetDaemonInfoRequest`, `UpdateAnnotationResponse`) have **no fields → the populated instance IS `::default()`**; round-trip `::default()` once (note this explicitly in the test — "empty message: populated == default"). Construct via the generated owned struct (public fields; nested singular message → `buffa::MessageField::some(...)`; `repeated` → `vec![...]`; oneof → `Some(<OneofAlias>::<Variant>(...))` — possibly `Box`ed, confirm from generated source). **Before writing each batch's tests, run** `grep -n "pub struct <T>\|pub enum <T>\|pub .*:\|MessageField\|EnumValue\|pub mod <msg_snake>" src/generated/media.v1.types*.rs` for the batch's types and match the EXACT generated field names/wrappers (buffa may raw-ident-escape, box oneof arms, or wrap nested messages); adjust constructions to the real generated API. All SP1/SP2/SP3 types are same-package `media.v1` — construct via the `mediaschema::<Type>` re-export; reuse the **already-present SP2 test builders** in `tests/roundtrip.rs`: `make_video_meta()`, `make_scene_meta()`, `make_error_info()`, `make_local_location()`, `make_detection`/`make_bbox`/`make_subject`, `sp2_track_time_one()`. *(Original: grep path `src/generated/findit.net.v1.network*.rs`; "nested `findit.db.v1` message" — superseded by mono-consolidation.)*
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
- Commit each batch on `sp3-network`:
```
git add proto/media/v1/types.proto src/generated src/lib.rs tests/roundtrip.rs
git commit -m "feat(sp3): <batch name> media.v1 SP3-network types"
```
(append `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`. `xtask/src/main.rs` only changes in Task 0; later commits will simply have no xtask diff to stage.)
- Keep the top-level `use mediaschema::{…}` import block in `tests/roundtrip.rs` fully ascending-alphabetical when adding SP3 idents (mirrors the existing SP1/SP2 ordering at the file head).

---

## Task 0: SP3 codegen scaffolding (de-risk the THREE-WAY cross-package + extern BEFORE any content)

**Files:** create `proto/findit/net/v1/network.proto`; modify `xtask/src/main.rs`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate `src/generated/**`.

This task proves the NEW-package + cross-`media.v1`-ref + cross-`findit.db.v1`-ref + `mediatime` extern pipeline end-to-end with ONE tiny smoke message. SP3 is the FIRST three-way consumer — this front-loads the entire SP3 risk. No content batch proceeds until this is green.

- [ ] **Step 1: Create `proto/findit/net/v1/network.proto`** (header + all three imports + ONE smoke message referencing exactly one `media.v1` message, one `findit.db.v1` message, an inline `bytes` id, and one `optional mediatime.v1.TimeRange`):

```protobuf
syntax = "proto3";

package findit.net.v1;

// SP3 — findit `network` domain (daemon<->desktop protocol). Authored from
// findit-proto's src/network/ Rust as a clean proto3 redesign (NO wire-compat
// with the hand-rolled framing/encoding; correctness = round-trip + semantic
// fidelity).
//
// REUSE, NEVER REDEFINE: every media.v1.* / findit.db.v1.* / mediatime.v1.*
// type below is referenced from its own package. buffa compiles this file
// together with proto/media/v1/types.proto AND proto/findit/db/v1/database.proto
// in one run and resolves the THREE-WAY cross-package refs natively
// (super::-rooted); mediatime.v1.* is extern-mapped to ::mediatime and is NOT
// generated. SP3 owns ZERO proto enums (the source "enums" are unconstrained
// u32 — modeled as plain uint32 with the value table in a field comment, so
// out-of-table values round-trip). Header/MessageType/MessageFlags/RequestId/
// framing/*Ref/*Chunk/ErrorResponse are EXCLUDED (transport plumbing /
// ownership mirrors). findit.net.v1.FailedFile is a DIFFERENT type from
// findit.db.v1.FailedFile (the distinct package disambiguates).

import "media/v1/types.proto";
import "findit/db/v1/database.proto";
import "mediatime/v1/mediatime.proto";

// Task-0 codegen smoke fixture (NOT a domain message): proves the THREE-WAY
// findit.net.v1 -> media.v1 + findit.net.v1 -> findit.db.v1 cross-package
// references, the inline-bytes id convention, and the mediatime.v1.TimeRange
// extern, all resolving in a single buffa codegen run. Kept permanently: it
// is the permanent regression guard for the SP3 three-way cross-package
// pipeline (mirrors SP0's TimedDetection / SP2's Sp2CodegenSmoke fixture role).
message Sp3CodegenSmoke {
  bytes id = 1;                              // inline-bytes id convention (§7.1)
  media.v1.ErrorInfo error = 2;              // cross-package media.v1 ref
  findit.db.v1.VideoMeta video_meta = 3;     // cross-package findit.db.v1 ref
  optional mediatime.v1.TimeRange range = 4; // mediatime extern (-> ::mediatime)
}
```

- [ ] **Step 2: Apply the xtask edit.** In `xtask/src/main.rs`, replace the existing two-entry `.files(&[ … ])` block

```rust
        .files(&[
            root.join("proto/media/v1/types.proto"),
            root.join("proto/findit/db/v1/database.proto"),
        ])
```

with the three-entry block

```rust
        .files(&[
            root.join("proto/media/v1/types.proto"),
            root.join("proto/findit/db/v1/database.proto"),
            root.join("proto/findit/net/v1/network.proto"),
        ])
```

(No other change to `xtask/src/main.rs`. Do NOT add an `extern_path` for `media.v1` or `findit.db.v1` — see "Cross-package codegen mechanism".)

- [ ] **Step 3: Regenerate** — `env CARGO_HOME=/Users/user/Develop/findit-studio/.cargo cargo run -p xtask -- gen`. Expect `generated -> …/src/generated`. Confirm with `ls src/generated`: the new files `findit.net.v1.mod.rs` and `findit.net.v1.network.rs` exist (no `findit.net.v1.network.__oneof.rs` yet — `Sp3CodegenSmoke` has no oneof); the existing `media.v1.*` and `findit.db.v1.*` files are still present and **unchanged** (`git diff --stat -- src/generated/media.v1.types.rs src/generated/findit.db.v1.database.rs` shows no change to those bodies); there is **no `mediatime.v1.*` file** and **no `*__view*` file**. Confirm `src/generated/mod.rs` now contains, inside the existing `pub mod findit { … }`, a `pub mod net { use super::*; pub mod v1 { use super::*; include!("findit.net.v1.mod.rs"); } }` block alongside the existing `pub mod db { … }` (the shared `findit` wrapper is regenerated, not duplicated — see the mechanism section), and the whole sits alongside `pub mod media`.

- [ ] **Step 4: Add the SP3 re-export block to `src/lib.rs`.** After the existing SP2 `findit.db.v1` re-export block (the `pub use generated::findit::db::v1::{…};` brace list, the `pub use … MediaKind as DbMediaKind;` line, and the `pub use … subtitle_track_origin::Source as SubtitleTrackOriginSource;` line — i.e. at the very end of the current `src/lib.rs`), append:

```rust

// SP3 — findit `network` package (`findit.net.v1`). Referenced media.v1 +
// findit.db.v1 types are re-exported above (SP1/SP2); these are the
// network-domain owned types (the daemon<->desktop protocol). Named (not
// glob) so buffa internals stay private. Re-exported under the crate root to
// match the media.v1/findit.db.v1 surface. SP3 introduces NO name colliding
// with media.v1 or findit.db.v1 — note `findit.net.v1.FailedFile` is a
// DIFFERENT type from the SP2 `findit.db.v1.FailedFile` (already re-exported
// as `FailedFile` above); the SP3 one is exposed as `NetFailedFile` so both
// stay reachable at the crate root (proto packages stay distinct; only this
// Rust re-export alias disambiguates — mirrors the SP2 `DbMediaKind` precedent,
// spec §7.1/§7.8). The 3 oneof envelopes `Request`/`Response`/`Event` and the
// 2 correlation envelopes `RequestEnvelope`/`ResponseEnvelope` are added in
// batches 6–9; their `kind` oneof companion idents (`RequestKind`/
// `ResponseKind`/`EventKind`) follow the SP2 `SubtitleTrackOriginSource`
// pattern and go immediately after this block.
pub use generated::findit::net::v1::{
    Sp3CodegenSmoke,
};
```

(Each later batch extends THIS brace list with its new idents, EXCEPT `findit.net.v1.FailedFile` which is added as the aliased `NetFailedFile` re-export — introduced in Task 2 Step 3 (its batch). Oneof companion `pub use` lines for `Request`/`Response`/`Event` go immediately after this block, mirroring the SP2 `subtitle_track_origin::Source as SubtitleTrackOriginSource` line — introduced in their respective batches 6/7/8.)

- [ ] **Step 5: Add the Task-0 smoke round-trip test** to `tests/roundtrip.rs`. First confirm generated field names: `grep -n "pub struct Sp3CodegenSmoke\|pub .*:\|MessageField\|EnumValue" src/generated/findit.net.v1.network.rs`. Add `Sp3CodegenSmoke` to the top-level `mediaschema::{…}` import list in `tests/roundtrip.rs` (keep the list ascending-alphabetical). Then add (the nested `findit.db.v1.VideoMeta` is built via the already-present SP2 `make_video_meta()` helper; `media.v1.ErrorInfo` + `mediatime::TimeRange` via the existing forms):

```rust
// ── SP3 Task 0: three-way cross-package + extern codegen smoke ──────────────

#[test]
fn sp3_codegen_smoke_roundtrip() {
    use mediaschema::Sp3CodegenSmoke;
    // Populated: media.v1 MESSAGE ref + findit.db.v1 MESSAGE ref + inline
    // bytes + mediatime extern, all in one findit.net.v1 message.
    let s = Sp3CodegenSmoke {
        id: vec![0x01, 0x02, 0x03, 0x04],
        error: make_error_info(),                 // buffa::MessageField<ErrorInfo> (SP2 builder)
        video_meta: make_video_meta(),            // buffa::MessageField<VideoMeta> (SP2 builder)
        range: buffa::MessageField::some(mediatime::TimeRange::new(
            10,
            20,
            mediatime::Timebase::new(30000, core::num::NonZeroU32::new(1001).unwrap()),
        )),
        ..Default::default()
    };
    rt(&s);
    // Default (all empty / no extern set) — guards the absent-extern path.
    rt(&Sp3CodegenSmoke::default());
}

#[test]
#[cfg(feature = "json")]
fn sp3_codegen_smoke_json_roundtrip() {
    use mediaschema::Sp3CodegenSmoke;
    let s = Sp3CodegenSmoke {
        id: vec![0xAA, 0xBB],
        error: make_error_info(),
        video_meta: make_video_meta(),
        range: buffa::MessageField::some(mediatime::TimeRange::new(
            0,
            5,
            mediatime::Timebase::new(1, core::num::NonZeroU32::new(48000).unwrap()),
        )),
        ..Default::default()
    };
    let json = serde_json::to_string(&s).expect("to_json");
    let back: Sp3CodegenSmoke = serde_json::from_str(&json).expect("from_json");
    assert_eq!(s, back);
}
```

(If the grep shows `error`/`video_meta` are not bare `buffa::MessageField<…>` or `id` is raw-ident-escaped/`Bytes`-typed, adjust per the real signature. `make_error_info()`/`make_video_meta()` are already defined in `tests/roundtrip.rs` from SP2 — reuse them, do not redefine. `mediatime` is already a dev-dependency and imported in `tests/roundtrip.rs`.)

- [ ] **Step 6: Verify** (Conventions verification block). Expected: all 6 builds PASS, `cargo test --features quickcheck,json` shows all tests passing (incl. `sp3_codegen_smoke_roundtrip` + the SP0/SP1/SP2 suite still green), `GEN_CLEAN`. This green state proves the three-way cross-package + extern mechanism; content batches may proceed.

- [ ] **Step 7: Commit** — `feat(sp3): batch0 codegen scaffolding (findit.net.v1 package + three-way cross-package/extern smoke)` (stage incl. `proto/findit/net/v1/network.proto` and `xtask/src/main.rs`).

---

## Task 1: Batch 1 — scalar leaves (no cross-package, no SP3 deps)

`Pagination, SearchFilter, HeartbeatRequest, HeartbeatResponse`. (Bare-`u32` doc-enums and `VideoIndexStatus` are inline `uint32` — no standalone type; materialized at use sites in later batches.)

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (non-zero Rust defaults → plain scalar, NOT `optional`, with the client default in the comment — §7.8 #9)

```protobuf
// ── Batch 1: scalar leaves (§7.5 shared.rs/heartbeat.rs) ────────────────────

// `limit`: client default 50 when absent (documented client-side
// convention, NOT `optional` — the wire format carries no presence bit;
// §7.8 #9). `offset` default 0.
message Pagination {
  uint32 limit = 1;
  uint32 offset = 2;
}

// `weight`: client default 1.0 when absent (documented client-side
// convention, NOT `optional`; §7.8 #9).
message SearchFilter {
  string key = 1;
  string value = 2;
  float weight = 3;
}

// Unix epoch milliseconds.
message HeartbeatRequest { int64 timestamp = 1; }

// Unix epoch milliseconds.
message HeartbeatResponse { int64 timestamp = 1; }
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the `pub use generated::findit::net::v1::{…};` brace list in `src/lib.rs` with: `HeartbeatRequest, HeartbeatResponse, Pagination, SearchFilter`.
- [ ] **Step 4: Tests** — confirm fields: `grep -n "pub struct Pagination\|pub struct SearchFilter\|pub struct HeartbeatRequest\|pub struct HeartbeatResponse\|pub .*:" src/generated/findit.net.v1.network.rs`. Add the 4 idents to the top-level `mediaschema::{…}` import (keep ascending-alphabetical). Add `batch1_sp3_roundtrip` (populated + `::default()` for each; `Pagination` with non-default `limit`/`offset` to prove the no-`optional` plain scalar round-trips exactly; `SearchFilter` with `weight: 0.5` to prove a non-1.0 value round-trips). Add `#[cfg(feature="json")] batch1_sp3_json_roundtrip` for a populated `SearchFilter` (covers the non-default `float` under serde):

```rust
// ── SP3 Batch 1: scalar leaves ──────────────────────────────────────────────

#[test]
fn batch1_sp3_roundtrip() {
    use mediaschema::{HeartbeatRequest, HeartbeatResponse, Pagination, SearchFilter};
    rt(&Pagination { limit: 25, offset: 100, ..Default::default() });
    rt(&Pagination::default()); // limit/offset == 0 on the wire (client supplies 50/0 on absence)
    rt(&SearchFilter { key: "scene".into(), value: "beach".into(), weight: 0.5, ..Default::default() });
    rt(&SearchFilter::default()); // weight == 0.0 on the wire (client supplies 1.0 on absence)
    rt(&HeartbeatRequest { timestamp: 1_700_000_000_123, ..Default::default() });
    rt(&HeartbeatRequest::default());
    rt(&HeartbeatResponse { timestamp: 1_700_000_000_456, ..Default::default() });
    rt(&HeartbeatResponse::default());
}

#[test]
#[cfg(feature = "json")]
fn batch1_sp3_json_roundtrip() {
    use mediaschema::SearchFilter;
    let f = SearchFilter { key: "tag".into(), value: "sunset".into(), weight: 2.5, ..Default::default() };
    let json = serde_json::to_string(&f).expect("to_json");
    let back: SearchFilter = serde_json::from_str(&json).expect("from_json");
    assert_eq!(f, back);
}
```

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch1 scalar leaves (Pagination/SearchFilter/HeartbeatRequest/HeartbeatResponse)`.

---

## Task 2: Batch 2 — reuse-only leaves (cross-package refs, no SP3 deps)

`SearchHit, BrowseItem, ModelInfo, ModelDownloadProgress, IndexingFile, FailedFile`.

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`scene_id`/`video_id` inline `bytes`; `range` = `mediatime.v1.TimeRange` extern; `dimensions` = `media.v1.Dimensions` split, §7.8 #8; `BrowseItem.meta` = `findit.db.v1.VideoMeta` cross-package; the 3 bare-`u32` doc-enums → plain `uint32` with the value table verbatim in the comment, §7.3; `error_status`/`index_status`/`completed_phases` = inline `uint32` VideoIndexStatus with full bit listing, §7.4 — NO SP3 type)

```protobuf
// ── Batch 2: reuse-only leaves (§7.5 shared.rs) ─────────────────────────────

// `scene_id`/`video_id`: inline bytes (Id convention, §7.1). `range`:
// mediatime.v1.TimeRange extern (-> ::mediatime). `dimensions`:
// media.v1.Dimensions (SP1 split uint32 width/height; source packed u32
// via to_u32() — clean-redesign, §7.8 #8).
message SearchHit {
  bytes scene_id = 1;
  bytes video_id = 2;
  string video_name = 3;
  media.v1.Location location = 4;
  string description = 5;
  float score = 6;
  mediatime.v1.TimeRange range = 7;
  bytes thumbnail = 8;
  media.v1.Dimensions dimensions = 9;
}

// `meta`: findit.db.v1.VideoMeta (SP2 cross-package ref). `scene_count`
// src u16 varint -> uint32. `thumbnail`: inline bytes.
message BrowseItem {
  findit.db.v1.VideoMeta meta = 1;
  media.v1.Location location = 2;
  uint32 scene_count = 3;
  bytes thumbnail = 4;
}

// `status`: bare u32 enum-by-convention (NOT a proto enum, §7.3) —
// 0=not_downloaded, 1=downloading, 2=ready, 3=error. Values outside this
// set round-trip exactly (the migration's correctness criterion).
message ModelInfo {
  string name = 1;
  uint32 status = 2;
  uint64 size_bytes = 3;
}

// `status`: bare u32 enum-by-convention (NOT a proto enum, §7.3) —
// 0=pending, 1=downloading, 2=paused, 3=completed, 4=error (distinct
// value set from ModelInfo.status; documentary only). `progress` 0.0–1.0.
message ModelDownloadProgress {
  string name = 1;
  float progress = 2;
  uint64 downloaded_bytes = 3;
  uint64 total_bytes = 4;
  uint32 status = 5;
  string error_msg = 6;
}

// `kind`: bare u32 enum-by-convention (NOT a proto enum, §7.3) — 0=video,
// 1=audio. `error_status`/`index_status`: inline uint32 VideoIndexStatus
// (SP2 bitflag reused by-value, §7.4) — bits: PROBED=0x01,
// SCENE_DETECTED=0x02, KEYFRAME_EXTRACTED=0x04, VLM_ANALYZED=0x10,
// APPLE_VISION_ANALYZED=0x20, TEXT_EMBEDDING_FINISHED=0x40,
// SCENE_EMBEDDING_FINISHED=0x80 (source gap at 0x08). DISTINCT type from
// findit.db.v1.FailedFile (the distinct package disambiguates, §7.1).
message FailedFile {
  uint32 kind = 1;
  media.v1.Location location = 2;
  media.v1.ErrorInfo error = 3;
  uint32 error_status = 4;
  uint32 index_status = 5;
}

// `completed_phases`: inline uint32 VideoIndexStatus (SP2 bitflag reused
// by-value, §7.4) — bits: PROBED=0x01, SCENE_DETECTED=0x02,
// KEYFRAME_EXTRACTED=0x04, VLM_ANALYZED=0x10, APPLE_VISION_ANALYZED=0x20,
// TEXT_EMBEDDING_FINISHED=0x40, SCENE_EMBEDDING_FINISHED=0x80 (src gap
// at 0x08).
message IndexingFile {
  media.v1.Location location = 1;
  string name = 2;
  uint32 completed_phases = 3;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the `pub use generated::findit::net::v1::{…};` brace list with: `BrowseItem, IndexingFile, ModelDownloadProgress, ModelInfo, SearchHit`. Then add the aliased re-export for `findit.net.v1.FailedFile` (the only name colliding with the SP2 `findit.db.v1.FailedFile` already re-exported at the crate root as `FailedFile`) immediately after the brace block:

```rust
/// Network-layer failed-file wire record (`{kind, location, error,
/// error_status, index_status}`). Aliased to avoid colliding with the SP2
/// [`FailedFile`] (the `findit.db.v1` DB record). The proto packages stay
/// distinct (`findit.net.v1.FailedFile`); only this crate-root re-export is
/// renamed — spec §7.1/§7.8 (mirrors the SP2 `DbMediaKind` precedent).
pub use generated::findit::net::v1::FailedFile as NetFailedFile;
```

- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct SearchHit\|pub struct BrowseItem\|pub struct ModelInfo\|pub struct ModelDownloadProgress\|pub struct IndexingFile\|pub struct FailedFile\|pub .*:\|MessageField" src/generated/findit.net.v1.network.rs`. Add `BrowseItem, IndexingFile, ModelDownloadProgress, ModelInfo, NetFailedFile, SearchHit` to the top-level `mediaschema::{…}` import (keep ascending-alphabetical; `make_local_location`/`make_video_meta`/`make_error_info` are already in-file from SP2). Add `batch2_sp3_roundtrip`:
  - `SearchHit`: `scene_id`/`video_id`/`thumbnail` non-empty bytes, `location: buffa::MessageField::some(make_local_location())`, `range: buffa::MessageField::some(mediatime::TimeRange::new(..))` (extern), `dimensions: buffa::MessageField::some(Dimensions{ width:1920, height:1080, ..Default::default() })` (SP1 split), scalars set; a 2nd case `location`/`range`/`dimensions` = `MessageField::none()`, empty bytes; + default.
  - `BrowseItem`: `meta: make_video_meta()` (SP2 `findit.db.v1.VideoMeta` builder), `location: buffa::MessageField::some(make_local_location())`, `scene_count: 42`, `thumbnail: vec![0xAB, 0xCD]`; a 2nd case `meta`/`location` = none, empty thumbnail; + default.
  - `ModelInfo`: `status: 2`; AND a `status: 99` case (out-of-table value — proves the plain-`uint32` choice round-trips an undocumented discriminant, §7.3); + default.
  - `ModelDownloadProgress`: `status: 4`, `progress: 0.73`, `error_msg: ""`; AND a `status: 250` out-of-table case; + default.
  - `NetFailedFile` (the SP3 `findit.net.v1.FailedFile`): `kind: 1`, `location: buffa::MessageField::some(make_local_location())`, `error: make_error_info()`, `error_status: 0x01|0x02`, `index_status: 0x01|0x80`; AND a `kind: 7` out-of-table case with `location`/`error` = none; + default.
  - `IndexingFile`: `location: buffa::MessageField::some(make_local_location())`, `name: "a.mp4".into()`, `completed_phases: 0x01|0x04|0x40`; a 2nd case `location: none()`; + default.
  - `#[cfg(feature="json")] batch2_sp3_json_roundtrip` for a populated `SearchHit` (covers `media.v1.Location` + `media.v1.Dimensions` + `mediatime.v1.TimeRange` extern + inline bytes under serde) and a populated `BrowseItem` (covers the `findit.db.v1.VideoMeta` cross-package ref under serde).

(Reuse the in-file SP2 builders `make_local_location()`, `make_video_meta()`, `make_error_info()`. Fully expand every case per the confirmed generated field names — do not abbreviate.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch2 reuse-only leaves (SearchHit/BrowseItem/ModelInfo/ModelDownloadProgress/FailedFile/IndexingFile)`.

---

## Task 3: Batch 3 — empty + simple request/response leaves

Empty: `ListLocationsRequest, RemoveLocationResponse, RetryFailedResponse, EjectVolumeResponse, GetModelStatusRequest, GetDaemonInfoRequest, UpdateAnnotationResponse`. Inline-bytes: `EjectVolumeRequest, GetIndexedFileRequest, GetFileIndexingStatsRequest`. `media.v1.Location`/`Pagination`: `GetLocationStatsRequest, RemoveLocationRequest, RetryFailedRequest, BrowseRequest`. Reuse: `IndexLocationRequest` (`media.v1.LocationTarget`), `IndexLocationResponse` (`media.v1.WatchedLocation`), `UpdateAnnotationRequest` (`repeated bytes` + `media.v1.Tag`), `SearchResponse`, `GetDaemonInfoResponse`.

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (empty messages have zero fields; `*_id`/`checksum`/`search_id`/`volume_id` inline `bytes`; `scene_ids` `repeated bytes`; cross-package refs FQN)

```protobuf
// ── Batch 3: empty + simple request/response leaves (§7.5 folder.rs/
//    volume_eject.rs/file.rs/daemon.rs/search.rs/browse.rs/annotation.rs) ────

// Empty request (zero fields) — the discriminant alone carries intent.
message ListLocationsRequest {}

message RemoveLocationResponse {}

message RetryFailedResponse {}

message EjectVolumeResponse {}

message GetModelStatusRequest {}

message GetDaemonInfoRequest {}

message UpdateAnnotationResponse {}

// `volume_id`: inline bytes (Id convention, §7.1).
message EjectVolumeRequest { bytes volume_id = 1; }

// `checksum`: inline bytes (FileChecksum convention, §7.1).
message GetIndexedFileRequest { bytes checksum = 1; }

// `video_id`: inline bytes (Id convention, §7.1).
message GetFileIndexingStatsRequest { bytes video_id = 1; }

message GetLocationStatsRequest { media.v1.Location location = 1; }

message RemoveLocationRequest { media.v1.Location location = 1; }

message RetryFailedRequest { media.v1.Location location = 1; }

message BrowseRequest {
  media.v1.Location location = 1;
  Pagination pagination = 2;
}

message IndexLocationRequest { media.v1.LocationTarget target = 1; }

message IndexLocationResponse { media.v1.WatchedLocation folder = 1; }

// `scene_ids`: repeated bytes (Id convention, §7.1). `user_tags`:
// media.v1.Tag (SP1 cross-package ref).
message UpdateAnnotationRequest {
  repeated bytes scene_ids = 1;
  repeated media.v1.Tag user_tags = 2;
}

// `search_id`: inline bytes (Id convention, §7.1).
message SearchResponse {
  bytes search_id = 1;
  uint32 total_count = 2;
}

message GetDaemonInfoResponse {
  string version = 1;
  int64 started_at = 2;
  uint64 total_videos = 3;
  uint64 total_scenes = 4;
  uint32 active_tasks = 5;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `BrowseRequest, EjectVolumeRequest, EjectVolumeResponse, GetDaemonInfoRequest, GetDaemonInfoResponse, GetFileIndexingStatsRequest, GetIndexedFileRequest, GetLocationStatsRequest, GetModelStatusRequest, IndexLocationRequest, IndexLocationResponse, ListLocationsRequest, RemoveLocationRequest, RemoveLocationResponse, RetryFailedRequest, RetryFailedResponse, SearchResponse, UpdateAnnotationRequest, UpdateAnnotationResponse`.
- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct ListLocationsRequest\|pub struct EjectVolumeRequest\|pub struct BrowseRequest\|pub struct IndexLocationRequest\|pub struct UpdateAnnotationRequest\|pub struct SearchResponse\|pub struct GetDaemonInfoResponse\|pub .*:\|MessageField" src/generated/findit.net.v1.network.rs`. Add the 19 idents to the top-level `mediaschema::{…}` import (keep ascending-alphabetical; SP1 `LocationTarget`, `LocationTargetKind`, `WatchedLocation`, `Tag`, `Id`, `Local`, `Location`, `LocationKind` are already in-file). Add `batch3_sp3_roundtrip`:
  - The 7 empty messages: round-trip `::default()` ONCE each, with a comment `// empty message: populated == default (zero fields)`.
  - `EjectVolumeRequest`/`GetIndexedFileRequest`/`GetFileIndexingStatsRequest`: non-empty bytes + default.
  - `GetLocationStatsRequest`/`RemoveLocationRequest`/`RetryFailedRequest`: `location: buffa::MessageField::some(make_local_location())`; a `none()` case; + default.
  - `BrowseRequest`: `location: buffa::MessageField::some(make_local_location())`, `pagination: buffa::MessageField::some(Pagination{ limit:10, offset:0, ..Default::default() })`; a both-`none()` case; + default.
  - `IndexLocationRequest`: `target: buffa::MessageField::some(LocationTarget{ kind: Some(LocationTargetKind::Local("/tmp/media".into())), ..Default::default() })` (reuse the exact SP1 `LocationTarget` construction form); a `none()` case; + default.
  - `IndexLocationResponse`: `folder` = `buffa::MessageField::some(WatchedLocation{ id: buffa::MessageField::some(Id{ value:(1u8..=16).collect(), ..Default::default() }), location: buffa::MessageField::some(make_local_location()), name:"L".into(), status:1, created_at:1, deleted_at:None, total_files:1, indexed_files:1, total_videos:1, indexed_videos:1, total_scenes:1, total_audios:1, indexed_audios:1, total_failed_files:0, failed_videos:0, failed_audios:0, ..Default::default() })` (reuse the exact SP1 `WatchedLocation` field set); a `none()` case; + default.
  - `UpdateAnnotationRequest`: `scene_ids: vec![vec![1], vec![2]]`, `user_tags: vec![Tag{ name:"fav".into(), color:0xFF_AA_00_FF, ..Default::default() }]` (reuse the SP1 `Tag` form); an empty-both case; + default.
  - `SearchResponse`: `search_id: vec![9,9]`, `total_count: 123`; + default.
  - `GetDaemonInfoResponse`: all 5 fields set; + default.
  - `#[cfg(feature="json")] batch3_sp3_json_roundtrip` for a populated `IndexLocationResponse` (covers the `media.v1.WatchedLocation` cross-package ref + nested `media.v1.Location` oneof under serde) and a populated `UpdateAnnotationRequest` (covers `repeated bytes` + repeated `media.v1.Tag` under serde).

(Reuse the in-file SP1 `make_local_location()` and the SP1 inline `LocationTarget`/`WatchedLocation`/`Tag` construction forms. Fully expand every case per the confirmed generated field names.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch3 empty + simple request/response leaves (ListLocationsRequest/RemoveLocationResponse/RetryFailedResponse/EjectVolumeResponse/GetModelStatusRequest/GetDaemonInfoRequest/UpdateAnnotationResponse/EjectVolumeRequest/GetIndexedFileRequest/GetFileIndexingStatsRequest/GetLocationStatsRequest/RemoveLocationRequest/RetryFailedRequest/BrowseRequest/IndexLocationRequest/IndexLocationResponse/UpdateAnnotationRequest/SearchResponse/GetDaemonInfoResponse)`.

---

## Task 4: Batch 4 — composite responses (depend on batch-2/3 leaves)

`Volume, ListLocationsResponse, GetLocationStatsResponse, FailedFilesResponse, SearchRequest, BrowseResponse, GetIndexedFileResponse, GetFileIndexingStatsResponse, GetModelStatusResponse, ModelDownloadProgressResponse, IndexingProgressResponse`.

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`Volume`/`ListLocationsResponse`/`GetLocationStatsResponse`/`FailedFilesResponse`/`BrowseResponse` depend on batch-2 `FailedFile`/`BrowseItem` + batch-1 `Pagination`; `SearchRequest` on batch-1 `Pagination`/`SearchFilter`; `GetIndexedFileResponse` on `findit.db.v1.Video`/`Scene` cross-package; `ModelDownloadProgressResponse` is the thin wrapper kept as a named **response** message referencing the shared batch-2 `ModelDownloadProgress`, §7.8 #4; `index_status` inline `uint32` VideoIndexStatus, §7.4; `location_id` inline `bytes`)

```protobuf
// ── Batch 4: composite responses (§7.5 folder.rs/search.rs/browse.rs/file.rs/
//    model.rs/progress.rs) ────────────────────────────────────────────────

message Volume {
  media.v1.VolumeMeta meta = 1;
  repeated media.v1.WatchedLocation folders = 2;
}

message ListLocationsResponse { repeated Volume groups = 1; }

message GetLocationStatsResponse {
  uint64 total_files = 1;
  uint64 indexed_files = 2;
  uint64 total_videos = 3;
  uint64 total_scenes = 4;
  uint64 total_audios = 5;
  repeated FailedFile failed_files = 6;
}

// `location_id`: inline bytes (Id convention, §7.1).
message FailedFilesResponse {
  bytes location_id = 1;
  repeated FailedFile failed_files = 2;
}

message SearchRequest {
  string query = 1;
  Pagination pagination = 2;
  repeated SearchFilter filters = 3;
}

message BrowseResponse {
  repeated BrowseItem items = 1;
  uint32 total_count = 2;
  Pagination pagination = 3;
}

// `video`/`scenes`: findit.db.v1.Video / findit.db.v1.Scene (SP2
// cross-package refs).
message GetIndexedFileResponse {
  findit.db.v1.Video video = 1;
  repeated findit.db.v1.Scene scenes = 2;
}

// `video_id`: inline bytes (Id convention, §7.1). `index_status`: inline
// uint32 VideoIndexStatus (SP2 bitflag reused by-value, §7.4) — bits:
// PROBED=0x01, SCENE_DETECTED=0x02, KEYFRAME_EXTRACTED=0x04,
// VLM_ANALYZED=0x10, APPLE_VISION_ANALYZED=0x20,
// TEXT_EMBEDDING_FINISHED=0x40, SCENE_EMBEDDING_FINISHED=0x80 (src gap at
// 0x08). `error`: source Option<ErrorInfo> -> optional media.v1.ErrorInfo
// (source normalizes empty->None; presence per §2).
message GetFileIndexingStatsResponse {
  bytes video_id = 1;
  uint32 index_status = 2;
  optional media.v1.ErrorInfo error = 3;
}

message GetModelStatusResponse { repeated ModelInfo models = 1; }

// Thin wrapper kept as a named message — distinct RESPONSE role; references
// the shared batch-2 ModelDownloadProgress leaf (NOT a re-typed copy). NOT
// collapsed: it is a separate decode target from ModelDownloadProgressEvent
// in the source dispatch (response vs push role, §7.8 #4/#10).
message ModelDownloadProgressResponse { ModelDownloadProgress model = 1; }

message IndexingProgressResponse {
  media.v1.Location location = 1;
  uint64 total_files = 2;
  uint64 indexed_files = 3;
  repeated IndexingFile active_files = 4;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `BrowseResponse, FailedFilesResponse, GetFileIndexingStatsResponse, GetIndexedFileResponse, GetLocationStatsResponse, GetModelStatusResponse, IndexingProgressResponse, ListLocationsResponse, ModelDownloadProgressResponse, SearchRequest, Volume`.
- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct Volume\b\|pub struct ListLocationsResponse\|pub struct GetLocationStatsResponse\|pub struct FailedFilesResponse\|pub struct SearchRequest\|pub struct BrowseResponse\|pub struct GetIndexedFileResponse\|pub struct GetFileIndexingStatsResponse\|pub struct GetModelStatusResponse\|pub struct ModelDownloadProgressResponse\|pub struct IndexingProgressResponse\|pub .*:\|MessageField" src/generated/findit.net.v1.network.rs`. Add the 11 idents to the top-level `mediaschema::{…}` import (keep ascending-alphabetical; SP1 `VolumeMeta`/`WatchedLocation` and the SP2 `make_video_meta`/`make_scene_meta` builders are already in-file). Add `batch4_sp3_roundtrip`:
  - `Volume`: `meta` = `buffa::MessageField::some(VolumeMeta{ id: buffa::MessageField::some(Id{ value:(1u8..=16).collect(), ..Default::default() }), location: buffa::MessageField::some(make_local_location()), name:"Disk".into(), total_size:1000, used_size:500, status:3, ..Default::default() })` (reuse the SP1 `VolumeMeta` form), `folders: vec![WatchedLocation{ id: buffa::MessageField::some(Id{ value:(1u8..=16).collect(), ..Default::default() }), location: buffa::MessageField::some(make_local_location()), name:"L".into(), status:1, created_at:1, deleted_at:None, total_files:1, indexed_files:1, total_videos:1, indexed_videos:1, total_scenes:1, total_audios:1, indexed_audios:1, total_failed_files:0, failed_videos:0, failed_audios:0, ..Default::default() }]` (reuse the SP1 `WatchedLocation` form); a 2nd `meta: none()`/empty `folders` case; + default.
  - `ListLocationsResponse`: `groups: vec![<the populated Volume above>]`; empty-groups case; + default.
  - `GetLocationStatsResponse`: all uint64s set, `failed_files: vec![NetFailedFile{ kind:0, location: buffa::MessageField::some(make_local_location()), error: make_error_info(), error_status:0x01, index_status:0x02, ..Default::default() }]`; an empty-`failed_files` case; + default.
  - `FailedFilesResponse`: `location_id: vec![7,7]`, `failed_files: vec![NetFailedFile{..}]`; an empty case; + default.
  - `SearchRequest`: `query:"beach"`, `pagination: buffa::MessageField::some(Pagination{ limit:20, offset:40, ..Default::default() })`, `filters: vec![SearchFilter{ key:"k".into(), value:"v".into(), weight:0.8, ..Default::default() }]`; a `pagination: none()`/empty-`filters` case; + default.
  - `BrowseResponse`: `items: vec![BrowseItem{ meta: make_video_meta(), location: buffa::MessageField::some(make_local_location()), scene_count:3, thumbnail:vec![1], ..Default::default() }]`, `total_count:1`, `pagination: buffa::MessageField::some(Pagination{ limit:50, offset:0, ..Default::default() })`; an empty case; + default.
  - `GetIndexedFileResponse`: `video` = `buffa::MessageField::some(Video{ meta: make_video_meta(), scenes: vec![vec![1]], index_status:0x01|0x80, index_error: make_error_info(), error_status:1, ..Default::default() })` (reuse the SP2 `Video` field set + `make_video_meta()`), `scenes: vec![Scene{ meta: make_scene_meta(), keyframes: vec![vec![1]], description:"s".into(), shot_type:"wide".into(), camera_motion:"pan".into(), tags:"a".into(), people_count:1, tag_ids: vec![vec![9]], vision_provider: vec!["apple".into()], smart_folders: vec!["f".into()], ..Default::default() }]` (reuse the SP2 `Scene` field set + `make_scene_meta()`); a `video: none()`/empty-`scenes` case; + default.
  - `GetFileIndexingStatsResponse`: `video_id: vec![1]`, `index_status: 0x01|0x04`, `error: make_error_info()`; an `error: none()` case; + default.
  - `GetModelStatusResponse`: `models: vec![ModelInfo{ name:"m".into(), status:2, size_bytes:1024, ..Default::default() }]`; an empty case; + default.
  - `ModelDownloadProgressResponse`: `model: buffa::MessageField::some(ModelDownloadProgress{ name:"m".into(), progress:0.5, downloaded_bytes:1, total_bytes:2, status:1, error_msg:"".into(), ..Default::default() })`; a `model: none()` case; + default.
  - `IndexingProgressResponse`: `location: buffa::MessageField::some(make_local_location())`, uint64s set, `active_files: vec![IndexingFile{ location: buffa::MessageField::some(make_local_location()), name:"a".into(), completed_phases:0x01, ..Default::default() }]`; a `location: none()`/empty case; + default.
  - `#[cfg(feature="json")] batch4_sp3_json_roundtrip` for a populated `GetIndexedFileResponse` (the heaviest cross-package serde case — `findit.db.v1.Video` + repeated `findit.db.v1.Scene`), a populated `ListLocationsResponse` (nested `Volume` → `media.v1.VolumeMeta`/`WatchedLocation`), and a populated `SearchRequest` (nested `Pagination`+`SearchFilter`).

(Reuse the in-file SP1/SP2 builders `make_local_location()`, `make_video_meta()`, `make_scene_meta()`, `make_error_info()` and the SP1 inline `VolumeMeta`/`WatchedLocation` forms. The SP3 `findit.net.v1.FailedFile` is `NetFailedFile` (aliased re-export). Fully expand every case per the confirmed generated field names — do not abbreviate.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch4 composite responses (Volume/ListLocationsResponse/GetLocationStatsResponse/FailedFilesResponse/SearchRequest/BrowseResponse/GetIndexedFileResponse/GetFileIndexingStatsResponse/GetModelStatusResponse/ModelDownloadProgressResponse/IndexingProgressResponse)`.

---

## Task 5: Batch 5 — events

`VolumeStateChangedEvent, FolderUpdatedEvent, ModelDownloadProgressEvent`.

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`event` = bare-`u32` doc-enum → plain `uint32` with the value table verbatim in the comment, §7.3; `ModelDownloadProgressEvent` is the thin wrapper kept as a named **push/event** message referencing the shared batch-2 `ModelDownloadProgress`, §7.8 #4/#10 — distinct type from `ModelDownloadProgressResponse`)

```protobuf
// ── Batch 5: events (§7.5 event.rs) ─────────────────────────────────────────

// `event`: bare u32 enum-by-convention (NOT a proto enum, §7.3) —
// 0=mounted, 1=unmounted, 2=ejected. Out-of-set values round-trip exactly.
message VolumeStateChangedEvent {
  media.v1.VolumeMeta volume = 1;
  uint32 event = 2;
}

// `event`: bare u32 enum-by-convention (NOT a proto enum, §7.3) —
// 0=file_created, 1=file_modified, 2=file_removed. Out-of-set values
// round-trip exactly.
message FolderUpdatedEvent {
  media.v1.Location folder_location = 1;
  string path = 2;
  uint32 event = 3;
}

// Thin wrapper kept as a named message — distinct PUSH/EVENT role (the
// uncorrelated server-push Event arm at discriminant tag 20002); same wire
// shape as ModelDownloadProgressResponse but a separate type per its source
// role; references the shared batch-2 ModelDownloadProgress leaf (NOT a
// re-typed copy). NOT collapsed (§7.8 #4/#10).
message ModelDownloadProgressEvent { ModelDownloadProgress model = 1; }
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `FolderUpdatedEvent, ModelDownloadProgressEvent, VolumeStateChangedEvent`.
- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct VolumeStateChangedEvent\|pub struct FolderUpdatedEvent\|pub struct ModelDownloadProgressEvent\|pub .*:\|MessageField" src/generated/findit.net.v1.network.rs`. Add the 3 idents to the top-level `mediaschema::{…}` import (keep ascending-alphabetical). Add `batch5_sp3_roundtrip`:
  - `VolumeStateChangedEvent`: `volume` = `buffa::MessageField::some(VolumeMeta{ id: buffa::MessageField::some(Id{ value:(1u8..=16).collect(), ..Default::default() }), location: buffa::MessageField::some(make_local_location()), name:"Disk".into(), total_size:1, used_size:0, status:3, ..Default::default() })` (reuse the SP1 `VolumeMeta` form), `event: 2`; AND an out-of-table `event: 99` case with `volume: none()`; + default.
  - `FolderUpdatedEvent`: `folder_location: buffa::MessageField::some(make_local_location())`, `path:"/a/b.mp4".into()`, `event: 1`; AND an out-of-table `event: 200` case with `folder_location: none()`; + default.
  - `ModelDownloadProgressEvent`: `model: buffa::MessageField::some(ModelDownloadProgress{ name:"m".into(), progress:0.9, downloaded_bytes:9, total_bytes:10, status:1, error_msg:"".into(), ..Default::default() })`; a `model: none()` case; + default.
  - `#[cfg(feature="json")] batch5_sp3_json_roundtrip` for a populated `VolumeStateChangedEvent` (covers `media.v1.VolumeMeta` cross-package + the bare-`uint32` event under serde) and a populated `FolderUpdatedEvent` (covers `media.v1.Location` oneof under serde).

(Reuse the in-file SP1 `make_local_location()` + inline `VolumeMeta` form. Fully expand every case per the confirmed generated field names.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch5 events (VolumeStateChangedEvent/FolderUpdatedEvent/ModelDownloadProgressEvent)`.

---

## Task 6: Batch 6 — `Request` oneof envelope (own batch — large; depends on all 14 request messages of batches 1–4)

`message Request { oneof kind { … 14 arms … } }`. Mirrors SP2 `SubtitleTrackOrigin` (Rust enum-with-data → message+oneof). Source `MessageType` discriminant integer of each variant is the arm's tag, verbatim, sparse, NO `reserved` (§7.8 #1). **Every inner message is defined in an earlier batch:** `HeartbeatRequest` (B1); `SearchRequest` (B4); `BrowseRequest`, `GetLocationStatsRequest`, `ListLocationsRequest`, `GetIndexedFileRequest`, `GetFileIndexingStatsRequest`, `GetModelStatusRequest`, `GetDaemonInfoRequest`, `IndexLocationRequest`, `RemoveLocationRequest`, `UpdateAnnotationRequest`, `EjectVolumeRequest`, `RetryFailedRequest` (B3).

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (arm tags = source `MessageType` integers VERBATIM — sparse bands 1 / 500–501 / 1000s / 2000s; NO `reserved` per §7.5/§7.8 #1)

```protobuf
// ── Batch 6: Request oneof envelope (§7.5 mod.rs) ───────────────────────────

// Rust enum-with-data -> proto3 message + single `oneof kind` (owned;
// mirrors SP2 SubtitleTrackOrigin). Each arm's tag is the source
// MessageType discriminant integer, VERBATIM (sparse, deliberately-banded:
// 1 / 500–501 / 1000s / 2000s) — emitted with NO `reserved` per §2's
// wide-band rule and the SP2 §6.8 #3 AudioAnalysis/TrackRecord precedent.
message Request {
  oneof kind {
    HeartbeatRequest heartbeat = 1;
    SearchRequest search = 500;
    BrowseRequest browse = 501;
    GetLocationStatsRequest get_location_stats = 1000;
    ListLocationsRequest list_locations = 1001;
    GetIndexedFileRequest get_indexed_file = 1002;
    GetFileIndexingStatsRequest get_file_indexing_stats = 1003;
    GetModelStatusRequest get_model_status = 1004;
    GetDaemonInfoRequest get_daemon_info = 1007;
    IndexLocationRequest index_location = 2000;
    RemoveLocationRequest remove_location = 2001;
    UpdateAnnotationRequest update_annotation = 2002;
    EjectVolumeRequest eject_volume = 2003;
    RetryFailedRequest retry_failed = 2005;
  }
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`. Confirm `findit.net.v1.network.__oneof.rs` now appears (the `Request.kind` oneof) and the `findit.net.v1.mod.rs` stitcher now `include!`s it under `__buffa::oneof`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `Request`. Then add the oneof companion alias immediately after the brace block (mirroring SP2's `subtitle_track_origin::Source as SubtitleTrackOriginSource`):

```rust
/// Oneof variant for [`Request`]: one of the 14 request arms
/// (`Kind::Heartbeat(…)` … `Kind::RetryFailed(…)`; proto `oneof kind`).
pub use generated::findit::net::v1::request::Kind as RequestKind;
```

(Confirm the generated submodule path `request` and oneof enum name `Kind` from `grep -n "pub mod request\|pub enum Kind" src/generated/findit.net.v1.network*.rs` before finalizing the `as` target — buffa names the per-message oneof submodule by the message snake_case and the oneof enum `Kind` for a `oneof kind`, exactly as the SP1 `media_kind::Kind`/`location::Kind` and SP2 `subtitle_track_origin::Source` precedents.)

- [ ] **Step 4: Tests** — confirm the oneof shape: `grep -n "pub struct Request\b\|pub mod request\|pub enum Kind\|MessageField\|Box<" src/generated/findit.net.v1.network*.rs`. Add `Request, RequestKind` to the top-level `mediaschema::{…}` import (keep ascending-alphabetical). Add `batch6_sp3_roundtrip`:
  - One `rt(&Request { kind: Some(RequestKind::<Arm>(...)), ..Default::default() })` per arm — exercise **every** of the 14 arms (each inner message constructed minimally-but-populated reusing the batch-1/3/4 builders/forms; e.g. `RequestKind::Heartbeat(HeartbeatRequest{ timestamp:1, ..Default::default() })`, `RequestKind::Search(SearchRequest{ query:"q".into(), pagination: buffa::MessageField::none(), filters: vec![], ..Default::default() })`, `RequestKind::ListLocations(ListLocationsRequest::default())`, `RequestKind::IndexLocation(IndexLocationRequest{ target: buffa::MessageField::none(), ..Default::default() })`, … one per variant, covering the full tag band 1/500/501/1000s/2000s). If the grep shows arms are `Box`ed, wrap the inner value in `Box::new(...)` (mirrors the SP1 `LocationKind::Local(Box::new(Local{..}))` precedent).
  - `rt(&Request::default())` — the no-arm-set default (proves the empty oneof round-trips).
  - A couple of representative arms also exercised with their inner message *fully* populated (e.g. `RequestKind::Search(<the fully-populated SearchRequest from batch 4>)`, `RequestKind::UpdateAnnotation(UpdateAnnotationRequest{ scene_ids: vec![vec![1]], user_tags: vec![Tag{ name:"f".into(), color:1, ..Default::default() }], ..Default::default() })`).
  - `#[cfg(feature="json")] batch6_sp3_json_roundtrip` for a populated `RequestKind::Search(...)` arm and the high-tag `RequestKind::RetryFailed(RetryFailedRequest{ location: buffa::MessageField::some(make_local_location()), ..Default::default() })` arm (tag 2005 — proves the sparse high arm-tag survives serde).

(Reuse the in-file batch-1/3/4 builders/forms and `make_local_location()`. Fully expand all 14 arm cases — do not abbreviate; every arm in the proto block above must have its own `rt(...)` line.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch6 Request oneof envelope (14 arms, source MessageType tags verbatim)`.

---

## Task 7: Batch 7 — `Response` oneof envelope (own batch — large; depends on all response messages of batches 1–5 + `media.v1.ErrorInfo`)

`message Response { oneof kind { … 15 arms; the `ErrorResponse` arm → `media.v1.ErrorInfo` at tag 20000 (remapped from reserved 19999, §7.8 #12; collapse, §7.8 #4 — NO `findit.net.v1.ErrorResponse`) … } }`. **Every inner message is defined in an earlier batch (or is the SP1 `media.v1.ErrorInfo`):** `HeartbeatResponse` (B1); `SearchResponse`, `GetDaemonInfoResponse`, `IndexLocationResponse`, `RemoveLocationResponse`, `UpdateAnnotationResponse`, `EjectVolumeResponse`, `RetryFailedResponse` (B3); `BrowseResponse`, `GetIndexedFileResponse`, `ListLocationsResponse`, `GetLocationStatsResponse`, `GetFileIndexingStatsResponse`, `GetModelStatusResponse` (B4); `ErrorResponse` arm → `media.v1.ErrorInfo` (SP1).

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (15 arms; arm tags = source `MessageType` integers VERBATIM — bands 10000s / 12000s; the error arm at tag 20000 (remapped from reserved 19999, §7.8); the error arm references `media.v1.ErrorInfo` directly per §7.8 #4; NO `reserved` per §7.5/§7.8 #1)

```protobuf
// ── Batch 7: Response oneof envelope (§7.5 mod.rs) ──────────────────────────

// Rust enum-with-data -> proto3 message + single `oneof kind` (owned;
// mirrors SP2 SubtitleTrackOrigin). 15 arms (§7.8 #1: mod.rs defines 15,
// incl. ErrorResponse whose source discriminant is 19999). Each arm's tag
// is the source MessageType discriminant integer, VERBATIM (sparse bands
// 10000s / 12000s) — NO `reserved` per §2's wide-band rule / SP2 §6.8 #3
// precedent. SINGLE EXCEPTION: the `error` arm is remapped from source
// MessageType::ErrorResponse=19999 to tag 20000 because protobuf reserves
// field numbers 19000–19999 (protoc rejects 19999); per §2 NO-wire-compat
// the source discriminant is non-binding (§7.8 #12). The `error` arm
// references media.v1.ErrorInfo DIRECTLY (the source thin
// error.rs::ErrorResponse {error:ErrorInfo} wrapper is collapsed, §7.8 #4 —
// NO findit.net.v1.ErrorResponse type).
message Response {
  oneof kind {
    HeartbeatResponse heartbeat = 10001;
    SearchResponse search = 10500;
    BrowseResponse browse = 10501;
    GetIndexedFileResponse get_indexed_file = 11002;
    ListLocationsResponse list_locations = 11003;
    GetLocationStatsResponse get_location_stats = 11004;
    GetFileIndexingStatsResponse get_file_indexing_stats = 11005;
    GetModelStatusResponse get_model_status = 11006;
    GetDaemonInfoResponse get_daemon_info = 11009;
    IndexLocationResponse index_location = 12000;
    RemoveLocationResponse remove_location = 12001;
    UpdateAnnotationResponse update_annotation = 12003;
    EjectVolumeResponse eject_volume = 12004;
    RetryFailedResponse retry_failed = 12006;
    media.v1.ErrorInfo error = 20000;  // remapped from reserved 19999 (§7.8 #12)
  }
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`. Confirm the `Response.kind` oneof is added to `findit.net.v1.network.__oneof.rs` and stitched in `findit.net.v1.mod.rs`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `Response`. Then add the oneof companion alias immediately after the brace block:

```rust
/// Oneof variant for [`Response`]: one of the 15 response arms
/// (`Kind::Heartbeat(…)` … `Kind::Error(media.v1.ErrorInfo)`; proto
/// `oneof kind`). The `Error` arm carries `media.v1.ErrorInfo` directly
/// (the source thin `ErrorResponse` wrapper is collapsed, spec §7.8 #4).
pub use generated::findit::net::v1::response::Kind as ResponseKind;
```

(Confirm `pub mod response` + `pub enum Kind` from `grep -n "pub mod response\|pub enum Kind" src/generated/findit.net.v1.network*.rs`. The `error` arm's inner type is the cross-package `media.v1.ErrorInfo` — confirm the generated arm payload type from the grep; it will be the same `mediaschema::ErrorInfo` re-export, possibly `Box`ed.)

- [ ] **Step 4: Tests** — confirm the oneof shape: `grep -n "pub struct Response\b\|pub mod response\|pub enum Kind\|MessageField\|Box<" src/generated/findit.net.v1.network*.rs`. Add `Response, ResponseKind` to the top-level `mediaschema::{…}` import (keep ascending-alphabetical; `ErrorInfo` is already in-file). Add `batch7_sp3_roundtrip`:
  - One `rt(&Response { kind: Some(ResponseKind::<Arm>(...)), ..Default::default() })` per arm — exercise **all 15 arms**, including the error arm `ResponseKind::Error(ErrorInfo{ code:5, message:"e".into(), ..Default::default() })` at tag 20000 (remapped from reserved 19999, §7.8 #12; this is also the §7.8 #4 collapse — assert the error round-trips through `media.v1.ErrorInfo` directly, with NO `findit.net.v1.ErrorResponse`). Inner messages constructed minimally-but-populated reusing the batch-1/3/4 builders; empty-response arms use `<Resp>::default()` (e.g. `ResponseKind::RemoveLocation(RemoveLocationResponse::default())`). If arms are `Box`ed, wrap accordingly.
  - `rt(&Response::default())` — no-arm default.
  - A couple representative arms with the inner message fully populated (e.g. `ResponseKind::GetIndexedFile(<the fully-populated GetIndexedFileResponse from batch 4>)`, `ResponseKind::Error(ErrorInfo{ code:404, message:"not found".into(), ..Default::default() })`).
  - `#[cfg(feature="json")] batch7_sp3_json_roundtrip` for a populated `ResponseKind::GetIndexedFile(...)` arm (heaviest cross-package serde) and the `ResponseKind::Error(ErrorInfo{..})` arm at tag 20000 (remapped from reserved 19999, §7.8 #12 — proves the collapsed error arm + the remapped sparse arm-tag survive serde).

(Reuse the in-file batch-1/3/4 builders + `make_video_meta`/`make_scene_meta`/`make_local_location`/`make_error_info`. Fully expand all 15 arm cases — do not abbreviate.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch7 Response oneof envelope (15 arms incl. ErrorResponse->media.v1.ErrorInfo collapse @20000 (remapped from reserved 19999))`.

---

## Task 8: Batch 8 — `Event` oneof envelope (own batch; depends on `IndexingProgressResponse`/`FailedFilesResponse` + the 3 event messages)

`message Event { oneof kind { … 5 arms … } }`. **Every inner message is defined in an earlier batch:** `IndexingProgressEvent` → `IndexingProgressResponse` (B4); `FailedFilesEvent` → `FailedFilesResponse` (B4); `VolumeStateChangedEvent`, `FolderUpdatedEvent`, `ModelDownloadProgressEvent` (B5). No envelope wraps `Event` (uncorrelated push, §7.8 #3 — handled in batch 9 by `Event` having no correlation envelope).

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (arm tags = source `MessageType` integers VERBATIM — 13000 / 12007 / 20000s; note the inner type for `IndexingProgressEvent` is `IndexingProgressResponse` and for `FailedFilesEvent` is `FailedFilesResponse` — §7.5; NO `reserved` per §7.8 #1)

```protobuf
// ── Batch 8: Event oneof envelope (§7.5 mod.rs) ─────────────────────────────

// Rust enum-with-data -> proto3 message + single `oneof kind` (owned;
// mirrors SP2 SubtitleTrackOrigin). 5 arms. Each arm's tag is the source
// MessageType discriminant integer, VERBATIM (sparse: 13000 / 12007 /
// 20000–20002) — NO `reserved` per §2's wide-band rule / SP2 §6.8 #3
// precedent. `progress`/`failed_files` arms reuse the batch-4
// IndexingProgressResponse / FailedFilesResponse messages (§7.5). Event has
// NO correlation envelope (uncorrelated server-push, §7.8 #3).
message Event {
  oneof kind {
    FailedFilesResponse failed_files = 12007;
    IndexingProgressResponse indexing_progress = 13000;
    VolumeStateChangedEvent volume_state_changed = 20000;
    FolderUpdatedEvent folder_updated = 20001;
    ModelDownloadProgressEvent model_download_progress = 20002;
  }
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`. Confirm the `Event.kind` oneof is added to `findit.net.v1.network.__oneof.rs` and stitched.
- [ ] **Step 3: Re-exports** — extend the brace list with: `Event`. Then add the oneof companion alias immediately after the brace block:

```rust
/// Oneof variant for [`Event`]: one of the 5 server-push event arms
/// (`Kind::FailedFiles(…)` … `Kind::ModelDownloadProgress(…)`; proto
/// `oneof kind`). `Event` has NO correlation envelope (uncorrelated push,
/// spec §7.8 #3).
pub use generated::findit::net::v1::event::Kind as EventKind;
```

(Confirm `pub mod event` + `pub enum Kind` from `grep -n "pub mod event\b\|pub enum Kind" src/generated/findit.net.v1.network*.rs`.)

- [ ] **Step 4: Tests** — confirm the oneof shape: `grep -n "pub struct Event\b\|pub mod event\b\|pub enum Kind\|MessageField\|Box<" src/generated/findit.net.v1.network*.rs`. Add `Event, EventKind` to the top-level `mediaschema::{…}` import (keep ascending-alphabetical). Add `batch8_sp3_roundtrip`:
  - One `rt(&Event { kind: Some(EventKind::<Arm>(...)), ..Default::default() })` per arm — all 5: `EventKind::FailedFiles(FailedFilesResponse{ location_id: vec![1], failed_files: vec![NetFailedFile{ kind:0, location: buffa::MessageField::some(make_local_location()), error: make_error_info(), error_status:0x01, index_status:0x02, ..Default::default() }], ..Default::default() })`, `EventKind::IndexingProgress(IndexingProgressResponse{ location: buffa::MessageField::some(make_local_location()), total_files:10, indexed_files:5, active_files: vec![IndexingFile{ location: buffa::MessageField::some(make_local_location()), name:"a".into(), completed_phases:0x01, ..Default::default() }], ..Default::default() })`, `EventKind::VolumeStateChanged(VolumeStateChangedEvent{ volume: buffa::MessageField::none(), event:2, ..Default::default() })`, `EventKind::FolderUpdated(FolderUpdatedEvent{ folder_location: buffa::MessageField::some(make_local_location()), path:"/x".into(), event:1, ..Default::default() })`, `EventKind::ModelDownloadProgress(ModelDownloadProgressEvent{ model: buffa::MessageField::some(ModelDownloadProgress{ name:"m".into(), progress:0.5, downloaded_bytes:1, total_bytes:2, status:1, error_msg:"".into(), ..Default::default() }), ..Default::default() })`. If arms are `Box`ed, wrap accordingly.
  - `rt(&Event::default())` — no-arm default.
  - `#[cfg(feature="json")] batch8_sp3_json_roundtrip` for a populated `EventKind::IndexingProgress(...)` arm (covers nested `media.v1.Location` + repeated `IndexingFile` under serde) and the `EventKind::ModelDownloadProgress(...)` arm at tag 20002 (proves the highest event arm-tag survives serde).

(Reuse the in-file batch-2/4/5 builders/forms + `make_local_location()`/`make_error_info()`. The SP3 `findit.net.v1.FailedFile` is `NetFailedFile`. Fully expand all 5 arm cases.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch8 Event oneof envelope (5 arms, source MessageType tags verbatim)`.

---

## Task 9: Batch 9 — correlation envelopes (LAST — depend on `Request`+`Response`)

`RequestEnvelope { uint64 request_id=1; Request request=2 }`, `ResponseEnvelope { uint64 request_id=1; Response response=2 }`. `Payload<T>` monomorphized into these two concrete messages (proto3 has no generics, §7.8 #3). `request_id` = source `RequestId` u64 newtype inlined as `uint64` (§7.2/§7.8 #3); `request_id=0` ⇒ no-correlation default (matches source `RequestId::from_raw(0)` Default + the `id.value()!=0` encode guard). `Request` is defined in batch 6, `Response` in batch 7 — both earlier. `Event` has NO envelope (uncorrelated push, §7.8 #3) — nothing emitted for it here.

**Files:** modify `proto/findit/net/v1/network.proto`, `src/lib.rs`, `tests/roundtrip.rs`; regenerate.

- [ ] **Step 1: Append proto** (`request_id` plain `uint64` — the `RequestId` newtype inlined, NOT `optional`; `request_id=0` = no-correlation, documented in the comment per §7.8 #3; `request`/`response` reference the batch-6/7 oneof envelope messages)

```protobuf
// ── Batch 9: correlation envelopes (§7.5 mod.rs; §7.8 #3) ───────────────────

// `Payload<Request>` monomorphized (proto3 has no generics, §7.8 #3).
// `request_id`: the src RequestId u64 newtype inlined as uint64 (§7.2);
// request_id=0 => no-correlation default (matches the source
// RequestId::from_raw(0) Default and the `id.value()!=0` encode guard) —
// plain uint64, NOT `optional`. `request`: the batch-6 Request oneof
// envelope.
message RequestEnvelope {
  uint64 request_id = 1;
  Request request = 2;
}

// `Payload<Response>` monomorphized. Same request_id convention. The source
// RequestMessage=Payload<Request> / ResponseMessage=Payload<Response>
// aliases are NOT separate types. `response`: the batch-7 Response oneof
// envelope. (Event has NO envelope — uncorrelated push, §7.8 #3.)
message ResponseEnvelope {
  uint64 request_id = 1;
  Response response = 2;
}
```

- [ ] **Step 2: Regenerate** — `cargo run -p xtask -- gen`.
- [ ] **Step 3: Re-exports** — extend the brace list with: `RequestEnvelope, ResponseEnvelope`. (No new oneof companion — these are plain messages whose `request`/`response` fields nest the already-aliased `Request`/`Response` messages.)
- [ ] **Step 4: Tests** — confirm fields/wrappers: `grep -n "pub struct RequestEnvelope\|pub struct ResponseEnvelope\|pub .*:\|MessageField" src/generated/findit.net.v1.network.rs`. Add `RequestEnvelope, ResponseEnvelope` to the top-level `mediaschema::{…}` import (keep ascending-alphabetical; `Request, RequestKind, Response, ResponseKind` are already in-file from B6/B7). Add `batch9_sp3_roundtrip`:
  - `RequestEnvelope`: `request_id: 42`, `request: buffa::MessageField::some(Request{ kind: Some(RequestKind::Heartbeat(HeartbeatRequest{ timestamp:1, ..Default::default() })), ..Default::default() })`; a 2nd case `request_id: 0` (the no-correlation default — proves `request_id=0` round-trips exactly, NOT dropped) with a high-tag arm `RequestKind::RetryFailed(RetryFailedRequest{ location: buffa::MessageField::some(make_local_location()), ..Default::default() })`; a 3rd case `request: buffa::MessageField::none()` (no inner Request); + `RequestEnvelope::default()`.
  - `ResponseEnvelope`: `request_id: 7`, `response: buffa::MessageField::some(Response{ kind: Some(ResponseKind::Heartbeat(HeartbeatResponse{ timestamp:1, ..Default::default() })), ..Default::default() })`; a 2nd case `request_id: 0` with the error arm `ResponseKind::Error(ErrorInfo{ code:5, message:"e".into(), ..Default::default() })` (proves the collapsed error arm survives inside the envelope, and request_id=0 round-trips); a 3rd case `response: buffa::MessageField::none()`; + default.
  - `#[cfg(feature="json")] batch9_sp3_json_roundtrip` for a populated `RequestEnvelope` (covers nested `Request` oneof under serde) and a populated `ResponseEnvelope` carrying the `ResponseKind::Error(ErrorInfo{..})` arm (covers the nested collapsed-error oneof under serde) — both with `request_id` set non-zero AND a `request_id: 0` JSON case to prove the zero default round-trips through serde.

(Reuse the in-file batch-6/7 oneof aliases `RequestKind`/`ResponseKind`, `make_local_location()`, and `ErrorInfo`. If the `request`/`response` fields are not bare `buffa::MessageField<Request>`/`<Response>` per the grep, adjust. Fully expand every case per the confirmed generated field names.)

- [ ] **Step 5: Verify** (Conventions block).
- [ ] **Step 6: Commit** — `feat(sp3): batch9 correlation envelopes (RequestEnvelope/ResponseEnvelope; Payload<T> monomorphized, request_id=0 default)`.

---

## Task 10: SP3 finalize — DoD + CI confirm + stacked PR

**Files:** possibly `.github/workflows/codegen.yml` (only if needed); branch finish.

- [ ] **Step 1: Full SP3 Definition-of-Done check** (§7.9 — run, all must pass):
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
All SP0 + SP1 + SP2 + SP3 tests green; `GEN_CLEAN`; no `mediatime.v1.*` / `*__view*` files in `src/generated`. Spot-check the §7.9 invariants: `grep -rn "ErrorResponse" src/generated/findit.net.v1.network.rs` returns NOTHING (no `findit.net.v1.ErrorResponse`); `grep -n "pub enum " src/generated/findit.net.v1.network.rs` returns ONLY the oneof `Kind` enums (`request::Kind`/`response::Kind`/`event::Kind`) — NO domain proto `enum` (SP3 owns zero enums); `grep -rn "Header\|MessageType\|MessageFlags\|RequestId\|framing" src/generated/findit.net.v1.network.rs` returns NOTHING.

- [ ] **Step 2:** Confirm `.github/workflows/codegen.yml` covers SP3. The existing `gen-is-committed` job runs `cargo run -p xtask -- gen` + `git diff --exit-code -- src/generated` (now also covers the new `findit.net.v1.*` files — no change needed; the xtask `.files()` third-entry edit is part of the committed tree). The `test-matrix` job runs `cargo test` / `--no-default-features` / `--all-features` and (added by SP1/SP2) `cargo test --features quickcheck,json`. SP3's `#[cfg(feature="json")]` tests are exercised by `--all-features`/`quickcheck,json`; the protoc-34.0 setup step is already present. No CI change expected — if the `cargo test --features quickcheck,json` step is somehow absent from `test-matrix`, add `- run: cargo test --features quickcheck,json`. No other CI change.

- [ ] **Step 3: Open stacked PR** `sp3-network` → `sp2-database`:
```
git push -u origin sp3-network
gh pr create --repo Findit-AI/mediaschema --base sp2-database --head sp3-network \
  --title "SP3: migrate findit-proto network/ → findit.net.v1 (stacked on sp2-database)" \
  --body "<summary: Task 0 three-way codegen scaffolding (cross-package media.v1 + findit.db.v1 refs + mediatime extern, single-compile, no media.v1/findit.db.v1 extern) + 9 batches = 35 owned messages, 0 owned enums; the 3 oneof envelopes Request/Response/Event with source MessageType discriminant tags verbatim (no reserved); ErrorResponse collapsed to media.v1.ErrorInfo; Payload<T> -> RequestEnvelope/ResponseEnvelope; Header/MessageType/MessageFlags/RequestId/framing/*Ref/*Chunk excluded; DoD evidence; reuse/exclusion decisions per spec §7; stacked on sp2-database>

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```

---

## Self-Review

**1. Spec §7 coverage — every message/batch maps to a task.**
- §7.3 — **0 owned enums** (SP3 owns none; the 5 candidate "enums" are inline `uint32` with documented value tables — materialized at use sites in B2/B5). ✓ (no enum task exists, correctly).
- §7.5 — **all 43 §7.5 non-`mod.rs` message rows placed in exactly one task (B1–B5), plus the 5 §7.5 `mod.rs` messages in B6–B9 = 48 owned messages**, set-verified by a row-by-row diff of the §7.5 tables against the `message <Name> {` blocks in plan batches B1–B5 (zero §7.5 rows unplaced; zero invented messages):
  - B1 (4): `Pagination, SearchFilter, HeartbeatRequest, HeartbeatResponse`.
  - B2 (6): `SearchHit, BrowseItem, ModelInfo, ModelDownloadProgress, FailedFile, IndexingFile`.
  - B3 (19): `ListLocationsRequest, RemoveLocationResponse, RetryFailedResponse, EjectVolumeResponse, GetModelStatusRequest, GetDaemonInfoRequest, UpdateAnnotationResponse, EjectVolumeRequest, GetIndexedFileRequest, GetFileIndexingStatsRequest, GetLocationStatsRequest, RemoveLocationRequest, RetryFailedRequest, BrowseRequest, IndexLocationRequest, IndexLocationResponse, UpdateAnnotationRequest, SearchResponse, GetDaemonInfoResponse`.
  - B4 (11): `Volume, ListLocationsResponse, GetLocationStatsResponse, FailedFilesResponse, SearchRequest, BrowseResponse, GetIndexedFileResponse, GetFileIndexingStatsResponse, GetModelStatusResponse, ModelDownloadProgressResponse, IndexingProgressResponse`.
  - B5 (3): `VolumeStateChangedEvent, FolderUpdatedEvent, ModelDownloadProgressEvent`.
  - B6 (1): `Request`. B7 (1): `Response`. B8 (1): `Event`. B9 (2): `RequestEnvelope, ResponseEnvelope`.
  - 4+6+19+11+3 = **43** (= the exact §7.5 non-`mod.rs` row count) + 5 `mod.rs` = **48 owned `findit.net.v1` messages**. `Sp3CodegenSmoke` (Task 0) is the additional permanent cross-package regression-guard fixture (mirrors SP2's `Sp2CodegenSmoke`, which §6.6 likewise excluded from its "42 owned messages" count). See "Ambiguity resolved" for the §7.6-totals-line discrepancy.
- §7.6 batch order honored exactly: Task 0 scaffolding first; then scalar leaves → reuse-only leaves → empty/simple req-resp leaves → composite responses → events → `Request` oneof (own batch) → `Response` oneof (own batch) → `Event` oneof (own batch) → correlation envelopes LAST. ✓
- §7.7 exclusions are absent: NO `findit.net.v1` message for `Header`/`MessageType`/`MessageFlags`/`RequestId`/`Payload`/`error.rs::ErrorResponse`/`framing`/`async_framing`/any `*Ref`/`*Chunk`/`RequestMessage`/`ResponseMessage` alias; the 5 bare-`u32` "enums-by-convention" emit NO proto enum (inline `uint32`); `VideoIndexStatus` emits NO SP3 type (inline `uint32`); `media.v1.*`/`findit.db.v1.*`/`mediatime.v1.*` referenced not redefined. The proto blocks contain ZERO of these as messages/enums; Task 10 Step 1 adds explicit `grep` guards for `ErrorResponse`/`pub enum `/`Header|MessageType|MessageFlags|RequestId|framing`. ✓
- §7.8 ambiguity resolutions all baked in: #1 `Request`/`Response`(15 arms)/`Event` → message+oneof, source `MessageType` tags verbatim, NO `reserved` (B6/B7/B8 proto comments + arm tags) except the single `Response.error` arm (19999→20000, protobuf-reserved, §7.8 #12); #2/#6 the 5 bare-`u32` doc-"enums" → plain `uint32` with the value table verbatim in the field comment, NO proto enum, NO `*_UNSPECIFIED` (B2 `ModelInfo`/`ModelDownloadProgress`/`FailedFile`, B5 `VolumeStateChangedEvent`/`FolderUpdatedEvent`; tests include out-of-table-value cases proving exact round-trip); #3 `Payload<T>` → `RequestEnvelope`/`ResponseEnvelope`, `request_id` plain `uint64` not `optional`, `request_id=0` no-correlation default, `Event` no envelope (B9 + comment + the `request_id:0` test cases); #4 `error.rs::ErrorResponse` collapsed → `Response` error arm = `media.v1.ErrorInfo` @20000 (remapped from reserved 19999, §7.8 #12), NO `findit.net.v1.ErrorResponse` (B7 proto + Task 10 grep guard); #5 transport plumbing excluded (Conventions + §7.7 + Task 10 grep guard); #7 Task 0 three-way scaffolding (the explicit risk front-load); #8 `SearchHit.dimensions` → `media.v1.Dimensions` split, no SP3 redefine (B2 proto comment); #9 `Pagination.limit=50`/`SearchFilter.weight=1.0` → plain scalar not `optional`, client default in comment (B1 proto comment + the wire-default test note); #10 `ModelDownloadProgressResponse` (B4) vs `ModelDownloadProgressEvent` (B5) both kept as named messages referencing the shared B2 `ModelDownloadProgress`, NOT collapsed (B4/B5 proto comments). ✓

**2. Three-way cross-package codegen front-loaded.** Task 0 creates the new package file + the exact one-line xtask `.files()` *third-entry* edit (verified against buffa-build 0.6.0 + buffa-codegen 0.6.0 AND the already-merged SP2 two-way precedent: single `compile()` over all three protos → native `super::`-rooted cross-package refs via `context::TypePath`; neither `media.v1` nor `findit.db.v1` is extern; `mediatime.v1` stays extern, not generated), the SP3 re-export block, the smoke message exercising a `media.v1` message + a `findit.db.v1` message + inline bytes + `mediatime` extern, and a full feature-matrix + drift-gate + round-trip + JSON verification BEFORE any content batch. Exact expected `src/generated/` filenames stated (`findit.net.v1.mod.rs`, `findit.net.v1.network.rs`, `findit.net.v1.network.__oneof.rs` once B6 lands; `mediatime.v1` absent). The shared-`findit`-parent-module subtlety (SP2's `pub mod db` already exists; SP3 *adds* sibling `pub mod net`, the wrapper is regenerated not duplicated) is called out explicitly in the mechanism section and Task 0 Step 3. ✓

**3. Placeholder scan.** Every batch's proto block is transcribed in full from §7.5's mapping tables (every `proto_type proto_name = tag;`, every `oneof kind { … }` arm with its exact source-`MessageType` tag number, the bare-`u32` value-table comments, the `VideoIndexStatus` bit-listing comments, the client-default comments, the collapse/extern intent comments) — NO "see §7", NO "TBD", NO "handle edge cases". Re-export idents are listed explicitly per batch; the two aliased re-exports (`NetFailedFile`, and the three `RequestKind`/`ResponseKind`/`EventKind` oneof companions) are spelled out with their exact `pub use generated::findit::net::v1::…` paths mirroring the SP2 `DbMediaKind`/`SubtitleTrackOriginSource` precedent. Test steps give the reusable `rt` recipe + a mandatory "confirm generated field names from src/generated" grep + concrete `batchN_sp3_roundtrip`/`#[cfg(feature="json")]` shapes (Task 0 + batch 1 fully expanded as code; batches 2–9 give the exhaustive per-field/per-arm construction recipe + reuse the established in-file SP1/SP2 helpers — the same density as the SP2 plan's later batches). Empty messages have the explicit "populated == default" note. No structural placeholders. ✓

**4. Type-name consistency & oneof-arm dependency ordering.** Message names are identical across proto blocks, re-export lists, and test descriptions in every task. `media.v1.*`/`findit.db.v1.*` always written as the proto FQN in proto and as the `mediaschema::<Type>` re-export (or in-file SP2 builder) in tests; `mediatime.v1.*` always `mediatime.v1.*` in proto / `::mediatime::*` in tests. The single message-name collision (`findit.net.v1.FailedFile` vs SP2 `findit.db.v1.FailedFile`) is resolved once via the `NetFailedFile` aliased re-export (Task 2 Step 3) and used consistently as `NetFailedFile` in Tasks 4/8 tests — exactly mirroring the SP2 `DbMediaKind` precedent. The `rt` + `make_*` helpers (`make_local_location`, `make_video_meta`, `make_scene_meta`, `make_error_info`, `make_detection`, `make_bbox`, `make_subject`, `sp2_track_time_one`) are reused from the already-executed SP1/SP2 `tests/roundtrip.rs` (verified present), never redefined. **Request/Response/Event arm inner-message dependency ordering verified:** every arm's inner `findit.net.v1` message in `Request` (B6) is defined in B1/B3/B4 (all < 6); every arm in `Response` (B7) is in B1/B3/B4 or is the SP1 `media.v1.ErrorInfo` (all < 7); every arm in `Event` (B8) is in B4/B5 (all < 8) — note the `IndexingProgressEvent`→`IndexingProgressResponse` and `FailedFilesEvent`→`FailedFilesResponse` arm-name-vs-inner-type indirection is transcribed exactly from §7.5; `RequestEnvelope`/`ResponseEnvelope` (B9) reference `Request`(B6)/`Response`(B7) (both < 9). NO oneof references a message defined in its own or a later batch. ✓

**Ambiguity resolved (no TBDs) — the §7.6 message-count discrepancy.** §7.6's totals line states "**35 owned messages** (30 leaf/request/response/event messages from the survey + `Request` + `Response` + `Event` + `RequestEnvelope` + `ResponseEnvelope`)". This is **internally inconsistent with §7.5, the spec's own authoritative field-level mapping**: a literal row count of §7.5's three non-`mod.rs` tables (`shared.rs` 8 rows; `folder.rs` 12 rows; the aggregated `heartbeat/volume_eject/file/daemon/search/browse/model/progress/annotation` table 20 rows; `event.rs` 3 rows) = **43** distinct message rows, not 30 — every request and its paired response is a separate `proto_type proto_name = tag;` row with its own fields (e.g. `HeartbeatRequest`/`HeartbeatResponse`, `EjectVolumeRequest`/`EjectVolumeResponse` are two messages each, not one). §7.6's "30" is the directive's *pre-survey estimate* that §7.5's actual survey superseded; the §7.6 totals prose sentence was not updated to match its own table (structurally the same situation as §6.8 #1, where SP2's inventory "~40" was corrected to 42 by survey — here the correction is "30 → 43" and it landed in §7.5 but not in the §7.6 summary sentence). **Resolution:** the **authoritative anchor is §7.5** (the per-field, per-tag mapping the implementer literally transcribes), NOT the §7.6 summary sentence. The plan emits **every §7.5 row exactly once**: all **43** non-`mod.rs` §7.5 rows across B1–B5 + the **5** `mod.rs` messages across B6–B9 = **48 owned `findit.net.v1` messages** (plus the Task-0 `Sp3CodegenSmoke` scaffolding fixture, excluded from the count exactly as §6.6 excluded `Sp2CodegenSmoke` from SP2's "42"). This was **set-verified mechanically while authoring this plan**: a sorted diff of the §7.5 non-`mod.rs` table names against the `message <Name> {` blocks in plan batches B1–B5 produced **zero names in §7.5 but not in the plan, and zero messages in the plan but not in §7.5** — an exact bijection (no row unplaced, no message invented). Rationale for following §7.5 over §7.6's prose: a closed proto schema must materialize each surveyed message — each carries distinct fields AND a distinct `MessageType` discriminant used as a `Request`/`Response`/`Event` oneof arm tag — so dropping the "missing 13" would lose real wire types and break the §7.5 oneof arm tables (which reference, e.g., both `HeartbeatRequest`@1 and `HeartbeatResponse`@10001 as separate inner messages). No message is added or omitted versus §7.5; the only documentary effect is that this plan states the corrected count (48 = 43 §7.5 + 5 `mod.rs`) and pins §7.5 as the anchor. Mirrors the SP2 plan's handling of its own §6.6 "≈40 → 42" survey discrepancy (documented, anchored to the §6.5 field mapping, no functional change).
