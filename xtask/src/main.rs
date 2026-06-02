//! `cargo run -p xtask -- gen` regenerates mediaschema/src/generated/.
//!
//! mediatime.v1.* is extern-mapped to ::mediatime (NOT generated); its
//! .proto exists only so protoc can resolve the import and so the wire
//! contract is documented.

use buffa_build::StringRepr;
use std::path::PathBuf;

fn main() {
  let mut args = std::env::args().skip(1);
  match args.next().as_deref() {
    Some("gen") => gen(),
    _ => {
      eprintln!("usage: cargo run -p xtask -- gen");
      std::process::exit(2);
    }
  }
}

fn gen() {
  // Crate root = mediaschema/ (xtask is a workspace member one level down).
  let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let root = manifest.parent().expect("workspace root").to_path_buf();

  buffa_build::Config::new()
        .files(&[root.join("proto/media/v1/types.proto")])
        .includes(&[root.join("proto")])
        .out_dir(root.join("src/generated"))
        .include_file("mod.rs")
        .extern_path(".mediatime.v1", "::mediatime")
        .generate_json(true)
        .generate_arbitrary(true)
        // buffa 0.6: without this, generate_json(true) emits UNGATED serde
        // derives (hard serde dep). On => serde is `#[cfg_attr(feature="json",…)]`.
        .gate_impls_on_crate_features(true)
        // No zero-copy views in SP0: scalar-only mediatime extern types don't
        // implement buffa's view-trait surface (ViewEncode/_decode_depth/…).
        .generate_views(false)
        // proto `bytes` -> ::buffa::bytes::Bytes (immutable, cheap-clone,
        // zero-copy-capable); the in-memory model is read-only, so a mutable
        // Vec<u8> buffer is unnecessary. buffa re-exports ::bytes and handles
        // json/arbitrary generically, so no extra dependency or feature
        // wiring is required.
        .use_bytes_type()
        // proto `string` -> `smol_str::SmolStr` (24-byte struct, inlines up
        // to 23 bytes, O(1) clone of long strings via `Arc<str>`). The wire
        // model is read-only, so SmolStr's immutability is a non-issue; the
        // cheap clone matters because messages copy strings through view +
        // owned conversions. buffa 0.7's `string_type` knob; consuming crate
        // must enable `buffa/smol_str` (forwarded via the `buffa` feature in
        // mediaschema/Cargo.toml) so buffa re-exports smol_str for codegen.
        .string_type(StringRepr::SmolStr)
        .compile()
        .expect("buffa codegen failed");

  println!("generated -> {}", root.join("src/generated").display());
}
