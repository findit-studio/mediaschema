//! Stub `Query` root.
//!
//! The resolvers all return `None` / empty `Vec`s. `mediaschema` is a
//! schema crate, not a runtime — a real GraphQL server plugs its own
//! data source in via `Schema::data(…)` and overrides this root. The
//! stub exists so the schema can be `build()`-ed and an SDL emitted.

use async_graphql::{Object, ID};

use super::{
  audio::{GqlAudio, GqlAudioSegment, GqlAudioTrack},
  enums::{GqlAudioIndexStage, GqlSubtitleIndexStage, GqlVideoIndexStage},
  leaves::{GqlSceneAnnotation, GqlSpeaker, GqlUserTag, GqlWatchedLocation},
  media::GqlMedia,
  subtitle::{GqlSubtitle, GqlSubtitleCue, GqlSubtitleTrack},
  video::{GqlKeyframe, GqlScene, GqlVideo, GqlVideoTrack},
};

/// GraphQL Query root for `mediaschema`. Every field returns `None` /
/// empty in this stub; downstream servers override / shadow it.
#[derive(Default)]
pub struct Query;

#[Object]
impl Query {
  async fn media(&self, _id: ID) -> Option<GqlMedia> {
    None
  }
  async fn video(&self, _id: ID) -> Option<GqlVideo> {
    None
  }
  async fn video_track(&self, _id: ID) -> Option<GqlVideoTrack> {
    None
  }
  async fn scene(&self, _id: ID) -> Option<GqlScene> {
    None
  }
  async fn keyframe(&self, _id: ID) -> Option<GqlKeyframe> {
    None
  }
  async fn audio(&self, _id: ID) -> Option<GqlAudio> {
    None
  }
  async fn audio_track(&self, _id: ID) -> Option<GqlAudioTrack> {
    None
  }
  async fn audio_segment(&self, _id: ID) -> Option<GqlAudioSegment> {
    None
  }
  async fn subtitle(&self, _id: ID) -> Option<GqlSubtitle> {
    None
  }
  async fn subtitle_track(&self, _id: ID) -> Option<GqlSubtitleTrack> {
    None
  }
  async fn subtitle_cue(&self, _id: ID) -> Option<GqlSubtitleCue> {
    None
  }
  async fn speaker(&self, _id: ID) -> Option<GqlSpeaker> {
    None
  }
  async fn watched_location(&self, _id: ID) -> Option<GqlWatchedLocation> {
    None
  }
  async fn user_tag(&self, _id: ID) -> Option<GqlUserTag> {
    None
  }
  async fn scene_annotation(&self, _id: ID) -> Option<GqlSceneAnnotation> {
    None
  }

  // List endpoints — empty in this stub.

  async fn media_list(&self) -> std::vec::Vec<GqlMedia> {
    std::vec::Vec::new()
  }
  async fn watched_locations(&self) -> std::vec::Vec<GqlWatchedLocation> {
    std::vec::Vec::new()
  }
  async fn user_tags(&self) -> std::vec::Vec<GqlUserTag> {
    std::vec::Vec::new()
  }

  // ---- Surface anchors for derived enums ----
  //
  // The coarse stage enums are **derived** from `index_status` +
  // `index_errors` per the locked schema, so no aggregate field exposes
  // them directly — async-graphql therefore drops them from the SDL.
  // Anchor each one as a helper field so the type appears in the
  // schema and downstream clients can codegen against it.

  /// Derived coarse stage for the given video status / errors. The
  /// real derivation runs on `VideoTrack` server-side; this is a
  /// schema anchor so the enum is reachable in the SDL.
  async fn video_index_stage_for(&self) -> Option<GqlVideoIndexStage> {
    None
  }

  /// Derived coarse stage for the given audio status / errors.
  async fn audio_index_stage_for(&self) -> Option<GqlAudioIndexStage> {
    None
  }

  /// Derived coarse stage for the given subtitle status / errors.
  async fn subtitle_index_stage_for(&self) -> Option<GqlSubtitleIndexStage> {
    None
  }
}
