//! End-to-end example — construct a `Media` + `Audio` + `AudioTrack`
//! via the domain layer, encode each through the MongoDB `bson::Document`
//! backend, then decode them back and assert structural equality.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example end_to_end_mongodb --features mongodb
//! ```
//!
//! Requires both the `mongodb` backend and the `audio` aggregate gate.
//! Both are present in the default feature set so the canonical
//! invocation above just works; an explicit `--no-default-features`
//! invocation must also list `--features audio`. A build without that
//! pair compiles into a no-op `main` so `cargo build --examples`
//! still succeeds.

#[cfg(not(all(feature = "mongodb", feature = "audio")))]
fn main() {
  eprintln!(
    "end_to_end_mongodb example requires the `mongodb` and `audio` features; \
         rebuild with e.g. `cargo run --example end_to_end_mongodb --features mongodb`."
  );
}

#[cfg(all(feature = "mongodb", feature = "audio"))]
use bson::Document;
#[cfg(all(feature = "mongodb", feature = "audio"))]
use mediaframe::{codec::AudioCodec, container::Format, lang::Language};
#[cfg(all(feature = "mongodb", feature = "audio"))]
use mediaschema::domain::{primitives::FileChecksum, Audio, AudioTrack, Media, MediaKind, Uuid7};

#[cfg(all(feature = "mongodb", feature = "audio"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
  // ────────────────────────────────────────────────────────────────────
  // 1. Build a `Media` — content-identified by its `FileChecksum`.
  //    Constructor validates that `id` is non-nil and the checksum is
  //    non-zero.
  // ────────────────────────────────────────────────────────────────────
  let media = Media::try_new(
    Uuid7::new(),
    FileChecksum::from_bytes([0xAB; 32]),
    Format::Mp4,
    12_345,
    MediaKind::Audio,
  )?;
  println!("media id   = {}", media.id_ref());
  println!("media size = {} bytes", media.size());

  // ────────────────────────────────────────────────────────────────────
  // 2. Build the `Audio` facet pointing at the media's id.
  // ────────────────────────────────────────────────────────────────────
  let audio = Audio::try_new(Uuid7::new(), *media.id_ref())?;
  println!("audio facet id = {}", audio.id_ref());

  // ────────────────────────────────────────────────────────────────────
  // 3. Build an `AudioTrack` for the audio facet — set a few
  //    descriptors via the builder surface (each is intrinsic
  //    single-value validated where applicable).
  // ────────────────────────────────────────────────────────────────────
  let track = AudioTrack::try_new(Uuid7::new(), *audio.id_ref())?
    .try_with_sample_rate(48_000)?
    .try_with_channels(2)?
    .with_language(Some(Language::from_bcp47("en")?))
    .with_codec(AudioCodec::Aac);
  println!(
    "track {}@{} Hz / {} channels",
    track.codec_ref(),
    track.sample_rate(),
    track.channels(),
  );

  // ────────────────────────────────────────────────────────────────────
  // 4. Encode each through the mongodb backend. `From<&Domain> for
  //    bson::Document` is infallible — the domain is the validated
  //    side.
  // ────────────────────────────────────────────────────────────────────
  let media_doc: Document = (&media).into();
  let audio_doc: Document = (&audio).into();
  let track_doc: Document = (&track).into();
  println!(
    "media doc keys  = {:?}",
    media_doc.keys().collect::<Vec<_>>()
  );
  println!(
    "audio doc keys  = {:?}",
    audio_doc.keys().collect::<Vec<_>>()
  );
  println!(
    "track doc keys  = {:?}",
    track_doc.keys().collect::<Vec<_>>()
  );

  // ────────────────────────────────────────────────────────────────────
  // 5. Decode them back and assert the round trip is lossless.
  //    `TryFrom<Document> for Domain` routes through the same
  //    `try_new` + `with_*` builders, so every invariant is
  //    re-enforced at the bson edge.
  // ────────────────────────────────────────────────────────────────────
  let media_back: Media<Uuid7> = media_doc.try_into()?;
  let audio_back: Audio<Uuid7> = audio_doc.try_into()?;
  let track_back: AudioTrack<Uuid7> = track_doc.try_into()?;
  assert_eq!(media, media_back, "Media bson round-trip");
  assert_eq!(audio, audio_back, "Audio bson round-trip");
  assert_eq!(track, track_back, "AudioTrack bson round-trip");

  println!("\nall three aggregates round-tripped losslessly.");
  Ok(())
}
