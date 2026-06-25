//! SQLite row mapping. `Uuid7` rides as a 16-byte `BLOB`, `FileChecksum`
//! as a 32-byte `BLOB`. Nested VOs (`Provenance`, capture `Device`, etc.)
//! are stored as `TEXT` containing JSON.
//!
//! See the module-level [`super`] doc for the cross-backend mapping
//! conventions and current coverage scope.

#[cfg(feature = "audio")]
#[cfg_attr(docsrs, doc(cfg(feature = "audio")))]
pub mod audio;
// `Attachment` is container-level (like `chapter` / `media`) — ungated.
pub mod attachment;
pub mod chapter;
// `Data` is container-level (like `chapter` / `media`) — ungated.
pub mod data;
pub mod leaves;
pub mod media;
pub mod media_file;
pub mod person;
#[cfg(feature = "subtitle")]
#[cfg_attr(docsrs, doc(cfg(feature = "subtitle")))]
pub mod subtitle;
#[cfg(feature = "video")]
#[cfg_attr(docsrs, doc(cfg(feature = "video")))]
pub mod thumbnail;
#[cfg(feature = "video")]
#[cfg_attr(docsrs, doc(cfg(feature = "video")))]
pub mod video;

pub use attachment::{
  attachment_track_from_rows, SqliteAttachmentRow, SqliteAttachmentTrackIndexErrorRow,
  SqliteAttachmentTrackMetadataRow, SqliteAttachmentTrackRow,
};
#[cfg(feature = "audio")]
pub use audio::{
  audio_track_from_rows, sound_event_from_row, SqliteAudioRow, SqliteAudioSegmentRow,
  SqliteAudioSegmentWordRow, SqliteAudioTrackIndexErrorRow, SqliteAudioTrackMetadataRow,
  SqliteAudioTrackRow, SqliteSoundEventRow,
};
pub use chapter::{chapter_from_rows, SqliteChapterMetadataRow, SqliteChapterRow};
pub use data::{
  data_track_from_rows, SqliteDataRow, SqliteDataTrackIndexErrorRow, SqliteDataTrackMetadataRow,
  SqliteDataTrackRow,
};
pub use leaves::{
  SqliteSceneAnnotationRow, SqliteSpeakerRow, SqliteUserTagRow, SqliteWatchedLocationRow,
};
pub use media::SqliteMediaRow;
pub use media_file::SqliteMediaFileRow;
pub use person::SqlitePersonRow;
#[cfg(feature = "subtitle")]
pub use subtitle::{
  subtitle_track_from_rows, SqliteSubtitleCueAssRow, SqliteSubtitleCueBaseRow,
  SqliteSubtitleCueLrcRow, SqliteSubtitleCueLrcWordRow, SqliteSubtitleCueVttRow, SqliteSubtitleRow,
  SqliteSubtitleTrackAssStyleRow, SqliteSubtitleTrackIndexErrorRow,
  SqliteSubtitleTrackLrcMetadataRow, SqliteSubtitleTrackMetadataRow, SqliteSubtitleTrackRow,
  SqliteSubtitleTrackVttRegionRow, SqliteSubtitleTrackVttStyleRow,
};
#[cfg(feature = "video")]
pub use thumbnail::SqliteThumbnailRow;
#[cfg(feature = "video")]
pub use video::{
  cover_keyframe_from_rows, cover_keyframe_rows_from, video_track_from_rows,
  SqliteCoverKeyframeRow, SqliteCoverKeyframeRows, SqliteKeyframeActionRow,
  SqliteKeyframeBarcodeRow, SqliteKeyframeBodyPose3DJointRow, SqliteKeyframeBodyPose3DRow,
  SqliteKeyframeBodyPoseJointRow, SqliteKeyframeBodyPoseRow, SqliteKeyframeClassificationRow,
  SqliteKeyframeColorRow, SqliteKeyframeDocumentSegmentRow, SqliteKeyframeFaceLandmarkPointRow,
  SqliteKeyframeFaceLandmarkRegionRow, SqliteKeyframeFaceLandmarksRow, SqliteKeyframeFaceRow,
  SqliteKeyframeHandPoseRow, SqliteKeyframeMaskRow, SqliteKeyframeObjectRow, SqliteKeyframeRow,
  SqliteKeyframeRows, SqliteKeyframeSaliencyRow, SqliteKeyframeSubjectRow,
  SqliteKeyframeTextDetectionRow, SqliteKeyframeVlmLabelRow, SqliteSceneRow, SqliteVideoRow,
  SqliteVideoTrackIndexErrorRow, SqliteVideoTrackMetadataRow, SqliteVideoTrackRow,
};

/// Canonical SQLite DDL for the mediaschema tables this revision maps.
///
/// Sourced from [`schema.sql`](./schema.sql) so the DDL is text-grep-able
/// alongside the row structs.
pub const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Initial migration: the `CREATE TABLE` baseline, embedded as a static
/// string so consumers can wire it into their migration runner. Frozen —
/// schema growth lands as an additive migration after it (see
/// [`MIGRATION_0002_PROVENANCE_BACKEND_PLATFORM`]) so a database that
/// already applied this file upgrades cleanly. Applying every
/// `MIGRATION_*` in order yields the schema in [`SCHEMA_SQL`].
pub const MIGRATION_0001_INIT: &str = include_str!("migrations/0001_init.sql");

/// Additive migration: adds the voiceprint provenance backend + host
/// platform columns to `speaker` (`ALTER TABLE ... ADD COLUMN`, all
/// nullable). Apply after [`MIGRATION_0001_INIT`] to upgrade an existing
/// database; [`SCHEMA_SQL`] already includes these columns for a fresh
/// create.
pub const MIGRATION_0002_PROVENANCE_BACKEND_PLATFORM: &str =
  include_str!("migrations/0002_provenance_backend_platform.sql");

/// The lines present in the canonical [`SCHEMA_SQL`] but absent from the
/// frozen `0001` baseline — i.e. the textual delta the additive `0002`
/// migration is responsible for. Derived from the two sources (rather
/// than hand-copied) so the mirror invariant is whitespace-robust.
#[cfg(test)]
fn schema_lines_added_since_0001() -> Vec<&'static str> {
  let baseline: std::collections::HashSet<&str> = MIGRATION_0001_INIT.lines().collect();
  SCHEMA_SQL
    .lines()
    .filter(|line| !baseline.contains(line))
    .collect()
}

#[cfg(all(test, feature = "video"))]
mod schema_tests {
  use super::{
    schema_lines_added_since_0001, MIGRATION_0001_INIT, MIGRATION_0002_PROVENANCE_BACKEND_PLATFORM,
    SCHEMA_SQL,
  };

  #[test]
  fn schema_has_thumbnail_table_and_keyframe_fk() {
    // The thumbnail FK target exists with its storage discriminator.
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS thumbnail"));
    assert!(SCHEMA_SQL.contains("kind      TEXT    NOT NULL"));
    // Keyframe references the thumbnail by FK and no longer inlines the
    // image bytes / mime.
    assert!(SCHEMA_SQL.contains("thumbnail_id               BLOB    NOT NULL"));
    assert!(SCHEMA_SQL.contains("idx_keyframe_thumbnail_id"));
    assert!(!SCHEMA_SQL.contains("data                       BLOB    NOT NULL"));
    assert!(!SCHEMA_SQL.contains("mime                       TEXT    NOT NULL"));
    // The thumbnail table is declared BEFORE keyframe (FK target first).
    let thumb = SCHEMA_SQL
      .find("CREATE TABLE IF NOT EXISTS thumbnail")
      .expect("thumbnail table present");
    let keyframe = SCHEMA_SQL
      .find("CREATE TABLE IF NOT EXISTS keyframe (")
      .expect("keyframe table present");
    assert!(thumb < keyframe, "thumbnail must precede keyframe");
  }

  #[test]
  fn schema_has_cover_keyframe_and_role_and_nullable_scene_id() {
    // The cover keyframe is a distinct table parented by video_id.
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS cover_keyframe ("));
    assert!(SCHEMA_SQL.contains("idx_cover_keyframe_video_id"));
    assert!(SCHEMA_SQL.contains("idx_cover_keyframe_thumbnail_id"));
    // The video facet gains the cover FK.
    assert!(SCHEMA_SQL.contains("cover_keyframe_id      BLOB"));
    // The scene keyframe gains a role column and its scene_id is now
    // nullable (the cover decision-b override).
    assert!(SCHEMA_SQL.contains("role                       TEXT    NOT NULL DEFAULT 'scene'"));
    assert!(!SCHEMA_SQL.contains("scene_id                     BLOB    NOT NULL"));
  }

  /// The `0001` baseline plus the additive `0002` columns compose to the
  /// canonical [`SCHEMA_SQL`]. `0001` is frozen at the pre-provenance shape
  /// (so already-migrated databases stay valid); `0002` adds exactly the
  /// four provenance backend/platform columns; and the *only* schema lines
  /// the canonical DDL has beyond the frozen baseline are that block —
  /// every such line names one of the four new columns (or its comment).
  #[test]
  fn migrations_compose_to_schema() {
    // 0001 is the frozen baseline: it must NOT carry the provenance columns.
    assert!(
      !MIGRATION_0001_INIT.contains("voiceprint_provenance_backend"),
      "0001 must stay frozen at the pre-provenance baseline",
    );
    // 0002 adds exactly the four columns, additively.
    for col in [
      "voiceprint_provenance_backend",
      "voiceprint_provenance_platform_os",
      "voiceprint_provenance_platform_arch",
      "voiceprint_provenance_platform_os_version",
    ] {
      assert!(
        MIGRATION_0002_PROVENANCE_BACKEND_PLATFORM
          .contains(&format!("ALTER TABLE speaker ADD COLUMN {col}")),
        "0002 must ALTER-add {col}",
      );
      assert!(
        SCHEMA_SQL.contains(col),
        "fresh-create schema must define {col}"
      );
    }
    // The ONLY non-blank lines the canonical schema has beyond the frozen
    // baseline are the relocated provenance block: each adds a backend/
    // platform column or one of its explanatory comment lines. The `person_id`
    // line also differs — fresh-create now appends the provenance columns
    // AFTER it (to match the additive 0002 ADD-COLUMN order), so it gained a
    // trailing comma — but it is the same column, not drift. Nothing else has
    // changed between the fresh-create schema and the migration baseline.
    let added: Vec<&str> = schema_lines_added_since_0001()
      .into_iter()
      .filter(|l| !l.trim().is_empty())
      .collect();
    assert!(
      !added.is_empty(),
      "the provenance columns must be the delta"
    );
    for line in &added {
      let l = line.trim();
      assert!(
        l.contains("voiceprint_provenance_")
          || l.starts_with("person_id")
          || l.starts_with("-- Inference backend")
          || l.starts_with("-- NULL = not recorded")
          || l.starts_with("-- forward-compatible")
          || l.starts_with("-- Appended AFTER person_id")
          || l.starts_with("-- the additive 0002")
          || l.starts_with("-- (ADD COLUMN"),
        "unexpected schema drift beyond the provenance block: {line:?}",
      );
    }
  }

  /// Upgrade-safety: every provenance backend/platform column the
  /// [`SqliteSpeakerRow`](super::SqliteSpeakerRow) mapper now reads is
  /// supplied to a pre-existing database by the additive `0002`
  /// migration — never only by an in-place edit to the frozen `0001`. A
  /// column the mapper expects but no migration adds would surface as a
  /// missing-column error on a database created before this revision.
  #[test]
  fn upgrade_path_supplies_every_new_speaker_column() {
    // The four columns the row mapper gained this revision.
    let mapper_columns = [
      "voiceprint_provenance_backend",
      "voiceprint_provenance_platform_os",
      "voiceprint_provenance_platform_arch",
      "voiceprint_provenance_platform_os_version",
    ];
    for col in mapper_columns {
      // A fresh create gets it from the canonical schema…
      assert!(
        SCHEMA_SQL.contains(col),
        "fresh-create schema must define {col}",
      );
      // …and an existing database gets it from the additive 0002 migration,
      // NOT from a (frozen) 0001 it may already have applied.
      assert!(
        !MIGRATION_0001_INIT.contains(col),
        "{col} must not be back-edited into the frozen 0001",
      );
      assert!(
        MIGRATION_0002_PROVENANCE_BACKEND_PLATFORM.contains(col),
        "existing databases must receive {col} via 0002",
      );
    }
  }
}

#[cfg(test)]
mod data_schema_tests {
  use super::{schema_lines_added_since_0001, MIGRATION_0001_INIT, SCHEMA_SQL};

  #[test]
  fn schema_has_data_cluster_tables() {
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS data ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS data_track ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS data_track_metadata ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS data_track_index_error ("));
    assert!(SCHEMA_SQL.contains("idx_data_track_data_id"));
  }

  #[test]
  fn schema_has_attachment_cluster_tables() {
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS attachment ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS attachment_track ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS attachment_track_metadata ("));
    assert!(SCHEMA_SQL.contains("CREATE TABLE IF NOT EXISTS attachment_track_index_error ("));
    assert!(SCHEMA_SQL.contains("idx_attachment_track_attachment_id"));
    // Reserved blob columns are declared (always NULL in v1).
    assert!(SCHEMA_SQL.contains("blob_uri"));
  }

  #[test]
  fn media_wires_data_and_attachment_facet_fks() {
    // The root `media` row carries the two new nullable facet FKs + indexes,
    // mirroring `video` / `audio` / `subtitle`.
    assert!(SCHEMA_SQL.contains("idx_media_data"));
    assert!(SCHEMA_SQL.contains("idx_media_attachment"));
  }

  /// Mirror invariant from the data-cluster angle: the frozen `0001`
  /// baseline still defines the full data/attachment clusters, and the only
  /// schema lines beyond it are the additive `0002` provenance columns —
  /// the data/attachment DDL has not drifted between the two sources.
  #[test]
  fn data_migration_mirror_matches_schema() {
    assert!(MIGRATION_0001_INIT.contains("CREATE TABLE IF NOT EXISTS data ("));
    assert!(MIGRATION_0001_INIT.contains("CREATE TABLE IF NOT EXISTS attachment ("));
    for line in schema_lines_added_since_0001() {
      // The only deltas are the relocated speaker provenance block: its four
      // columns, its comment lines, and the `person_id` line (now carries a
      // trailing comma since the provenance columns moved after it). The
      // data/attachment cluster DDL itself has not drifted.
      assert!(
        line.trim().is_empty()
          || line.contains("voiceprint_provenance_")
          || line.trim().starts_with("person_id")
          || line.contains("--"),
        "data/attachment schema must not drift from the 0001 baseline: {line:?}",
      );
    }
  }
}

/// The migration-compose invariant at the *physical column-order* level: a
/// `speaker` table created fresh from [`SCHEMA_SQL`] must have byte-for-byte
/// the same column ordinals as one built by applying [`MIGRATION_0001_INIT`]
/// then [`MIGRATION_0002_PROVENANCE_BACKEND_PLATFORM`]. SQLite
/// `ALTER TABLE ... ADD COLUMN` can only *append* the four provenance columns
/// (after `person_id`), so the fresh `CREATE TABLE` must declare them in that
/// same trailing position — otherwise a positional `INSERT ... VALUES(...)`,
/// `SELECT *`, or ordinal decode would map columns differently between a
/// fresh DB and an upgraded one.
#[cfg(test)]
mod column_order_tests {
  use super::{MIGRATION_0001_INIT, MIGRATION_0002_PROVENANCE_BACKEND_PLATFORM, SCHEMA_SQL};

  /// `PRAGMA table_info(speaker)` rendered as `cid|name` lines (one per
  /// column, in ordinal order) by piping `sql` into the `sqlite3` CLI against
  /// a throwaway on-disk database. Returns `None` when the CLI is absent so
  /// the caller can fall back to the textual ordering check.
  fn speaker_table_info(sql: &str) -> Option<String> {
    use std::{
      io::Write,
      process::{Command, Stdio},
    };

    // A unique temp path per call; `sqlite3` creates it on connect.
    let mut db = std::env::temp_dir();
    db.push(format!(
      "mediaschema-speaker-order-{}-{:?}.sqlite",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0),
    ));
    let _ = std::fs::remove_file(&db);

    let script = format!(
      "{sql}\n.mode list\n.separator |\nSELECT cid, name FROM pragma_table_info('speaker') ORDER BY cid;\n"
    );

    let mut child = match Command::new("sqlite3")
      .arg(&db)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
    {
      Ok(c) => c,
      // CLI not installed in this environment — signal the fallback path.
      Err(_) => return None,
    };
    child
      .stdin
      .take()
      .expect("stdin piped")
      .write_all(script.as_bytes())
      .expect("write sqlite3 script");
    let out = child.wait_with_output().expect("sqlite3 runs");
    let _ = std::fs::remove_file(&db);
    assert!(
      out.status.success(),
      "sqlite3 failed: {}",
      String::from_utf8_lossy(&out.stderr),
    );
    Some(String::from_utf8(out.stdout).expect("utf8 pragma output"))
  }

  /// The ordered list of `speaker` column names as they textually appear in a
  /// `CREATE TABLE speaker (...)` block — the in-process fallback when the
  /// `sqlite3` CLI is unavailable. Parses leading identifiers, skipping
  /// comment / constraint lines.
  fn speaker_columns_textual(ddl: &str) -> Vec<String> {
    let start = ddl
      .find("CREATE TABLE IF NOT EXISTS speaker (")
      .expect("speaker table present");
    let body = &ddl[start..];
    let open = body.find('(').expect("open paren");
    let close = body.find(");").expect("close paren");
    let inner = &body[open + 1..close];
    inner
      .lines()
      .map(str::trim)
      .filter(|l| !l.is_empty() && !l.starts_with("--"))
      .filter_map(|l| {
        let ident: String = l
          .chars()
          .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
          .collect();
        // Drop table-level constraint clauses (PRIMARY KEY / FOREIGN KEY / …);
        // the speaker table has none, but keep the parser honest.
        if ident.is_empty()
          || matches!(
            ident.as_str(),
            "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK" | "CONSTRAINT"
          )
        {
          None
        } else {
          Some(ident)
        }
      })
      .collect()
  }

  /// Fresh `SCHEMA_SQL` and `0001`+`0002` must produce identical `speaker`
  /// column ordinals (names AND order). Uses the `sqlite3` CLI when present
  /// (authoritative `PRAGMA table_info`), else falls back to comparing the
  /// textual column order of `SCHEMA_SQL` against the order the additive
  /// migration would yield (`0001` body columns, then the four appended).
  #[test]
  fn fresh_schema_matches_upgraded_speaker_column_order() {
    if let (Some(fresh), Some(upgraded)) = (
      speaker_table_info(SCHEMA_SQL),
      speaker_table_info(&format!(
        "{MIGRATION_0001_INIT}\n{MIGRATION_0002_PROVENANCE_BACKEND_PLATFORM}"
      )),
    ) {
      assert_eq!(
        fresh, upgraded,
        "speaker column ordinals diverge between a fresh SCHEMA_SQL create and a 0001+0002 upgrade:\nfresh:\n{fresh}\nupgraded:\n{upgraded}",
      );
      // Guard against the parse silently matching two empty results.
      assert!(
        fresh.contains("voiceprint_provenance_backend"),
        "PRAGMA output must include the new provenance columns: {fresh}",
      );
      // The four provenance columns must be the LAST four ordinals (appended
      // after person_id) in both shapes.
      let last_four: Vec<&str> = fresh
        .lines()
        .rev()
        .take(4)
        .map(|l| l.split('|').nth(1).unwrap_or(""))
        .collect();
      for col in [
        "voiceprint_provenance_platform_os_version",
        "voiceprint_provenance_platform_arch",
        "voiceprint_provenance_platform_os",
        "voiceprint_provenance_backend",
      ] {
        assert!(
          last_four.contains(&col),
          "{col} must be among the trailing four speaker columns: {fresh}",
        );
      }
      let person_idx = fresh
        .lines()
        .position(|l| l.split('|').nth(1) == Some("person_id"))
        .expect("person_id column present");
      let backend_idx = fresh
        .lines()
        .position(|l| l.split('|').nth(1) == Some("voiceprint_provenance_backend"))
        .expect("backend column present");
      assert!(
        person_idx < backend_idx,
        "person_id must precede the appended provenance columns: {fresh}",
      );
    } else {
      // Fallback: build the column order the additive migration produces —
      // the 0001 speaker columns in declaration order, then the four columns
      // 0002 appends — and assert SCHEMA_SQL declares exactly that sequence.
      let fresh = speaker_columns_textual(SCHEMA_SQL);
      let mut upgraded = speaker_columns_textual(MIGRATION_0001_INIT);
      upgraded.extend(
        [
          "voiceprint_provenance_backend",
          "voiceprint_provenance_platform_os",
          "voiceprint_provenance_platform_arch",
          "voiceprint_provenance_platform_os_version",
        ]
        .into_iter()
        .map(String::from),
      );
      assert_eq!(
        fresh, upgraded,
        "speaker column order in SCHEMA_SQL must equal the 0001+0002 (append) order",
      );
      assert_eq!(
        upgraded.last().map(String::as_str),
        Some("voiceprint_provenance_platform_os_version"),
      );
    }
  }
}
