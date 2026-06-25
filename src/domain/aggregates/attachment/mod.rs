//! Attachment media-facet cluster: the [`Attachment`] facet + the
//! [`AttachmentTrack`] track aggregate + the reserved [`BlobRef`]
//! externalization handle.
//!
//! Attachment tracks carry container-level **attachment** streams (FFmpeg
//! `codec_type=attachment`): fonts, cover art, thumbnails. v1 records
//! **presence + a descriptor + container metadata only** — the
//! `AttachmentTrack::blob: Option<BlobRef>` slot is reserved and **never
//! populated** (no attachment bytes are stored). The facet is the
//! attachment analog of [`Audio`](super::audio) / [`Subtitle`](super::subtitle) /
//! [`Data`](super::data).

pub mod blob;
pub mod facet;
pub mod track;

pub use blob::{BlobRef, BlobRefError};
pub use facet::{Attachment, AttachmentError, AttachmentParts};
pub use track::{AttachmentTrack, AttachmentTrackError, AttachmentTrackParts};
