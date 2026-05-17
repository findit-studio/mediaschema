# mediaschema ← findit-proto via buffa — Migration Context & SP0 (Foundation) Design

- **Date:** 2026-05-17
- **Status:** Approved (brainstorming) — pending written-spec review
- **Repos in scope:** `mediaschema` (primary), `mediatime`, `buffa` (capabilities only; no buffa changes in SP0)
- **Source-of-truth repo being migrated:** `findit-studio/indexer/findit-proto`

---

## 1. Context & Problem

`findit-proto` is a hand-written proto3-compatible serialization library: ~47k LOC across ~120
message/enum types, with a manual wire codec (`message.rs` + `message/*`, varint via `varing`,
chunked buffers via `bufkit`). Field numbers and wire types are embedded as Rust constants
(`Identifier::new(WireType::…, FieldTag::new(N))`); **there are no `.proto` files**. It is a
member of the `indexer` Cargo workspace and is depended on by ~13 crates.

We are migrating the **schema types** out of `findit-proto` into a standalone `mediaschema`
crate, replacing the hand-rolled codec with **buffa**-generated code (buffa = pure-Rust,
editions-first protobuf; codegen from `.proto` via `protoc`/`buf` + `buffa-build`).

This document records the cross-cutting decisions, the decomposition of the overall migration,
and the full design of the first sub-project (**SP0 — Foundation**). SP1–SP3 each get their
own spec → plan → implement cycle.

## 2. Decisions Locked (apply to the whole migration)

1. **Source of truth = the actual Rust code** in `findit-proto` (structs, enums, oneofs,
   field semantics). Not docs, not wire tags.
2. **No byte-for-byte wire compatibility.** Requirement is *correctness for transmission*:
   round-trip fidelity and semantic equivalence, not interoperability with data already
   encoded by the hand-rolled codec. `.proto` is authored clean (idiomatic, numbered from 1).
3. **redb / storage-key concerns are out of scope** for this phase. Only the protobuf
   *message* types are migrated.
4. **Required derivable capabilities on migrated types:** `serde`, `arbitrary`, `quickcheck`.
5. **mediatime types are reused, not regenerated.** `Timebase`, `TimeRange`, `Timestamp`
   stay as `mediatime` types via buffa `extern_path`; a new `mediatime/buffa` feature
   implements the buffa contract for them.
6. **Generated code is checked in** (reviewed diffs; no `protoc`/`buf` at consumer build time).
7. **Proto layout is multi-package, mirroring the domains** (`findit.common`,
   `findit.database`, `findit.audio`, `findit.network`).
8. **Consumer cutover (~13 crates) is deferred.** buffa's generated API differs from the
   hand-written one (`String` vs `SmolStr`, public fields, `MessageField`/`EnumValue`,
   `Message` trait vs `Encode`/`Decode`, no `*Ref` borrow types). Adapting consumers is its
   own later sub-project, not part of SP0–SP3.

## 3. Decomposition

| ID | Sub-project | Scope | Sequencing |
|----|-------------|-------|------------|
| **SP0** | **Foundation** | Crate skeleton, codegen pipeline, `mediatime/buffa` feature + extern wiring, serde/arbitrary/quickcheck, round-trip harness — proven on a vertical slice | **First** (this spec) |
| SP1 | `common` (~50 types) | Detection/analysis messages | After SP0 |
| SP2 | `database` | scene/keyframe/video/media + audio subsystem | After SP1 (depends on `common`) |
| SP3 | `network` | daemon/desktop protocol messages | After SP1 (depends on `common`) |
| — | Consumer cutover | Re-point ~13 workspace crates | Deferred, separate effort |

Dependency direction follows the existing module tree: `common` is foundational; `database`
and `network` build on it.

## 4. SP0 — Foundation: Goal

Stand up `mediaschema` as a buffa-codegen crate with the **entire toolchain working
end-to-end** (`.proto` → checked-in generated Rust → serde/arbitrary/quickcheck →
round-trip + semantic tests), proven on a thin vertical slice:

- `Detection` — `findit-proto`: `{ label: SmolStr, confidence: f32 }`
- `BoundingBox` — `findit-proto`: `{ x: f32, y: f32, width: f32, height: f32 }`
- the `mediatime` trio via `extern_path` (`Timebase`, `TimeRange`, `Timestamp`)

SP0 is **not** about migrating real volume — it is about de-risking the pipeline (especially
the mediatime extern integration and the quickcheck bridge) before SP1–SP3 scale it.

## 5. SP0 Design

### A. Crate & repo layout

Rename the `template-rs` scaffold → `mediaschema` (package name, `Cargo.toml` metadata,
README). Keep the standalone repo and branch `0.1.0`.

```
mediaschema/
  proto/
    findit/common/v1/detection.proto      # Detection, BoundingBox
    mediatime/v1/mediatime.proto          # Timebase/TimeRange/Timestamp shapes; extern-mapped
  src/
    generated/                            # checked-in buffa output
      mod.rs                              # include_file stitcher (relative includes)
    lib.rs                                # re-exports generated tree + curated prelude
  xtask/                                  # `cargo xtask gen` → regenerates src/generated/
  tests/roundtrip.rs
mediaschema-derive/                       # tiny proc-macro crate: QuickcheckArbitrary
```

`mediaschema` Cargo features (mirror `findit-proto`'s shape):

| Feature | Effect |
|---------|--------|
| `default = ["std"]` | |
| `std` | `buffa/std` |
| `serde` | `buffa/json`, `buffa-types/json` (views+WKT), `dep:serde`, `dep:serde_json` |
| `arbitrary` | `dep:arbitrary`, `buffa/arbitrary` (literal name required by generated `cfg_attr`) |
| `quickcheck` | implies `arbitrary`; `dep:quickcheck`, `dep:mediaschema-derive` |

Dependencies: `buffa`, `buffa-types`, `mediatime` (with `buffa` feature), and the
feature-gated `serde`/`serde_json`/`arbitrary`/`quickcheck`/`mediaschema-derive`.

### B. Proto authoring + codegen pipeline

- `.proto` authored from the **Rust structs/enums**, clean numbering from 1, idiomatic
  proto3, one proto package per domain (`findit.common.v1` for the slice).
- Generation is a **manual developer step**, not `build.rs`: an `xtask` binary runs
  `buffa_build::Config`, writing into `src/generated/` with `include_file("mod.rs")` and an
  explicit `out_dir` (→ relative `include!("…")` so the tree is a normal `mod generated;`).
- `src/lib.rs` does `mod generated;` and re-exports a curated prelude.
- **CI gate:** re-run `cargo xtask gen`; `git diff --exit-code`. Mirrors buffa's own
  `check-generated-code` discipline.
- `protoc` **or** `buf` is required only at gen time (developer + CI), never by consumers.
  SP0 defaults to `protoc` (buffa-build's default `DescriptorSource`); switch to `buf`
  via `Config::use_buf()` if that is the available toolchain. The choice is pinned in the
  xtask so gen is reproducible.

`buffa_build::Config` (verified API surface) used roughly as:

```rust,ignore
buffa_build::Config::new()
    .files(&["proto/findit/common/v1/detection.proto",
             "proto/mediatime/v1/mediatime.proto"])
    .includes(&["proto"])
    .out_dir("src/generated")
    .include_file("mod.rs")
    .extern_path(".mediatime.v1", "::mediatime")
    .generate_json(true)                 // generated once with all capability derives;
    .generate_arbitrary(true)            //   Cargo features gate them downstream (see note)
    .use_bytes_type()                    // bytes fields → bytes::Bytes (matches findit-proto)
    .type_attribute(".",
        "#[cfg_attr(feature = \"quickcheck\", derive(mediaschema_derive::QuickcheckArbitrary))]")
    .compile()?;
```

Note: codegen flags (`generate_json`) are compile-time decisions, but generated code is
checked in once and feature-gated at the Rust level by the derives buffa emits
(`#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]` etc.). The xtask generates a
single canonical output with **all** capability derives enabled (`generate_json(true)`,
`generate_arbitrary(true)`, the quickcheck `type_attribute`); the Cargo features then switch
them on/off downstream. This is buffa's intended model and avoids per-feature generated trees.

### C. mediatime ↔ buffa integration — new `mediatime/buffa` feature

The novel, highest-risk piece. buffa's documented custom-type pattern (the same mechanism
`buffa-types` uses for well-known types): hand-implement `buffa::Message` for the type and
`extern_path` it from consuming crates.

In the **mediatime** repo, behind a new optional `buffa` feature
(`buffa = ["dep:buffa"]`, `buffa` pulled `default-features = false`):

For each of `Timebase`, `TimeRange`, `Timestamp`:

1. **`impl buffa::Message`** — `compute_size(&self, &mut SizeCache) -> u32`,
   `write_to(&self, &mut SizeCache, &mut impl bytes::BufMut)`,
   `merge_field(&mut self, buffa::encoding::Tag, &mut impl bytes::Buf, depth) -> Result<(), DecodeError>`,
   `clear(&mut self)`. Wire format = the authored `.proto` (clean; no legacy constraints).
   - `Timebase` is a scalar leaf → `SizeCache` unused.
   - `TimeRange` / `Timestamp` nest `Timebase` → use the `SizeCache.reserve()/set()`
     pre-order pattern (per buffa guide) for linear-time sizing.
   - `UnknownFields` omitted (leaf/simple types; unknown tags `skip_field`'d), per the
     guide's explicit allowance.
2. **`impl buffa::DefaultInstance`** via `buffa::__private::OnceBox` static.
3. **View types** at `::mediatime::__buffa::view::{Timebase,TimeRange,Timestamp}View`.
   None of the three borrow from the input buffer (only `i64`/`u32`/`NonZeroU32`), so each
   view is the owned-type alias: `pub type TimebaseView<'a> = Timebase;` inside
   `pub mod __buffa { pub mod view { … } }` under `#[cfg(feature = "buffa")]`.
4. **serde / arbitrary**: reuse mediatime's **existing** `serde` and `arbitrary` features
   (round-trip correctness only; proto3-canonical JSON exactness not required). `Timebase`'s
   serde already renames to `numerator`/`denominator` — acceptable.
5. **`MessageName`**: not implemented in SP0 (no `Any`/type-erased dispatch needed).

Proto shapes (authored from the live mediatime structs):

```protobuf
package mediatime.v1;
message Timebase  { uint32 num = 1;  uint32 den = 2; }   // den != 0 validated on decode
message TimeRange { int64 start = 1; int64 end = 2; Timebase timebase = 3; }
message Timestamp { int64 pts = 1;   Timebase timebase = 2; }
```

`mediatime::Timebase::default()` is `1/1`; `den` is `NonZeroU32` (decode rejects 0).
`mediaschema` consumes them via `.extern_path(".mediatime.v1", "::mediatime")`, so any
generated message with one of these as a field references the rich `mediatime` type
(singular message field ⇒ `buffa::MessageField<::mediatime::Timebase>` etc.).

**Fallback to de-risk SP0:** if buffa's view expectations for an extern type with a nested
message field prove awkward, SP0 may set `.generate_views(false)` for the slice — zero-copy
views are not required to prove the foundation. Decided during implementation; recorded here
as an accepted fallback.

### D. serde / arbitrary / quickcheck

- **serde** — buffa `generate_json(true)`: owned types derive `Serialize`/`Deserialize`
  (proto3-canonical JSON), views get a manual `Serialize`. Gated by `mediaschema/serde` →
  `buffa/json` (+ `buffa-types/json` because views + WKTs).
- **arbitrary** — buffa `generate_arbitrary(true)`: structural
  `#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]`. Sufficient for
  round-trip/fuzz. The Cargo feature **must** be literally `arbitrary` and forward to
  `buffa/arbitrary`. Hand-tuned domain-constrained arbitrary impls from `findit-proto` are
  ported **only** where a specific test depends on the constraint — none in the SP0 slice.
- **quickcheck** — buffa has no quickcheck support and quickcheck has no off-the-shelf
  derive. Mechanism: a tiny `mediaschema-derive` proc-macro `QuickcheckArbitrary` whose
  expansion is, for type `T`:

  ```rust,ignore
  #[cfg(feature = "quickcheck")]
  impl ::quickcheck::Arbitrary for T {
      fn arbitrary(g: &mut ::quickcheck::Gen) -> Self {
          // sample a byte budget from quickcheck's RNG, feed arbitrary
          let n = (usize::arbitrary(g) % 4096) + 64;
          let bytes: ::std::vec::Vec<u8> = (0..n).map(|_| u8::arbitrary(g)).collect();
          let mut u = ::arbitrary::Unstructured::new(&bytes);
          <T as ::arbitrary::Arbitrary>::arbitrary(&mut u).unwrap_or_default()
      }
  }
  ```

  Injected on every generated message/enum via the buffa `type_attribute(".", …)` shown in
  §B. Orphan-rule-legal: generated types are local to `mediaschema`, `quickcheck::Arbitrary`
  is foreign — a local-type/foreign-trait impl is allowed. `quickcheck` feature implies
  `arbitrary`. `mediatime` keeps its own native `quickcheck` feature (not the bridge).

### E. Correctness harness + Definition of Done

Tests (`tests/roundtrip.rs`, property-based via quickcheck and/or arbitrary):

- **Wire round-trip**, every slice type: `decode(encode(x)) == x`.
- **Owned ↔ view** consistency where views are enabled.
- **JSON round-trip** when `serde`: `from_str(to_string(x)) == x`.
- **Semantic equivalence** vs the old `findit-proto` types: a small explicit set proving the
  same logical content is representable (constructor/field mapping) — **not** byte equality.
- **mediatime extern**: a `mediaschema` message with `Timebase`/`TimeRange`/`Timestamp`
  fields round-trips, exercising `::mediatime::__buffa::view::*` and `MessageField`.

**SP0 is done when:**

1. `mediaschema` builds across feature combinations: default, `--no-default-features`,
   `std`, `serde`, `arbitrary`, `quickcheck`, and `all-features`.
2. `cargo xtask gen` reproduces the checked-in `src/generated/` (CI `git diff --exit-code`).
3. Round-trip + JSON + semantic tests pass for `Detection`, `BoundingBox`, and
   `Timebase`/`TimeRange`/`Timestamp`.
4. `mediatime` builds with `--features buffa`; its three types round-trip through a buffa
   parent message in `mediaschema`.
5. The quickcheck bridge is exercised by at least one property test.

## 6. Risks & Verification Items (resolve during planning/implementation)

- **buffa scalar-encoding helper names** (`uint32`/`int64` encode/len/decode in
  `buffa::types`, `Tag`/`WireType`/`skip_field` in `buffa::encoding`) — confirmed to exist
  from the guide's `Int64Range` example; exact symbol paths to be pinned when writing the
  `mediatime/buffa` impls.
- **Extern view with nested message field** — confirm buffa accepts an owned-alias view for
  `TimeRange`/`Timestamp` (they nest `Timebase`); fallback `generate_views(false)` for SP0.
- **mediatime `no_std`/no-alloc** — mediatime is currently `no_std` + no-alloc. The optional
  `buffa` feature pulls `buffa` (needs `alloc`). Acceptable because it is opt-in and gated;
  default mediatime stays no-alloc. Recorded, not blocking.
- **`Timestamp` field set** — designed as `{ pts, timebase }` from the live struct; reconfirm
  no additional fields when authoring the proto (no wire-compat constraint, so low risk).

## 7. Explicitly Out of Scope (SP0 and/or whole phase)

- redb `Key`/`Value` storage impls, identity types (`Id`, `FileChecksum`), uniffi FFI.
- Consumer cutover of the ~13 workspace crates and the generated-vs-hand-written API gap
  (`SmolStr`, private fields/`with_*`, `*Ref` borrow types, `Encode`/`Decode`).
- SP1–SP3 type volume (only the SP0 vertical slice is migrated here).
- Upstreaming a native `quickcheck` codegen mode into buffa (revisitable later).
