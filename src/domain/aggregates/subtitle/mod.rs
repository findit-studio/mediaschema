//! Subtitle media-kind aggregates (locked `schema/subtitle*.md`).
//!
//! - [`Subtitle`] — thin subtitle facet of a `Media`.
//! - [`SubtitleTrack`] — one subtitle stream of a `Subtitle`.
//! - [`SubtitleCue`]`<Id, D>` — one parsed cue, polymorphic over the
//!   per-format payload `D` (`SrtData`, `VttData`, `AssData`, `LrcData`).
//!   See [`cue`] for the full type list (per-format `D` types, aggregate
//!   types `VttRegion`/`VttStyleBlock`/`AssStyle`/`LrcMetadata`, the
//!   closed [`SubtitleCueKind`] discriminator, and the type aliases
//!   `SrtCue`/`VttCue`/`AssCue`/`LrcCue`).

pub mod cue;
pub mod facet;
pub mod track;

pub use crate::domain::vo::IndexProgress;
pub use cue::{
  AssCue, AssData, AssStyle, Cea608Cue, Cea608Data, CueData, EbuStlCue, EbuStlData, LrcCue,
  LrcData, LrcMetadata, LrcWord, MicroDvdCue, MicroDvdData, PgsCue, PgsData, SamiCue, SamiData,
  SamiStyle, SbvCue, SbvData, SrtCue, SrtData, SubViewerCue, SubViewerData, SubtitleCue,
  SubtitleCueDetails, SubtitleCueError, SubtitleCueKind, TtmlCue, TtmlData, TtmlRegion, TtmlStyle,
  VobSubCue, VobSubData, VobSubPalette, VttCue, VttData, VttLineAlign, VttPositionAlign, VttRegion,
  VttStyleBlock, VttTextAlign, VttVertical,
};
pub use facet::{Subtitle, SubtitleError};
pub use track::{SubtitleTrack, SubtitleTrackError};
