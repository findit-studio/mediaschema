//! Shared cross-cutting value-objects.
//!
//! Defined once here, reused by every analysis aggregate
//! (`Keyframe`/`Scene`/`AudioTrack`/`SubtitleTrack`/…) per the locked README
//! cross-cutting rules. Free-text rule: `SmolStr` with `""`=absent, never
//! `Option<SmolStr>`.
//!
//! **Encapsulation rule:** no public fields anywhere in the domain layer.
//! Access goes through `field()` getters and `with_field(...)` const-where-
//! possible setters; mutation uses `set_field(...)`.

use derive_more::IsVariant;
use jiff::Timestamp as JiffTimestamp;
use smol_str::SmolStr;

use crate::domain::Uuid7;

// ---------------------------------------------------------------------------
// IndexProgress — per-kind facet rollup over child tracks
// ---------------------------------------------------------------------------

/// Per-kind facet rollup of `{total, indexed, failed}` over a kind
/// container's child tracks. **Denormalised cache** — the source of truth
/// lives on each `*Track`'s `index_status` + `index_errors`; the facet
/// maintains this so list queries don't have to re-aggregate across tracks.
///
/// Locked shared cross-cutting VO (`schema/README.md` "Indexing
/// model-correction"), reused by the `Video`/`Audio`/`Subtitle` facets via
/// each facet's `track_progress` field.
///
/// Invariant: `indexed + failed <= total`. Validated at the type boundary
/// via [`IndexProgress::try_new`]. The unchecked constructors
/// ([`IndexProgress::from_parts`] + the `with_*`/`set_*` field mutators) do
/// **not** enforce it — they exist for cheap field-wise (re)construction
/// where the rollup-recompute pass is the integrity backstop; reach for
/// `try_new` when the invariant must hold at the boundary.
///
/// **Default convention**: `Default::default()` calls [`IndexProgress::new`]
/// — the empty rollup `{0, 0, 0}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexProgress {
  total: u32,
  indexed: u32,
  failed: u32,
}

impl IndexProgress {
  /// Canonical no-arg constructor — the empty rollup (`{0, 0, 0}`).
  /// [`Default::default`] is `Self::new()`.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      total: 0,
      indexed: 0,
      failed: 0,
    }
  }

  /// Construct from the three counts directly, **without** validating the
  /// `indexed + failed <= total` invariant — use [`IndexProgress::try_new`]
  /// when the invariant must hold at the boundary.
  #[inline(always)]
  pub const fn from_parts(total: u32, indexed: u32, failed: u32) -> Self {
    Self {
      total,
      indexed,
      failed,
    }
  }

  /// Validating constructor: rejects `indexed + failed > total` (the rollup
  /// invariant). `u32::checked_add` guards the overflow case.
  pub const fn try_new(total: u32, indexed: u32, failed: u32) -> Result<Self, IndexProgressError> {
    // `u32::checked_add` is const fn since 1.61.
    let sum = match indexed.checked_add(failed) {
      Some(s) => s,
      None => return Err(IndexProgressError::SumOverflows),
    };
    if sum > total {
      return Err(IndexProgressError::SumExceedsTotal);
    }
    Ok(Self {
      total,
      indexed,
      failed,
    })
  }

  /// Total child tracks the facet owns.
  #[inline(always)]
  pub const fn total(&self) -> u32 {
    self.total
  }

  /// Tracks that finished indexing successfully.
  #[inline(always)]
  pub const fn indexed(&self) -> u32 {
    self.indexed
  }

  /// Tracks whose indexing failed (`index_errors` non-empty at the time of
  /// last rollup maintenance).
  #[inline(always)]
  pub const fn failed(&self) -> u32 {
    self.failed
  }

  /// True iff the facet has at least one failed track — the locked "kind
  /// container's error signal" rule (`failed > 0` ⇒ drill down).
  #[inline(always)]
  pub const fn has_failures(&self) -> bool {
    self.failed > 0
  }

  /// Builder: replace `total`.
  #[must_use]
  #[inline(always)]
  pub const fn with_total(mut self, total: u32) -> Self {
    self.total = total;
    self
  }

  /// Builder: replace `indexed`.
  #[must_use]
  #[inline(always)]
  pub const fn with_indexed(mut self, indexed: u32) -> Self {
    self.indexed = indexed;
    self
  }

  /// Builder: replace `failed`.
  #[must_use]
  #[inline(always)]
  pub const fn with_failed(mut self, failed: u32) -> Self {
    self.failed = failed;
    self
  }

  /// In-place mutator for `total`.
  #[inline(always)]
  pub const fn set_total(&mut self, total: u32) -> &mut Self {
    self.total = total;
    self
  }

  /// In-place mutator for `indexed`.
  #[inline(always)]
  pub const fn set_indexed(&mut self, indexed: u32) -> &mut Self {
    self.indexed = indexed;
    self
  }

  /// In-place mutator for `failed`.
  #[inline(always)]
  pub const fn set_failed(&mut self, failed: u32) -> &mut Self {
    self.failed = failed;
    self
  }
}

impl Default for IndexProgress {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

/// Error returned when [`IndexProgress::try_new`] cannot uphold the
/// `indexed + failed <= total` invariant. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum IndexProgressError {
  /// `indexed + failed > total` — would overcount.
  #[error("IndexProgress: indexed + failed must not exceed total")]
  SumExceedsTotal,
  /// `indexed + failed` overflows `u32` — definitely overcounts.
  #[error("IndexProgress: indexed + failed overflows u32")]
  SumOverflows,
}

// ---------------------------------------------------------------------------
// Provenance — analysis-run reproducibility
// ---------------------------------------------------------------------------

/// Analysis-run reproducibility — which model/prompt/indexer produced this
/// analysis record, so a re-run on upgrade is deterministic.
///
/// Locked shared cross-cutting VO (see `schema/README.md`). Carried by every
/// analysis record — `Keyframe`, `Scene`, `AudioTrack` index-state,
/// `SubtitleTrack` — as a `provenance` field. Per-track on `AudioTrack`/
/// `SubtitleTrack` (one value per run), not per `AudioSegment`/`SubtitleCue`.
///
/// All four fields are `SmolStr` with `""`=absent. No `Option` — the locked
/// rule reserves `Option` for structured/enum/numeric absence.
///
/// **Default convention**: `Default::default()` calls [`Provenance::new`],
/// which returns the all-empty record. Use [`Provenance::from_parts`] to
/// supply all four fields in one call, or chain the `with_*` builders
/// onto `Provenance::new()` to fill incrementally.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Provenance {
  model_name: SmolStr,
  model_version: SmolStr,
  prompt_version: SmolStr,
  indexer_version: SmolStr,
}

impl Provenance {
  /// Canonical no-arg constructor — every field empty (`""`).
  /// [`Default::default`] is `Self::new()`.
  ///
  /// (Not `const fn` — `SmolStr::default()` is not `const` in
  /// `smol_str` 0.3.)
  #[inline]
  pub fn new() -> Self {
    Self {
      model_name: SmolStr::default(),
      model_version: SmolStr::default(),
      prompt_version: SmolStr::default(),
      indexer_version: SmolStr::default(),
    }
  }

  /// Construct a `Provenance` from its four fields.
  #[inline]
  pub fn from_parts(
    model_name: impl Into<SmolStr>,
    model_version: impl Into<SmolStr>,
    prompt_version: impl Into<SmolStr>,
    indexer_version: impl Into<SmolStr>,
  ) -> Self {
    Self {
      model_name: model_name.into(),
      model_version: model_version.into(),
      prompt_version: prompt_version.into(),
      indexer_version: indexer_version.into(),
    }
  }

  /// Model name (`""` = absent).
  #[inline]
  pub fn model_name(&self) -> &str {
    self.model_name.as_str()
  }

  /// Model version (`""` = absent).
  #[inline]
  pub fn model_version(&self) -> &str {
    self.model_version.as_str()
  }

  /// Prompt-template version (`""` = absent).
  #[inline]
  pub fn prompt_version(&self) -> &str {
    self.prompt_version.as_str()
  }

  /// Indexer build/version (`""` = absent).
  #[inline]
  pub fn indexer_version(&self) -> &str {
    self.indexer_version.as_str()
  }

  /// Builder: replace `model_name` and return `self`.
  #[inline]
  pub fn with_model_name(mut self, v: impl Into<SmolStr>) -> Self {
    self.model_name = v.into();
    self
  }

  /// Builder: replace `model_version` and return `self`.
  #[inline]
  pub fn with_model_version(mut self, v: impl Into<SmolStr>) -> Self {
    self.model_version = v.into();
    self
  }

  /// Builder: replace `prompt_version` and return `self`.
  #[inline]
  pub fn with_prompt_version(mut self, v: impl Into<SmolStr>) -> Self {
    self.prompt_version = v.into();
    self
  }

  /// Builder: replace `indexer_version` and return `self`.
  #[inline]
  pub fn with_indexer_version(mut self, v: impl Into<SmolStr>) -> Self {
    self.indexer_version = v.into();
    self
  }

  /// In-place mutator.
  #[inline]
  pub fn set_model_name(&mut self, v: impl Into<SmolStr>) {
    self.model_name = v.into();
  }

  /// In-place mutator.
  #[inline]
  pub fn set_model_version(&mut self, v: impl Into<SmolStr>) {
    self.model_version = v.into();
  }

  /// In-place mutator.
  #[inline]
  pub fn set_prompt_version(&mut self, v: impl Into<SmolStr>) {
    self.prompt_version = v.into();
  }

  /// In-place mutator.
  #[inline]
  pub fn set_indexer_version(&mut self, v: impl Into<SmolStr>) {
    self.indexer_version = v.into();
  }

  /// Is every field absent (`""`)? Useful when an analysis record exists
  /// but its provenance has not been recorded yet.
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.model_name.is_empty()
      && self.model_version.is_empty()
      && self.prompt_version.is_empty()
      && self.indexer_version.is_empty()
  }
}

impl Default for Provenance {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

// ---------------------------------------------------------------------------
// LocalizedText — free-text + optional translation
// ---------------------------------------------------------------------------

/// Free-text narrative paired with an optional translation to a single
/// canonical target (English by convention).
///
/// Locked shared cross-cutting VO (see `schema/README.md`). Used for *all
/// VLM natural-language output* on `Keyframe.VlmAnalysis` (every text/label
/// vector field) and `AudioSegment.text` (with the planned `asry`
/// `translated_text` extension — whisper-`translate` task). Per-token text
/// (e.g. `AudioSegment.Word.text`) stays `SmolStr` — translation is segment
/// / narrative-level only, not word-aligned.
///
/// Both fields are `SmolStr` with `""`=absent. `pt-BR` ≠ `pt-PT` are
/// distinct values; the language tag itself lives in `mediaframe::Language`
/// (a separate field where present).
///
/// **Default convention**: `Default::default()` calls
/// [`LocalizedText::new`], which returns the all-empty record. Use
/// [`LocalizedText::from_src`] for source-only, or
/// [`LocalizedText::from_src_translated`] for both fields in one call.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalizedText {
  src: SmolStr,
  translated: SmolStr,
}

impl LocalizedText {
  /// Canonical no-arg constructor — both fields empty (`""`).
  /// [`Default::default`] is `Self::new()`.
  ///
  /// (Not `const fn` — `SmolStr::default()` is not `const` in
  /// `smol_str` 0.3.)
  #[inline]
  pub fn new() -> Self {
    Self {
      src: SmolStr::default(),
      translated: SmolStr::default(),
    }
  }

  /// Construct from explicit source + translation.
  #[inline]
  pub fn from_src_translated(src: impl Into<SmolStr>, translated: impl Into<SmolStr>) -> Self {
    Self {
      src: src.into(),
      translated: translated.into(),
    }
  }

  /// Construct from source text only (translation not yet available).
  #[inline]
  pub fn from_src(src: impl Into<SmolStr>) -> Self {
    Self {
      src: src.into(),
      translated: SmolStr::default(),
    }
  }

  /// Source text (`""` = absent).
  #[inline]
  pub fn src(&self) -> &str {
    self.src.as_str()
  }

  /// Translated text (`""` = no translation yet).
  #[inline]
  pub fn translated(&self) -> &str {
    self.translated.as_str()
  }

  /// Builder: replace `src` and return `self`.
  #[inline]
  pub fn with_src(mut self, v: impl Into<SmolStr>) -> Self {
    self.src = v.into();
    self
  }

  /// Builder: replace `translated` and return `self`.
  #[inline]
  pub fn with_translated(mut self, v: impl Into<SmolStr>) -> Self {
    self.translated = v.into();
    self
  }

  /// In-place mutator.
  #[inline]
  pub fn set_src(&mut self, v: impl Into<SmolStr>) {
    self.src = v.into();
  }

  /// In-place mutator.
  #[inline]
  pub fn set_translated(&mut self, v: impl Into<SmolStr>) {
    self.translated = v.into();
  }

  /// Both fields empty?
  #[inline]
  pub fn is_empty(&self) -> bool {
    self.src.is_empty() && self.translated.is_empty()
  }

  /// Best available text: `translated` if non-empty, else `src`. Useful
  /// for default UI rendering / search-index population.
  #[inline]
  pub fn display(&self) -> &str {
    if !self.translated.is_empty() {
      self.translated.as_str()
    } else {
      self.src.as_str()
    }
  }
}

impl Default for LocalizedText {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

// ---------------------------------------------------------------------------
// VoiceFingerprint — vendor-neutral voice-embedding metadata
// ---------------------------------------------------------------------------

/// Voice-embedding metadata. The vector itself lives in an external
/// vector store (LanceDB / Qdrant / Milvus / pgvector / …, the application
/// picks one); this VO is the linkage + provenance kept in the relational /
/// document model.
///
/// Vendor-neutral on purpose: `vector_id` is opaque to mediaschema; the
/// application is responsible for keeping it consistent with whichever
/// vector store is in use. Migrating to a different store is a re-write of
/// the external vector data under the same `vector_id` keys; mediaschema is
/// not touched.
///
/// Used at three levels — per-`AudioSegment` (the raw extraction from one
/// speak range), per-`Speaker` (per-track centroid), and per-`Person`
/// (cross-track / cross-modality centroid). Wired into those aggregates in
/// a later stacked PR; this PR adds JUST the VO definition.
///
/// Decoupled from `dia`'s segmentation/clustering model: the embedding
/// model recorded in `provenance` is a separate, stable choice. Swapping
/// `dia` does not invalidate fingerprints, because the fingerprint model's
/// vector space is independent.
///
/// ## Derives
///
/// `PartialEq` but **not `Eq`/`Hash`** — the `confidence: Option<f32>` field
/// carries a float (NaN ≠ NaN), so the locked rule from
/// `rust-type-conventions` §1 ("a floating-point field precludes
/// `Eq`/`Hash`") applies. The other VOs in this module (`Provenance`,
/// `LocalizedText`, `IndexProgress`) are `Eq`/`Hash` because they are
/// string- / integer-only.
///
/// ## Construction
///
/// - [`VoiceFingerprint::try_new`] on the canonical `Uuid7` specialization —
///   the validating boundary entry (nil `vector_id`, zero `dimensions`,
///   non-finite / out-of-range `confidence` rejected).
/// - [`VoiceFingerprint::from_parts`] on the generic `impl<Id>` — the
///   storage-reconstruction constructor; mirrors
///   [`MediaFile::from_parts`](crate::domain::MediaFile::from_parts) and
///   [`IndexProgress::from_parts`].
///
/// **No `Default` impl** — a "default" fingerprint would carry a nil
/// `vector_id` (orphan linkage) and zero `dimensions` (nonsensical vector
/// shape). Construct via `try_new` / `from_parts`.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceFingerprint<Id = Uuid7> {
  /// Opaque key into the external vector store. mediaschema does not
  /// interpret it — the application keeps it consistent with whichever
  /// backend (LanceDB / Qdrant / Milvus / pgvector / …) is in use.
  vector_id: Id,
  /// Dimensionality of the embedding (the length of the vector that
  /// lives in the external store). Validated `> 0`.
  dimensions: u32,
  /// Wall-clock time the embedding was extracted (the moment the model
  /// produced the vector — the equivalent of `Provenance`'s "when this
  /// analysis ran" for the embedding model).
  extracted_at: JiffTimestamp,
  /// Model-reported confidence in the embedding's quality, in `[0.0,
  /// 1.0]`. Validated finite (no `NaN` / `±inf`) and in range. `None`
  /// when the model does not expose a confidence score.
  confidence: Option<f32>,
  /// Which embedding model + version + indexer produced this
  /// fingerprint. Reused from the shared cross-cutting VO so the
  /// "swap the embedding model and re-extract" story is uniform with
  /// the rest of the analysis layer.
  provenance: Provenance,
}

impl VoiceFingerprint<Uuid7> {
  /// Validating constructor for the canonical `Uuid7` identity type.
  ///
  /// Rejects:
  /// - nil `vector_id` (a fingerprint with no linkage into the vector
  ///   store is an orphan — every fingerprint must point at a real
  ///   external vector);
  /// - `dimensions == 0` (a zero-dim embedding is nonsensical);
  /// - `confidence` that is `Some` and either non-finite (`NaN` /
  ///   `±inf`) or outside `[0.0, 1.0]`.
  ///
  /// Mirrors [`MediaFile::try_new`](crate::domain::MediaFile::try_new):
  /// the canonical fallible boundary entry on the `Uuid7`
  /// specialization, with [`VoiceFingerprint::from_parts`] on the
  /// generic `impl<Id>` for storage reconstruction.
  pub fn try_new(
    vector_id: Uuid7,
    dimensions: u32,
    extracted_at: JiffTimestamp,
    confidence: Option<f32>,
    provenance: Provenance,
  ) -> Result<Self, VoiceFingerprintError> {
    if vector_id.is_nil() {
      return Err(VoiceFingerprintError::NilVectorId);
    }
    if dimensions == 0 {
      return Err(VoiceFingerprintError::ZeroDimensions);
    }
    if let Some(c) = confidence {
      if !c.is_finite() || !(0.0..=1.0).contains(&c) {
        return Err(VoiceFingerprintError::ConfidenceOutOfRange);
      }
    }
    Ok(Self {
      vector_id,
      dimensions,
      extracted_at,
      confidence,
      provenance,
    })
  }
}

impl<Id> VoiceFingerprint<Id> {
  /// Raw constructor for **storage / wire reconstruction** — assembles a
  /// `VoiceFingerprint` directly from its persisted fields, bypassing the
  /// nil/zero/range checks that [`VoiceFingerprint::try_new`] performs.
  /// Intended ONLY for backends rebuilding a fingerprint from a trusted
  /// persisted row/document (the data was validated by `try_new` when
  /// first written). Application code building a fresh `VoiceFingerprint`
  /// must use [`VoiceFingerprint::try_new`].
  #[inline(always)]
  #[must_use]
  pub const fn from_parts(
    vector_id: Id,
    dimensions: u32,
    extracted_at: JiffTimestamp,
    confidence: Option<f32>,
    provenance: Provenance,
  ) -> Self {
    Self {
      vector_id,
      dimensions,
      extracted_at,
      confidence,
      provenance,
    }
  }

  /// Opaque key into the external vector store. Borrowed because `Id`
  /// is generic and not assumed to be `Copy`.
  #[inline(always)]
  pub const fn vector_id_ref(&self) -> &Id {
    &self.vector_id
  }

  /// Dimensionality of the embedding (`> 0` for `try_new`-constructed
  /// values).
  #[inline(always)]
  pub const fn dimensions(&self) -> u32 {
    self.dimensions
  }

  /// Wall-clock time the embedding was extracted. `jiff::Timestamp` is
  /// `Copy`, so by-value.
  #[inline(always)]
  pub const fn extracted_at(&self) -> JiffTimestamp {
    self.extracted_at
  }

  /// Model-reported confidence in `[0.0, 1.0]`, or `None` when the
  /// model does not expose one.
  #[inline(always)]
  pub const fn confidence(&self) -> Option<f32> {
    self.confidence
  }

  /// Which embedding model + version + indexer produced this fingerprint.
  #[inline(always)]
  pub const fn provenance_ref(&self) -> &Provenance {
    &self.provenance
  }

  // --- builders -----------------------------------------------------------

  /// Builder: replace `vector_id`. Plain (non-fallible) — the nil-id
  /// rejection is only meaningful at construction, where it gates the
  /// boundary entry. Storage-reconstruction code uses
  /// [`VoiceFingerprint::from_parts`] and is responsible for the
  /// linkage's integrity.
  #[inline(always)]
  #[must_use]
  pub fn with_vector_id(mut self, vector_id: Id) -> Self {
    self.vector_id = vector_id;
    self
  }

  /// Builder: replace `dimensions`, rejecting `0`.
  #[inline]
  pub fn try_with_dimensions(mut self, dimensions: u32) -> Result<Self, VoiceFingerprintError> {
    self.try_set_dimensions(dimensions)?;
    Ok(self)
  }

  /// Builder: replace `extracted_at`.
  #[inline(always)]
  #[must_use]
  pub const fn with_extracted_at(mut self, t: JiffTimestamp) -> Self {
    self.extracted_at = t;
    self
  }

  /// Builder: replace `confidence` with a *present* value, rejecting
  /// non-finite (`NaN` / `±inf`) and out-of-range. Mirrors the
  /// `set_*` / `with_*` "present value" form of the `Option<T>`
  /// mutator vocabulary (golden-rules §3).
  #[inline]
  pub fn try_with_confidence(mut self, c: f32) -> Result<Self, VoiceFingerprintError> {
    self.try_set_confidence(c)?;
    Ok(self)
  }

  /// Builder: replace `provenance`.
  #[inline(always)]
  #[must_use]
  pub fn with_provenance(mut self, provenance: Provenance) -> Self {
    self.provenance = provenance;
    self
  }

  // --- in-place setters ---------------------------------------------------

  /// In-place mutator for `vector_id`. Plain — see
  /// [`VoiceFingerprint::with_vector_id`].
  #[inline(always)]
  pub fn set_vector_id(&mut self, vector_id: Id) -> &mut Self {
    self.vector_id = vector_id;
    self
  }

  /// In-place mutator for `dimensions`, rejecting `0`. On error `self`
  /// is left unchanged.
  #[inline]
  pub fn try_set_dimensions(
    &mut self,
    dimensions: u32,
  ) -> Result<&mut Self, VoiceFingerprintError> {
    if dimensions == 0 {
      return Err(VoiceFingerprintError::ZeroDimensions);
    }
    self.dimensions = dimensions;
    Ok(self)
  }

  /// In-place mutator for `extracted_at`.
  #[inline(always)]
  pub const fn set_extracted_at(&mut self, t: JiffTimestamp) -> &mut Self {
    self.extracted_at = t;
    self
  }

  /// In-place mutator for `confidence` with a *present* value, rejecting
  /// non-finite / out-of-range. On error `self` is left unchanged.
  #[inline]
  pub fn try_set_confidence(&mut self, c: f32) -> Result<&mut Self, VoiceFingerprintError> {
    if !c.is_finite() || !(0.0..=1.0).contains(&c) {
      return Err(VoiceFingerprintError::ConfidenceOutOfRange);
    }
    self.confidence = Some(c);
    Ok(self)
  }

  /// In-place mutator: clear `confidence` (absent state — model did not
  /// expose one).
  #[inline(always)]
  pub const fn clear_confidence(&mut self) -> &mut Self {
    self.confidence = None;
    self
  }

  /// In-place mutator for `provenance`.
  #[inline(always)]
  pub fn set_provenance(&mut self, provenance: Provenance) -> &mut Self {
    self.provenance = provenance;
    self
  }
}

/// Error returned when a [`VoiceFingerprint`] construction or validating
/// mutation cannot uphold the type's invariants. Unit-only enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IsVariant, thiserror::Error)]
#[non_exhaustive]
pub enum VoiceFingerprintError {
  /// Supplied `vector_id` was the [`Uuid7`] nil sentinel — a fingerprint
  /// with no linkage into the external vector store is an orphan.
  #[error("VoiceFingerprint vector_id must not be the nil UUID")]
  NilVectorId,
  /// Supplied `dimensions` was `0` — a zero-dimensional embedding is
  /// nonsensical.
  #[error("VoiceFingerprint dimensions must be > 0")]
  ZeroDimensions,
  /// Supplied `confidence` was not finite (`NaN` / `±inf`) or fell
  /// outside `[0.0, 1.0]`.
  #[error("VoiceFingerprint confidence must be finite and in [0.0, 1.0]")]
  ConfidenceOutOfRange,
}

// ===========================================================================
// Tests
// ===========================================================================

// vo.rs itself is only compiled under `feature = "alloc"`, so the
// test module is automatically alloc-gated. No separate gate needed.
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn provenance_new_is_empty_and_default_delegates() {
    // `Default::default()` calls `Self::new()` per the rule.
    let p = Provenance::new();
    assert!(p.is_empty());
    assert_eq!(p.model_name(), "");
    assert_eq!(p.model_version(), "");
    assert_eq!(p.prompt_version(), "");
    assert_eq!(p.indexer_version(), "");
    assert_eq!(Provenance::default(), p);
  }

  #[test]
  fn provenance_construction_and_emptiness() {
    let p = Provenance::from_parts(
      "qwen2-vl-7b",
      "v0.3.0",
      "vlm-prompt@2",
      "findit-indexer-0.1.0",
    );
    assert!(!p.is_empty());
    assert_eq!(p.model_name(), "qwen2-vl-7b");
    assert_eq!(p.indexer_version(), "findit-indexer-0.1.0");

    // Even one non-empty field defeats is_empty.
    let p2 = Provenance::from_parts("", "", "", "x");
    assert!(!p2.is_empty());
  }

  #[test]
  fn provenance_builders_chain() {
    let p = Provenance::default()
      .with_model_name("qwen2-vl-7b")
      .with_model_version("v0.3.0")
      .with_prompt_version("vlm-prompt@2")
      .with_indexer_version("findit-indexer-0.1.0");
    assert_eq!(p.model_name(), "qwen2-vl-7b");
    assert_eq!(p.model_version(), "v0.3.0");
    assert_eq!(p.prompt_version(), "vlm-prompt@2");
    assert_eq!(p.indexer_version(), "findit-indexer-0.1.0");
  }

  #[test]
  fn provenance_setters_mutate_in_place() {
    let mut p = Provenance::default();
    p.set_model_name("qwen2-vl-7b");
    p.set_indexer_version("findit-indexer-0.1.0");
    assert_eq!(p.model_name(), "qwen2-vl-7b");
    assert_eq!(p.indexer_version(), "findit-indexer-0.1.0");
  }

  #[test]
  fn localized_text_new_is_empty_and_default_delegates() {
    let t = LocalizedText::new();
    assert!(t.is_empty());
    assert_eq!(t.display(), "");
    assert_eq!(LocalizedText::default(), t);
  }

  #[test]
  fn localized_text_from_src_no_translation() {
    let t = LocalizedText::from_src("Jane is eating");
    assert!(!t.is_empty());
    assert_eq!(t.src(), "Jane is eating");
    assert_eq!(t.translated(), "");
    // No translation → display falls back to src.
    assert_eq!(t.display(), "Jane is eating");
  }

  #[test]
  fn localized_text_display_prefers_translation() {
    let t = LocalizedText::from_src_translated("\u{4f60}\u{597d}", "Hello");
    assert_eq!(t.src(), "\u{4f60}\u{597d}");
    assert_eq!(t.translated(), "Hello");
    // Translation present → display returns it.
    assert_eq!(t.display(), "Hello");
  }

  #[test]
  fn localized_text_translation_only_displays_translation() {
    let t = LocalizedText::from_src_translated("", "Hello");
    assert!(!t.is_empty());
    assert_eq!(t.display(), "Hello");
  }

  #[test]
  fn localized_text_builders_and_setters() {
    let t = LocalizedText::default()
      .with_src("Jane")
      .with_translated("Jane");
    assert_eq!(t.display(), "Jane");
    let mut t = t;
    t.set_translated("");
    assert_eq!(t.translated(), "");
    assert_eq!(t.display(), "Jane");
  }

  #[test]
  fn index_progress_new_is_empty_and_default_delegates() {
    let p = IndexProgress::new();
    assert_eq!(p.total(), 0);
    assert_eq!(p.indexed(), 0);
    assert_eq!(p.failed(), 0);
    assert!(!p.has_failures());
    assert_eq!(IndexProgress::default(), p);
  }

  #[test]
  fn index_progress_from_parts_is_unchecked() {
    // `from_parts` does NOT validate the rollup invariant.
    let p = IndexProgress::from_parts(2, 1, 0);
    assert_eq!(p.total(), 2);
    assert_eq!(p.indexed(), 1);
    assert_eq!(p.failed(), 0);
  }

  #[test]
  fn index_progress_try_new_validates_invariant() {
    assert_eq!(
      IndexProgress::try_new(2, 2, 1).err(),
      Some(IndexProgressError::SumExceedsTotal)
    );
    assert!(IndexProgressError::SumExceedsTotal.is_sum_exceeds_total());
    assert_eq!(
      IndexProgress::try_new(u32::MAX, u32::MAX, 1).err(),
      Some(IndexProgressError::SumOverflows)
    );
    assert!(IndexProgressError::SumOverflows.is_sum_overflows());
    let ok = IndexProgress::try_new(5, 3, 2).unwrap();
    assert_eq!(ok.total(), 5);
  }

  #[test]
  fn index_progress_has_failures() {
    let none = IndexProgress::try_new(5, 5, 0).unwrap();
    let some = IndexProgress::try_new(5, 3, 2).unwrap();
    assert!(!none.has_failures());
    assert!(some.has_failures());
  }

  #[test]
  fn index_progress_builders_and_setters() {
    let p = IndexProgress::new()
      .with_total(5)
      .with_indexed(3)
      .with_failed(1);
    assert_eq!(p.total(), 5);
    assert_eq!(p.indexed(), 3);
    assert_eq!(p.failed(), 1);

    let mut p = p;
    p.set_total(10);
    p.set_indexed(7);
    p.set_failed(2);
    assert_eq!(p.total(), 10);
    assert_eq!(p.indexed(), 7);
    assert_eq!(p.failed(), 2);
  }

  // -------------------------------------------------------------------------
  // VoiceFingerprint
  // -------------------------------------------------------------------------

  /// A representative extraction timestamp.
  fn vfp_ts() -> JiffTimestamp {
    JiffTimestamp::from_millisecond(1_700_000_000_000).expect("valid timestamp")
  }

  fn vfp_provenance() -> Provenance {
    Provenance::from_parts("ecapa-tdnn", "v1.0.0", "", "findit-indexer-0.1.0")
  }

  #[test]
  fn voice_fingerprint_try_new_rejects_nil_vector_id() {
    let err = VoiceFingerprint::try_new(Uuid7::nil(), 192, vfp_ts(), Some(0.95), vfp_provenance())
      .unwrap_err();
    assert_eq!(err, VoiceFingerprintError::NilVectorId);
    assert!(err.is_nil_vector_id());
  }

  #[test]
  fn voice_fingerprint_try_new_rejects_zero_dimensions() {
    let err = VoiceFingerprint::try_new(Uuid7::new(), 0, vfp_ts(), Some(0.95), vfp_provenance())
      .unwrap_err();
    assert_eq!(err, VoiceFingerprintError::ZeroDimensions);
    assert!(err.is_zero_dimensions());
  }

  #[test]
  fn voice_fingerprint_try_new_rejects_nan_inf_out_of_range_confidence() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
      let err = VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), Some(bad), vfp_provenance())
        .unwrap_err();
      assert_eq!(
        err,
        VoiceFingerprintError::ConfidenceOutOfRange,
        "value {bad} should be rejected"
      );
      assert!(err.is_confidence_out_of_range());
    }
    // Boundary values 0.0 and 1.0 must be accepted.
    for ok in [0.0_f32, 1.0_f32] {
      VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), Some(ok), vfp_provenance())
        .expect("boundary confidence must be accepted");
    }
    // `None` is always fine — the model may not expose a score.
    VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), None, vfp_provenance())
      .expect("None confidence must be accepted");
  }

  #[test]
  fn voice_fingerprint_try_new_happy_path_and_from_parts_round_trip() {
    let vid = Uuid7::new();
    let prov = vfp_provenance();
    let f = VoiceFingerprint::try_new(vid, 192, vfp_ts(), Some(0.83), prov.clone())
      .expect("valid construction must succeed");
    assert_eq!(f.vector_id_ref(), &vid);
    assert_eq!(f.dimensions(), 192);
    assert_eq!(f.extracted_at(), vfp_ts());
    assert_eq!(f.confidence(), Some(0.83));
    assert_eq!(f.provenance_ref(), &prov);

    // `from_parts` round-trips a validated instance — mirrors
    // `MediaFile::from_parts` round-trip test.
    let rebuilt = VoiceFingerprint::from_parts(
      *f.vector_id_ref(),
      f.dimensions(),
      f.extracted_at(),
      f.confidence(),
      f.provenance_ref().clone(),
    );
    assert_eq!(rebuilt, f);
  }

  #[test]
  fn voice_fingerprint_with_vector_id_replaces_field() {
    let f = VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), None, vfp_provenance()).unwrap();
    let new_id = Uuid7::new();
    let f = f.with_vector_id(new_id);
    assert_eq!(f.vector_id_ref(), &new_id);
  }

  #[test]
  fn voice_fingerprint_try_with_dimensions_validates() {
    let f = VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), None, vfp_provenance()).unwrap();
    assert_eq!(
      f.clone().try_with_dimensions(0).unwrap_err(),
      VoiceFingerprintError::ZeroDimensions
    );
    let f = f.try_with_dimensions(256).unwrap();
    assert_eq!(f.dimensions(), 256);
  }

  #[test]
  fn voice_fingerprint_with_extracted_at_replaces_field() {
    let f = VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), None, vfp_provenance()).unwrap();
    let later = JiffTimestamp::from_millisecond(1_800_000_000_000).unwrap();
    let f = f.with_extracted_at(later);
    assert_eq!(f.extracted_at(), later);
  }

  #[test]
  fn voice_fingerprint_try_with_confidence_validates_and_clear() {
    let f = VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), None, vfp_provenance()).unwrap();
    // Reject non-finite.
    assert_eq!(
      f.clone().try_with_confidence(f32::NAN).unwrap_err(),
      VoiceFingerprintError::ConfidenceOutOfRange
    );
    // Reject out-of-range.
    assert_eq!(
      f.clone().try_with_confidence(1.5).unwrap_err(),
      VoiceFingerprintError::ConfidenceOutOfRange
    );
    let f = f.try_with_confidence(0.5).unwrap();
    assert_eq!(f.confidence(), Some(0.5));

    // clear_confidence (in-place) drops it to None.
    let mut f = f;
    f.clear_confidence();
    assert_eq!(f.confidence(), None);
  }

  #[test]
  fn voice_fingerprint_with_provenance_replaces_field() {
    let f =
      VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), None, Provenance::default()).unwrap();
    let p2 = vfp_provenance();
    let f = f.with_provenance(p2.clone());
    assert_eq!(f.provenance_ref(), &p2);
  }

  #[test]
  fn voice_fingerprint_in_place_setters_chain() {
    let mut f =
      VoiceFingerprint::try_new(Uuid7::new(), 192, vfp_ts(), None, Provenance::default()).unwrap();
    let later = JiffTimestamp::from_millisecond(1_800_000_000_000).unwrap();
    let new_id = Uuid7::new();
    let p2 = vfp_provenance();
    f.set_vector_id(new_id)
      .set_extracted_at(later)
      .set_provenance(p2.clone())
      .try_set_dimensions(256)
      .unwrap()
      .try_set_confidence(0.75)
      .unwrap();
    assert_eq!(f.vector_id_ref(), &new_id);
    assert_eq!(f.dimensions(), 256);
    assert_eq!(f.extracted_at(), later);
    assert_eq!(f.confidence(), Some(0.75));
    assert_eq!(f.provenance_ref(), &p2);

    // Validating setters leave self unchanged on error.
    assert_eq!(
      f.try_set_dimensions(0).unwrap_err(),
      VoiceFingerprintError::ZeroDimensions
    );
    assert_eq!(f.dimensions(), 256, "rejected setter must not mutate");
    assert_eq!(
      f.try_set_confidence(f32::INFINITY).unwrap_err(),
      VoiceFingerprintError::ConfidenceOutOfRange
    );
    assert_eq!(
      f.confidence(),
      Some(0.75),
      "rejected setter must not mutate"
    );
  }
}
