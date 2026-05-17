//! `cargo run -p xtask -- gen` regenerates mediaschema/src/generated/.
//!
//! mediatime.v1.* is extern-mapped to ::mediatime (NOT generated); its
//! .proto exists only so protoc can resolve the import and so the wire
//! contract is documented.

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
        .type_attribute(
            ".",
            "#[cfg_attr(feature = \"quickcheck\", derive(::mediaschema_derive::QuickcheckArbitrary))]",
        )
        .compile()
        .expect("buffa codegen failed");

    println!("generated -> {}", root.join("src/generated").display());
}
