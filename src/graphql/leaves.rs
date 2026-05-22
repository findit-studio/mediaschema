//! GraphQL exposure of the standalone aggregates:
//! [`Speaker`], [`WatchedLocation`], [`UserTag`], [`SceneAnnotation`].

use async_graphql::{Object, ID};

use crate::domain::{SceneAnnotation, Speaker, UserTag, Uuid7, WatchedLocation};

use super::{
  enums::GqlScanStatus,
  media::{GqlErrorInfo, GqlRgba},
  scalars::{empty_as_none, GqlJiffTimestamp, GqlMediaTimestamp},
};

// ---------------------------------------------------------------------------
// Speaker
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`Speaker`].
#[derive(Debug, Clone)]
pub struct GqlSpeaker(pub Speaker<Uuid7>);

impl From<Speaker<Uuid7>> for GqlSpeaker {
  #[inline]
  fn from(v: Speaker<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlSpeaker> for Speaker<Uuid7> {
  #[inline]
  fn from(v: GqlSpeaker) -> Self {
    v.0
  }
}

#[Object(name = "Speaker")]
impl GqlSpeaker {
  async fn id(&self) -> ID {
    ID(self.0.id_ref().to_string())
  }
  async fn parent(&self) -> ID {
    ID(self.0.parent_ref().to_string())
  }
  async fn cluster_id(&self) -> u32 {
    self.0.cluster_id()
  }
  async fn name(&self) -> Option<String> {
    empty_as_none(self.0.name())
  }
  async fn speech_duration(&self) -> Option<GqlMediaTimestamp> {
    self.0.speech_duration_ref().copied().map(GqlMediaTimestamp)
  }
}

// ---------------------------------------------------------------------------
// WatchedLocation
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`WatchedLocation`].
#[derive(Debug, Clone)]
pub struct GqlWatchedLocation(pub WatchedLocation<Uuid7>);

impl From<WatchedLocation<Uuid7>> for GqlWatchedLocation {
  #[inline]
  fn from(v: WatchedLocation<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlWatchedLocation> for WatchedLocation<Uuid7> {
  #[inline]
  fn from(v: GqlWatchedLocation) -> Self {
    v.0
  }
}

#[Object(name = "WatchedLocation")]
impl GqlWatchedLocation {
  async fn id(&self) -> ID {
    ID(self.0.id_ref().to_string())
  }
  /// Stable id of the monitored storage volume (`WatchedLocation` is
  /// volume-scoped — the per-folder watch is application-layer config).
  async fn volume(&self) -> ID {
    ID(self.0.volume_ref().to_string())
  }
  async fn is_recursive(&self) -> bool {
    self.0.is_recursive()
  }
  async fn is_enabled(&self) -> bool {
    self.0.is_enabled()
  }
  async fn is_ejectable(&self) -> bool {
    self.0.is_ejectable()
  }
  async fn added_at(&self) -> GqlJiffTimestamp {
    GqlJiffTimestamp(*self.0.added_at_ref())
  }
  async fn last_reconciled_at(&self) -> Option<GqlJiffTimestamp> {
    self.0.last_reconciled_at_ref().copied().map(GqlJiffTimestamp)
  }
  async fn last_reconcile_status(&self) -> Option<GqlScanStatus> {
    self.0.last_reconcile_status_ref().copied().map(Into::into)
  }
  async fn last_error(&self) -> Option<GqlErrorInfo> {
    self.0.last_error_ref().cloned().map(GqlErrorInfo)
  }
}

// ---------------------------------------------------------------------------
// UserTag
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`UserTag`].
#[derive(Debug, Clone)]
pub struct GqlUserTag(pub UserTag<Uuid7>);

impl From<UserTag<Uuid7>> for GqlUserTag {
  #[inline]
  fn from(v: UserTag<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlUserTag> for UserTag<Uuid7> {
  #[inline]
  fn from(v: GqlUserTag) -> Self {
    v.0
  }
}

#[Object(name = "UserTag")]
impl GqlUserTag {
  async fn id(&self) -> ID {
    ID(self.0.id_ref().to_string())
  }
  async fn name(&self) -> String {
    self.0.name().to_string()
  }
  async fn color(&self) -> Option<GqlRgba> {
    self.0.color().map(GqlRgba)
  }
  async fn created_at(&self) -> GqlJiffTimestamp {
    GqlJiffTimestamp(*self.0.created_at_ref())
  }
}

// ---------------------------------------------------------------------------
// SceneAnnotation
// ---------------------------------------------------------------------------

/// GraphQL wrapper for [`SceneAnnotation`].
#[derive(Debug, Clone)]
pub struct GqlSceneAnnotation(pub SceneAnnotation<Uuid7>);

impl From<SceneAnnotation<Uuid7>> for GqlSceneAnnotation {
  #[inline]
  fn from(v: SceneAnnotation<Uuid7>) -> Self {
    Self(v)
  }
}
impl From<GqlSceneAnnotation> for SceneAnnotation<Uuid7> {
  #[inline]
  fn from(v: GqlSceneAnnotation) -> Self {
    v.0
  }
}

#[Object(name = "SceneAnnotation")]
impl GqlSceneAnnotation {
  async fn id(&self) -> ID {
    ID(self.0.id_ref().to_string())
  }
  async fn scene(&self) -> ID {
    ID(self.0.scene_ref().to_string())
  }
  async fn is_favorite(&self) -> bool {
    self.0.is_favorite()
  }
  async fn user_tags(&self) -> std::vec::Vec<ID> {
    self
      .0
      .user_tags_slice()
      .iter()
      .map(|id| ID(id.to_string()))
      .collect()
  }
  async fn rating(&self) -> Option<u32> {
    self.0.rating().map(u32::from)
  }
  async fn note(&self) -> Option<String> {
    empty_as_none(self.0.note())
  }
  async fn updated_at(&self) -> GqlJiffTimestamp {
    GqlJiffTimestamp(*self.0.updated_at_ref())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn speaker_wrapper_roundtrips() {
    let id = Uuid7::new();
    let parent = Uuid7::new();
    let s = Speaker::try_new(id, parent, 0, "Jane").unwrap();
    let g: GqlSpeaker = s.clone().into();
    let back: Speaker<Uuid7> = g.into();
    assert_eq!(back.name(), s.name());
  }
}
