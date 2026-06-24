-- mediaschema — SQLite DDL (canonical).
--
-- Identity columns are 16-byte BLOBs (Uuid7).
-- Checksum columns are 32-byte BLOBs.
-- Nested value-objects are flattened into real, individually-indexable
-- columns; many-to-many collections ride in dedicated join tables.
-- Wall-clock timestamps are INTEGER ms-since-epoch.

CREATE TABLE IF NOT EXISTS media (
    id                  BLOB    NOT NULL PRIMARY KEY,
    checksum            BLOB    NOT NULL,
    format              TEXT    NOT NULL,
    size                INTEGER NOT NULL,
    duration_raw        INTEGER,
    kind                INTEGER NOT NULL,        -- 0=Video, 1=Audio
    -- Verbatim AVFormatContext.nb_streams / nb_chapters (rev 11).
    nb_streams          INTEGER NOT NULL DEFAULT 0,
    nb_chapters         INTEGER NOT NULL DEFAULT 0,
    video               BLOB,
    audio               BLOB,
    subtitle            BLOB,
    error_flags         INTEGER NOT NULL DEFAULT 0,
    probe_error_code    INTEGER,
    probe_error_message TEXT,
    capture_date_ms     INTEGER,
    device_make         TEXT,
    device_model        TEXT,
    gps_lat             REAL,
    gps_lon             REAL,
    gps_altitude        REAL
);

-- Container-level chapters (AVFormatContext.chapters[i]). See
-- schema/chapter.md (rev 1). `title` hoisted from AVDictionary's "title"
-- key; remaining metadata in `chapter_metadata`, keyed by ordinal.
CREATE TABLE IF NOT EXISTS chapter (
    id                  BLOB    NOT NULL PRIMARY KEY,
    media_id            BLOB    NOT NULL,
    chapter_index       INTEGER NOT NULL,
    source_id           INTEGER NOT NULL,
    start_pts           INTEGER NOT NULL,
    end_pts             INTEGER NOT NULL,
    timebase_num        INTEGER NOT NULL,
    timebase_den        INTEGER NOT NULL,
    title               TEXT    NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_chapter_media_id_index ON chapter(media_id, chapter_index);
CREATE INDEX        IF NOT EXISTS idx_chapter_media_id       ON chapter(media_id);
CREATE INDEX        IF NOT EXISTS idx_chapter_title_lower    ON chapter(LOWER(title));

-- AVDictionary entries per chapter, **excluding** the "title" key.
-- `ordinal` preserves IndexMap insertion order.
CREATE TABLE IF NOT EXISTS chapter_metadata (
    chapter_id  BLOB    NOT NULL,
    ordinal     INTEGER NOT NULL,
    key         TEXT    NOT NULL,
    value       TEXT    NOT NULL,
    PRIMARY KEY (chapter_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_chapter_metadata_chapter_id ON chapter_metadata(chapter_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_media_checksum ON media(checksum);
CREATE INDEX        IF NOT EXISTS idx_media_video    ON media(video);
CREATE INDEX        IF NOT EXISTS idx_media_audio    ON media(audio);
CREATE INDEX        IF NOT EXISTS idx_media_subtitle ON media(subtitle);

CREATE TABLE IF NOT EXISTS watched_location (
    id                    BLOB    NOT NULL PRIMARY KEY,
    volume                BLOB    NOT NULL UNIQUE,
    recursive             INTEGER NOT NULL DEFAULT 0,
    enabled               INTEGER NOT NULL DEFAULT 0,
    is_ejectable          INTEGER NOT NULL DEFAULT 0,
    added_at_ms           INTEGER NOT NULL,
    last_reconciled_at_ms INTEGER,
    last_reconcile_status INTEGER,
    last_error_code       INTEGER,
    last_error_message    TEXT
);

CREATE TABLE IF NOT EXISTS media_file (
    id                  BLOB    NOT NULL PRIMARY KEY,
    media_id            BLOB    NOT NULL,    -- FK -> media.id
    created_at_ms       INTEGER,            -- filesystem birth time; NULL = absent
    location_volume     BLOB    NOT NULL,
    location_path       TEXT    NOT NULL,   -- path components joined by '/'
    watched_location_id BLOB    NOT NULL,   -- FK -> watched_location.id
    watch_volume        BLOB    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_file_media_id            ON media_file(media_id);
CREATE INDEX IF NOT EXISTS idx_media_file_watched_location_id ON media_file(watched_location_id);
-- Natural-key uniqueness of a copy: one path per volume. SQLite
-- UNIQUE-indexes TEXT natively, so the index uses the path column
-- directly; the mysql dialect carries an extra `location_path_hash`
-- column to dodge InnoDB's prefix-length requirement on variable-length
-- TEXT and indexes `(location_volume, location_path_hash)` there.
CREATE UNIQUE INDEX IF NOT EXISTS idx_media_file_path
    ON media_file(location_volume, location_path);

CREATE TABLE IF NOT EXISTS speaker (
    id                                    BLOB    NOT NULL PRIMARY KEY,
    audio_track_id                        BLOB    NOT NULL,    -- FK -> audio_track.id
    cluster_id                            INTEGER NOT NULL,
    name                                  TEXT    NOT NULL,
    speech_duration_ms                    INTEGER,
    -- Per-track aggregated voiceprint. `voiceprint_vector_id IS NOT NULL`
    -- is the discriminator: when present, the other voiceprint_* columns
    -- carry the full flattened VO; when NULL, they are all NULL.
    voiceprint_vector_id                  BLOB,
    voiceprint_dimensions                 INTEGER,
    voiceprint_extracted_at_ms            INTEGER,
    voiceprint_confidence                 REAL,
    voiceprint_provenance_model_name      TEXT,
    voiceprint_provenance_model_version   TEXT,
    voiceprint_provenance_prompt_version  TEXT,
    voiceprint_provenance_indexer_version TEXT,
    -- Cross-track identity FK -> person.id; NULL = not yet identified.
    person_id                             BLOB
);
CREATE INDEX IF NOT EXISTS idx_speaker_audio_track_id ON speaker(audio_track_id);
CREATE INDEX IF NOT EXISTS idx_speaker_person_id ON speaker(person_id);

CREATE TABLE IF NOT EXISTS user_tag (
    id            BLOB    NOT NULL PRIMARY KEY,
    name          TEXT    NOT NULL,
    color_rgba    INTEGER,
    created_at_ms INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_tag_name ON user_tag(name);

CREATE TABLE IF NOT EXISTS scene_annotation (
    id              BLOB    NOT NULL PRIMARY KEY,
    scene_id           BLOB    NOT NULL,        -- FK -> scene.id
    favorite        INTEGER NOT NULL DEFAULT 0,
    rating          INTEGER,
    note            TEXT    NOT NULL DEFAULT '',
    updated_at_ms   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scene_annotation_scene_id ON scene_annotation(scene_id);

CREATE TABLE IF NOT EXISTS scene_annotation_user_tag (
    scene_annotation_id  BLOB    NOT NULL,
    user_tag_id          BLOB    NOT NULL,
    ordinal           INTEGER NOT NULL,
    PRIMARY KEY (scene_annotation_id, user_tag_id)
);
CREATE INDEX IF NOT EXISTS idx_saut_user_tag_id ON scene_annotation_user_tag (user_tag_id);

-- Audio-cluster: the `Audio` facet + `AudioTrack` + `AudioSegment`
-- (+ the `Word` / `index_errors` child tables). Nested value-objects are
-- flattened into real columns; collections ride in child tables with an
-- `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS audio (
    id                     BLOB    NOT NULL PRIMARY KEY,
    media_id                 BLOB    NOT NULL UNIQUE,
    total_segments         INTEGER NOT NULL DEFAULT 0,
    track_progress_total   INTEGER NOT NULL DEFAULT 0,
    track_progress_indexed INTEGER NOT NULL DEFAULT 0,
    track_progress_failed  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS audio_track (
    id                        BLOB    NOT NULL PRIMARY KEY,
    audio_id                  BLOB    NOT NULL,   -- FK -> audio.id
    stream_index              INTEGER,
    container_track_id        INTEGER,
    codec                     TEXT    NOT NULL,
    profile                   TEXT    NOT NULL,
    sample_rate               INTEGER NOT NULL DEFAULT 0,
    channels                  INTEGER NOT NULL DEFAULT 0,
    channel_layout            TEXT    NOT NULL,
    -- `SampleFormat::to_u32` (FFmpeg `AV_SAMPLE_FMT_*` code). Default
    -- `4294967295` (`u32::MAX`) so a freshly-inserted row decodes back as
    -- `SampleFormat::Unknown(u32::MAX)` == `SampleFormat::default()`.
    sample_format             INTEGER NOT NULL DEFAULT 4294967295,
    bit_rate                  INTEGER NOT NULL DEFAULT 0,
    bit_rate_mode             INTEGER,
    bits_per_sample           INTEGER,
    is_lossless               INTEGER NOT NULL DEFAULT 0,
    duration_pts              INTEGER,
    duration_tb_num           INTEGER,
    duration_tb_den           INTEGER,
    start_pts                 INTEGER,
    start_pts_tb_num          INTEGER,
    start_pts_tb_den          INTEGER,
    language                  TEXT,
    detected_language         TEXT,
    disposition               INTEGER NOT NULL DEFAULT 0,
    is_primary                INTEGER NOT NULL DEFAULT 0,
    auto_selected             INTEGER NOT NULL DEFAULT 0,
    content                   INTEGER,            -- 0=Speech,1=Music,2=Mixed,3=Silence
    speech_ratio              REAL,
    is_silent                 INTEGER NOT NULL DEFAULT 0,
    has_loudness              INTEGER NOT NULL DEFAULT 0,
    loudness_integrated_lufs  REAL,
    loudness_range_lu         REAL,
    loudness_true_peak_dbtp   REAL,
    loudness_sample_peak_dbfs REAL,
    has_replay_gain           INTEGER NOT NULL DEFAULT 0,
    replay_gain_track_gain_db REAL,
    replay_gain_track_peak    REAL,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak    REAL,
    fingerprint_algo          TEXT,
    fingerprint_value         BLOB,
    isrc                      TEXT    NOT NULL,
    acoustid                  TEXT    NOT NULL,
    musicbrainz_recording_id  TEXT    NOT NULL,
    has_tags                  INTEGER NOT NULL DEFAULT 0,
    tags_title                TEXT,
    tags_artist               TEXT,
    tags_album_artist         TEXT,
    tags_album                TEXT,
    tags_composer             TEXT,
    tags_genre                TEXT,
    tags_comment              TEXT,
    tags_year                 INTEGER,
    tags_track_number         INTEGER,
    tags_track_total          INTEGER,
    tags_disc_number          INTEGER,
    tags_disc_total           INTEGER,
    tags_language             TEXT,
    cover_art_mime            TEXT,
    cover_art_data            BLOB,
    provenance_model_name     TEXT    NOT NULL,
    provenance_model_version  TEXT    NOT NULL,
    provenance_prompt_version TEXT    NOT NULL,
    provenance_indexer_version TEXT   NOT NULL,
    vad_provenance_model_name     TEXT    NOT NULL,
    vad_provenance_model_version  TEXT    NOT NULL,
    vad_provenance_prompt_version TEXT    NOT NULL,
    vad_provenance_indexer_version TEXT   NOT NULL,
    ced_provenance_model_name     TEXT    NOT NULL,
    ced_provenance_model_version  TEXT    NOT NULL,
    ced_provenance_prompt_version TEXT    NOT NULL,
    ced_provenance_indexer_version TEXT   NOT NULL,
    index_status              INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_audio_track_audio_id ON audio_track(audio_id);

-- AVDictionary entries per audio_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS audio_track_metadata (
    audio_track_id  BLOB    NOT NULL,
    ordinal         INTEGER NOT NULL,
    key             TEXT    NOT NULL,
    value           TEXT    NOT NULL,
    PRIMARY KEY (audio_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_audio_track_metadata_audio_track_id ON audio_track_metadata(audio_track_id);

CREATE TABLE IF NOT EXISTS audio_track_index_error (
    audio_track_id BLOB    NOT NULL,   -- FK -> audio_track.id
    ordinal     INTEGER NOT NULL,
    code        INTEGER NOT NULL,
    message     TEXT    NOT NULL,
    PRIMARY KEY (audio_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_atie_audio_track_id ON audio_track_index_error(audio_track_id);

CREATE TABLE IF NOT EXISTS audio_segment (
    id              BLOB    NOT NULL PRIMARY KEY,
    audio_track_id          BLOB    NOT NULL,   -- FK -> audio_track.id
    "index"         INTEGER NOT NULL,
    span_start_pts  INTEGER NOT NULL,
    span_end_pts    INTEGER NOT NULL,
    speaker_id         BLOB,               -- FK -> speaker.id; NULL = not diarized
    text_src        TEXT    NOT NULL,
    text_translated TEXT    NOT NULL,
    language        TEXT,
    no_speech_prob  REAL,
    avg_logprob     REAL,
    temperature     REAL,
    -- Per-segment voice embedding. `voice_fingerprint_vector_id IS NOT NULL`
    -- discriminates presence of the flattened VoiceFingerprint VO.
    voice_fingerprint_vector_id                  BLOB,
    voice_fingerprint_dimensions                 INTEGER,
    voice_fingerprint_extracted_at_ms            INTEGER,
    voice_fingerprint_confidence                 REAL,
    voice_fingerprint_provenance_model_name      TEXT,
    voice_fingerprint_provenance_model_version   TEXT,
    voice_fingerprint_provenance_prompt_version  TEXT,
    voice_fingerprint_provenance_indexer_version TEXT
);
CREATE INDEX IF NOT EXISTS idx_audio_segment_audio_track_id ON audio_segment(audio_track_id);

CREATE TABLE IF NOT EXISTS audio_segment_word (
    audio_segment_id  BLOB    NOT NULL,   -- FK -> audio_segment.id
    ordinal        INTEGER NOT NULL,
    text           TEXT    NOT NULL,
    span_start_pts INTEGER NOT NULL,
    span_end_pts   INTEGER NOT NULL,
    score          REAL    NOT NULL,
    language       TEXT,
    PRIMARY KEY (audio_segment_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_asw_audio_segment_id ON audio_segment_word(audio_segment_id);

-- Data-cluster: the `Data` facet + `DataTrack` (+ the metadata / index_error
-- child tables). Timed-metadata streams (codec_type=data: Sony rtmd / GoPro
-- GPMF / MISB KLV / timecode); presence + descriptor + metadata only — no
-- sample payloads. `codec` / `codec_tag` are plain slugs.

CREATE TABLE IF NOT EXISTS data (
    id                     BLOB    NOT NULL PRIMARY KEY,
    media_id               BLOB    NOT NULL UNIQUE,
    track_progress_total   INTEGER NOT NULL DEFAULT 0,
    track_progress_indexed INTEGER NOT NULL DEFAULT 0,
    track_progress_failed  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS data_track (
    id                 BLOB    NOT NULL PRIMARY KEY,
    data_id            BLOB    NOT NULL,   -- FK -> data.id
    stream_index       INTEGER,
    container_track_id INTEGER,
    codec              TEXT    NOT NULL,
    codec_tag          TEXT    NOT NULL,
    start_pts          INTEGER,
    start_pts_tb_num   INTEGER,
    start_pts_tb_den   INTEGER,
    duration_pts       INTEGER,
    duration_tb_num    INTEGER,
    duration_tb_den    INTEGER,
    nb_packets         INTEGER,
    byte_size          INTEGER NOT NULL DEFAULT 0,
    disposition        INTEGER NOT NULL DEFAULT 0,
    index_status       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_data_track_data_id ON data_track(data_id);

-- AVDictionary entries per data_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS data_track_metadata (
    data_track_id BLOB    NOT NULL,
    ordinal       INTEGER NOT NULL,
    key           TEXT    NOT NULL,
    value         TEXT    NOT NULL,
    PRIMARY KEY (data_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_data_track_metadata_data_track_id ON data_track_metadata(data_track_id);

CREATE TABLE IF NOT EXISTS data_track_index_error (
    data_track_id BLOB    NOT NULL,   -- FK -> data_track.id
    ordinal       INTEGER NOT NULL,
    code          INTEGER NOT NULL,
    message       TEXT    NOT NULL,
    PRIMARY KEY (data_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_dtie_data_track_id ON data_track_index_error(data_track_id);

-- Video-cluster: the `Video` facet + `VideoTrack` + `Scene` + `Keyframe`
-- (+ per-detection child tables). Booleans ride as 0/1 INTEGER.

CREATE TABLE IF NOT EXISTS video (
    id                     BLOB    NOT NULL PRIMARY KEY,
    media_id                 BLOB    NOT NULL UNIQUE,
    total_scenes           INTEGER NOT NULL DEFAULT 0,
    track_progress_total   INTEGER NOT NULL DEFAULT 0,
    track_progress_indexed INTEGER NOT NULL DEFAULT 0,
    track_progress_failed  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS video_track (
    id                       BLOB    NOT NULL PRIMARY KEY,
    video_id                 BLOB    NOT NULL,
    stream_index             INTEGER,
    container_track_id       INTEGER,
    start_pts                INTEGER,
    start_pts_tb_num         INTEGER,
    start_pts_tb_den         INTEGER,
    duration_pts             INTEGER,
    duration_tb_num          INTEGER,
    duration_tb_den          INTEGER,
    codec                    TEXT    NOT NULL,
    profile                  TEXT,
    level                    INTEGER,
    bit_rate                 INTEGER NOT NULL DEFAULT 0,
    nb_frames                INTEGER,
    has_b_frames             INTEGER NOT NULL DEFAULT 0,
    closed_gop               INTEGER,
    bits_per_raw_sample      INTEGER,
    width                    INTEGER NOT NULL DEFAULT 0,
    height                   INTEGER NOT NULL DEFAULT 0,
    has_visible_rect         INTEGER NOT NULL DEFAULT 0,
    visible_rect_x           INTEGER,
    visible_rect_y           INTEGER,
    visible_rect_w           INTEGER,
    visible_rect_h           INTEGER,
    sar_num                  INTEGER NOT NULL DEFAULT 1,
    sar_den                  INTEGER NOT NULL DEFAULT 1,
    pixel_format             INTEGER NOT NULL DEFAULT 0,
    color_primaries          INTEGER NOT NULL DEFAULT 0,
    color_transfer           INTEGER NOT NULL DEFAULT 0,
    color_matrix             INTEGER NOT NULL DEFAULT 0,
    color_range              INTEGER NOT NULL DEFAULT 0,
    color_chroma_location    INTEGER NOT NULL DEFAULT 0,
    has_hdr_static           INTEGER NOT NULL DEFAULT 0,
    hdr_has_mastering        INTEGER NOT NULL DEFAULT 0,
    hdr_primary_r_x          INTEGER,
    hdr_primary_r_y          INTEGER,
    hdr_primary_g_x          INTEGER,
    hdr_primary_g_y          INTEGER,
    hdr_primary_b_x          INTEGER,
    hdr_primary_b_y          INTEGER,
    hdr_white_point_x        INTEGER,
    hdr_white_point_y        INTEGER,
    hdr_max_luminance        INTEGER,
    hdr_min_luminance        INTEGER,
    hdr_has_content_light    INTEGER NOT NULL DEFAULT 0,
    hdr_max_cll              INTEGER,
    hdr_max_fall             INTEGER,
    rotation                 INTEGER NOT NULL DEFAULT 0,
    fr_num                   INTEGER NOT NULL DEFAULT 1,
    fr_den                   INTEGER NOT NULL DEFAULT 1,
    fr_is_vfr                INTEGER NOT NULL DEFAULT 0,
    -- `AVStream.avg_frame_rate` — defaults `0/1` (= `FrameRate::default()`,
    -- absent). For CFR content this equals (fr_num, fr_den); for VFR the
    -- two diverge.
    avg_fr_num               INTEGER NOT NULL DEFAULT 0,
    avg_fr_den               INTEGER NOT NULL DEFAULT 1,
    field_order              INTEGER NOT NULL DEFAULT 0,
    stereo_mode              INTEGER,
    has_dovi                 INTEGER NOT NULL DEFAULT 0,
    dovi_profile             INTEGER,
    dovi_level               INTEGER,
    dovi_rpu_present         INTEGER,
    dovi_el_present          INTEGER,
    dovi_bl_signal_compat_id INTEGER,
    has_embedded_captions    INTEGER NOT NULL DEFAULT 0,
    disposition              INTEGER NOT NULL DEFAULT 0,
    is_primary               INTEGER NOT NULL DEFAULT 0,
    auto_selected            INTEGER NOT NULL DEFAULT 0,
    provenance_model_name    TEXT    NOT NULL,
    provenance_model_version TEXT    NOT NULL,
    provenance_prompt_version TEXT   NOT NULL,
    provenance_indexer_version TEXT  NOT NULL,
    index_status             INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_video_track_video_id ON video_track(video_id);

-- AVDictionary entries per video_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS video_track_metadata (
    video_track_id  BLOB    NOT NULL,
    ordinal         INTEGER NOT NULL,
    key             TEXT    NOT NULL,
    value           TEXT    NOT NULL,
    PRIMARY KEY (video_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_video_track_metadata_video_track_id ON video_track_metadata(video_track_id);

CREATE TABLE IF NOT EXISTS video_track_index_error (
    video_track_id BLOB    NOT NULL,
    ordinal     INTEGER NOT NULL,
    code        INTEGER NOT NULL,
    message     TEXT    NOT NULL,
    PRIMARY KEY (video_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_vtie_video_track_id ON video_track_index_error(video_track_id);

CREATE TABLE IF NOT EXISTS scene (
    id              BLOB    NOT NULL PRIMARY KEY,
    video_track_id          BLOB    NOT NULL,
    "index"         INTEGER NOT NULL,
    span_start_pts  INTEGER NOT NULL,
    span_end_pts    INTEGER NOT NULL,
    detector        TEXT    NOT NULL,
    description     TEXT    NOT NULL
);
CREATE INDEX        IF NOT EXISTS idx_scene_video_track_id   ON scene(video_track_id);
CREATE INDEX        IF NOT EXISTS idx_scene_detector ON scene(detector);
CREATE UNIQUE INDEX IF NOT EXISTS idx_scene_video_track_id_index ON scene(video_track_id, "index");

-- Thumbnail image + storage descriptor. FK target of keyframe.thumbnail_id,
-- so it is declared BEFORE keyframe. `kind` is a ThumbnailKind slug
-- (`filesystem`/`database`/`remote`); exactly one payload slot is
-- populated per kind: `data` (BLOB) for `database`, `location` (TEXT) for
-- `filesystem`/`remote` — the other is NULL.
CREATE TABLE IF NOT EXISTS thumbnail (
    id        BLOB    NOT NULL PRIMARY KEY,
    kind      TEXT    NOT NULL,
    data      BLOB,
    location  TEXT,
    mime      TEXT    NOT NULL,
    width     INTEGER NOT NULL,
    height    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_thumbnail_kind ON thumbnail(kind);

CREATE TABLE IF NOT EXISTS keyframe (
    id                         BLOB    NOT NULL PRIMARY KEY,
    scene_id                     BLOB    NOT NULL,
    pts                        INTEGER NOT NULL,
    thumbnail_id               BLOB    NOT NULL,
    width                      INTEGER NOT NULL,
    height                     INTEGER NOT NULL,
    extractor                  TEXT    NOT NULL,
    vlm_description_src        TEXT    NOT NULL,
    vlm_description_translated TEXT    NOT NULL,
    vlm_shot_type              TEXT    NOT NULL,
    horizon_angle              REAL    NOT NULL,
    horizon_confidence         REAL    NOT NULL,
    aesthetics_overall_score   REAL    NOT NULL,
    aesthetics_is_utility      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_keyframe_scene_id ON keyframe(scene_id);
CREATE INDEX IF NOT EXISTS idx_keyframe_thumbnail_id ON keyframe(thumbnail_id);

CREATE TABLE IF NOT EXISTS keyframe_classification (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    label      TEXT    NOT NULL,
    confidence REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_classification_keyframe_id ON keyframe_classification(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_object (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    label      TEXT    NOT NULL,
    confidence REAL    NOT NULL,
    has_bbox   INTEGER NOT NULL DEFAULT 0,
    bbox_x     REAL,
    bbox_y     REAL,
    bbox_w     REAL,
    bbox_h     REAL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_object_keyframe_id ON keyframe_object(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_action (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    label      TEXT    NOT NULL,
    confidence REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_action_keyframe_id ON keyframe_action(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_text_detection (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    text       TEXT    NOT NULL,
    confidence REAL    NOT NULL,
    bbox_x     REAL    NOT NULL,
    bbox_y     REAL    NOT NULL,
    bbox_w     REAL    NOT NULL,
    bbox_h     REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_text_detection_keyframe_id ON keyframe_text_detection(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_barcode (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    payload    TEXT    NOT NULL,
    symbology  TEXT    NOT NULL,
    confidence REAL    NOT NULL,
    bbox_x     REAL    NOT NULL,
    bbox_y     REAL    NOT NULL,
    bbox_w     REAL    NOT NULL,
    bbox_h     REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_barcode_keyframe_id ON keyframe_barcode(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_saliency (
    keyframe_id   BLOB    NOT NULL,
    kind       INTEGER NOT NULL,
    ordinal    INTEGER NOT NULL,
    bbox_x     REAL    NOT NULL,
    bbox_y     REAL    NOT NULL,
    bbox_w     REAL    NOT NULL,
    bbox_h     REAL    NOT NULL,
    confidence REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_saliency_keyframe_id ON keyframe_saliency(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_document_segment (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    tl_x       REAL    NOT NULL,
    tl_y       REAL    NOT NULL,
    tr_x       REAL    NOT NULL,
    tr_y       REAL    NOT NULL,
    br_x       REAL    NOT NULL,
    br_y       REAL    NOT NULL,
    bl_x       REAL    NOT NULL,
    bl_y       REAL    NOT NULL,
    confidence REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_document_segment_keyframe_id ON keyframe_document_segment(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_color (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    rgba       INTEGER NOT NULL,
    name       TEXT    NOT NULL,
    percentage REAL    NOT NULL,
    population INTEGER NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_color_keyframe_id ON keyframe_color(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_subject (
    keyframe_id   BLOB    NOT NULL,
    scope      INTEGER NOT NULL,
    ordinal    INTEGER NOT NULL,
    label      TEXT    NOT NULL,
    confidence REAL    NOT NULL,
    bbox_x     REAL    NOT NULL,
    bbox_y     REAL    NOT NULL,
    bbox_w     REAL    NOT NULL,
    bbox_h     REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, scope, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_subject_keyframe_id ON keyframe_subject(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_face (
    keyframe_id        BLOB    NOT NULL,
    kind            INTEGER NOT NULL,
    ordinal         INTEGER NOT NULL,
    bbox_x          REAL    NOT NULL,
    bbox_y          REAL    NOT NULL,
    bbox_w          REAL    NOT NULL,
    bbox_h          REAL    NOT NULL,
    confidence      REAL    NOT NULL,
    capture_quality REAL    NOT NULL,
    roll            REAL    NOT NULL,
    yaw             REAL    NOT NULL,
    pitch           REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_keyframe_id ON keyframe_face(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_body_pose (
    keyframe_id   BLOB    NOT NULL,
    scope      INTEGER NOT NULL,
    ordinal    INTEGER NOT NULL,
    bbox_x     REAL    NOT NULL,
    bbox_y     REAL    NOT NULL,
    bbox_w     REAL    NOT NULL,
    bbox_h     REAL    NOT NULL,
    confidence REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, scope, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_keyframe_id ON keyframe_body_pose(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_joint (
    keyframe_id       BLOB    NOT NULL,
    scope          INTEGER NOT NULL,
    parent_ordinal INTEGER NOT NULL,
    ordinal        INTEGER NOT NULL,
    name           TEXT    NOT NULL,
    x              REAL    NOT NULL,
    y              REAL    NOT NULL,
    confidence     REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, scope, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_joint_keyframe_id ON keyframe_body_pose_joint(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_hand_pose (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    bbox_x     REAL    NOT NULL,
    bbox_y     REAL    NOT NULL,
    bbox_w     REAL    NOT NULL,
    bbox_h     REAL    NOT NULL,
    confidence REAL    NOT NULL,
    chirality  INTEGER NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_hand_pose_keyframe_id ON keyframe_hand_pose(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d (
    keyframe_id          BLOB    NOT NULL,
    ordinal           INTEGER NOT NULL,
    confidence        REAL    NOT NULL,
    body_height       REAL    NOT NULL,
    height_estimation INTEGER NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_3d_keyframe_id ON keyframe_body_pose_3d(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d_joint (
    keyframe_id       BLOB    NOT NULL,
    parent_ordinal INTEGER NOT NULL,
    ordinal        INTEGER NOT NULL,
    name           TEXT    NOT NULL,
    x              REAL    NOT NULL,
    y              REAL    NOT NULL,
    z              REAL    NOT NULL,
    confidence     REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_3d_joint_keyframe_id ON keyframe_body_pose_3d_joint(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_mask (
    keyframe_id       BLOB    NOT NULL,
    kind           INTEGER NOT NULL,
    ordinal        INTEGER NOT NULL,
    bbox_x         REAL    NOT NULL,
    bbox_y         REAL    NOT NULL,
    bbox_w         REAL    NOT NULL,
    bbox_h         REAL    NOT NULL,
    confidence     REAL    NOT NULL,
    instance_index INTEGER,
    width          INTEGER NOT NULL,
    height         INTEGER NOT NULL,
    data           BLOB    NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_mask_keyframe_id ON keyframe_mask(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_face_landmarks (
    keyframe_id   BLOB    NOT NULL,
    ordinal    INTEGER NOT NULL,
    bbox_x     REAL    NOT NULL,
    bbox_y     REAL    NOT NULL,
    bbox_w     REAL    NOT NULL,
    bbox_h     REAL    NOT NULL,
    confidence REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmarks_keyframe_id ON keyframe_face_landmarks(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_region (
    keyframe_id       BLOB    NOT NULL,
    parent_ordinal INTEGER NOT NULL,
    ordinal        INTEGER NOT NULL,
    name           TEXT    NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmark_region_keyframe_id ON keyframe_face_landmark_region(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_point (
    keyframe_id       BLOB    NOT NULL,
    parent_ordinal INTEGER NOT NULL,
    region_ordinal INTEGER NOT NULL,
    ordinal        INTEGER NOT NULL,
    x              REAL    NOT NULL,
    y              REAL    NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, region_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmark_point_keyframe_id ON keyframe_face_landmark_point(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_vlm_label (
    keyframe_id   BLOB    NOT NULL,
    kind       INTEGER NOT NULL,
    ordinal    INTEGER NOT NULL,
    src        TEXT    NOT NULL,
    translated TEXT    NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_vlm_label_keyframe_id ON keyframe_vlm_label(keyframe_id);

-- Subtitle-cluster: the `Subtitle` facet + `SubtitleTrack` +
-- `SubtitleCue` (+ the `index_errors` child table). Nested value-objects
-- are flattened into real columns; collections ride in child tables with
-- an `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS subtitle (
    id                     BLOB    NOT NULL PRIMARY KEY,
    media_id                 BLOB    NOT NULL,
    track_progress_total   INTEGER NOT NULL DEFAULT 0,
    track_progress_indexed INTEGER NOT NULL DEFAULT 0,
    track_progress_failed  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_subtitle_media_id ON subtitle(media_id);

CREATE TABLE IF NOT EXISTS subtitle_track (
    id                         BLOB    NOT NULL PRIMARY KEY,
    subtitle_id                BLOB    NOT NULL,
    stream_index               INTEGER,
    container_track_id         INTEGER,
    codec                      TEXT    NOT NULL,
    format                     TEXT    NOT NULL,
    origin                     INTEGER NOT NULL DEFAULT 0,
    language                   TEXT,
    title                      TEXT    NOT NULL,
    disposition                INTEGER NOT NULL DEFAULT 0,
    is_primary                 INTEGER NOT NULL DEFAULT 0,
    auto_selected              INTEGER NOT NULL DEFAULT 0,
    duration_pts               INTEGER,
    duration_tb_num            INTEGER,
    duration_tb_den            INTEGER,
    cue_count                  INTEGER NOT NULL DEFAULT 0,
    provenance_model_name      TEXT    NOT NULL,
    provenance_model_version   TEXT    NOT NULL,
    provenance_prompt_version  TEXT    NOT NULL,
    provenance_indexer_version TEXT    NOT NULL,
    source_checksum            BLOB,
    character_encoding         TEXT    NOT NULL,
    bom_present                INTEGER NOT NULL DEFAULT 0,
    is_sdh                     INTEGER NOT NULL DEFAULT 0,
    is_closed_caption          INTEGER NOT NULL DEFAULT 0,
    is_translation             INTEGER NOT NULL DEFAULT 0,
    kind                       INTEGER NOT NULL DEFAULT 0,
    coverage_ratio             REAL,
    is_empty                   INTEGER NOT NULL DEFAULT 0,
    first_cue_pts              INTEGER,
    first_cue_tb_num           INTEGER,
    first_cue_tb_den           INTEGER,
    last_cue_pts               INTEGER,
    last_cue_tb_num            INTEGER,
    last_cue_tb_den            INTEGER,
    index_status               INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_subtitle_id ON subtitle_track(subtitle_id);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_codec       ON subtitle_track(codec);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_language    ON subtitle_track(language);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_origin      ON subtitle_track(origin);

-- AVDictionary entries per subtitle_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS subtitle_track_metadata (
    subtitle_track_id  BLOB    NOT NULL,
    ordinal            INTEGER NOT NULL,
    key                TEXT    NOT NULL,
    value              TEXT    NOT NULL,
    PRIMARY KEY (subtitle_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_metadata_subtitle_track_id ON subtitle_track_metadata(subtitle_track_id);

CREATE TABLE IF NOT EXISTS subtitle_track_index_error (
    subtitle_track_id BLOB    NOT NULL,
    ordinal        INTEGER NOT NULL,
    code           INTEGER NOT NULL,
    message        TEXT    NOT NULL,
    PRIMARY KEY (subtitle_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_stie_subtitle_track_id ON subtitle_track_index_error(subtitle_track_id);

-- ── subtitle_cue: polymorphic base (schema/subtitle_cues.md rev 5) ──────
CREATE TABLE IF NOT EXISTS subtitle_cue (
    id                  BLOB    NOT NULL PRIMARY KEY,
    subtitle_track_id   BLOB    NOT NULL,
    ordinal             INTEGER NOT NULL,
    span_start_pts      INTEGER NOT NULL,
    span_end_pts        INTEGER NOT NULL,
    text_src            TEXT    NOT NULL,
    text_translated     TEXT    NOT NULL,
    kind                INTEGER NOT NULL
);
CREATE INDEX        IF NOT EXISTS idx_subtitle_cue_subtitle_track_id         ON subtitle_cue(subtitle_track_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subtitle_cue_subtitle_track_id_ordinal ON subtitle_cue(subtitle_track_id, ordinal);
CREATE INDEX        IF NOT EXISTS idx_subtitle_cue_kind                      ON subtitle_cue(kind);

CREATE TABLE IF NOT EXISTS subtitle_cue_vtt (
    id              BLOB    NOT NULL PRIMARY KEY,
    cue_identifier  TEXT    NOT NULL,
    vertical        INTEGER,
    line_value      TEXT    NOT NULL,
    line_align      INTEGER,
    position_value  TEXT    NOT NULL,
    position_align  INTEGER,
    size_value      REAL,
    text_align      INTEGER,
    region_id       BLOB,
    voice           TEXT    NOT NULL,
    styled_text     TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS subtitle_cue_ass (
    id            BLOB    NOT NULL PRIMARY KEY,
    layer         INTEGER NOT NULL DEFAULT 0,
    style_id      BLOB    NOT NULL,
    name          TEXT    NOT NULL,
    margin_l      INTEGER NOT NULL DEFAULT 0,
    margin_r      INTEGER NOT NULL DEFAULT 0,
    margin_v      INTEGER NOT NULL DEFAULT 0,
    effect        TEXT    NOT NULL,
    styled_text   TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_ass_style_id ON subtitle_cue_ass(style_id);

CREATE TABLE IF NOT EXISTS subtitle_cue_lrc (
    id                BLOB    NOT NULL PRIMARY KEY,
    has_word_timing   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS subtitle_cue_lrc_word (
    subtitle_cue_id   BLOB    NOT NULL,
    ordinal           INTEGER NOT NULL,
    text              TEXT    NOT NULL,
    start_pts         INTEGER NOT NULL,
    PRIMARY KEY (subtitle_cue_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_lrc_word_cue_id ON subtitle_cue_lrc_word(subtitle_cue_id);

CREATE TABLE IF NOT EXISTS subtitle_track_vtt_region (
    id                  BLOB    NOT NULL PRIMARY KEY,
    subtitle_track_id   BLOB    NOT NULL,
    name                TEXT    NOT NULL,
    width               REAL    NOT NULL,
    lines               INTEGER NOT NULL,
    region_anchor_x     REAL    NOT NULL,
    region_anchor_y     REAL    NOT NULL,
    viewport_anchor_x   REAL    NOT NULL,
    viewport_anchor_y   REAL    NOT NULL,
    scroll_up           INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_vtt_region_track_id ON subtitle_track_vtt_region(subtitle_track_id);

CREATE TABLE IF NOT EXISTS subtitle_track_vtt_style (
    id                  BLOB    NOT NULL PRIMARY KEY,
    subtitle_track_id   BLOB    NOT NULL,
    ordinal             INTEGER NOT NULL,
    css_text            TEXT    NOT NULL
);
CREATE INDEX        IF NOT EXISTS idx_subtitle_track_vtt_style_track_id         ON subtitle_track_vtt_style(subtitle_track_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subtitle_track_vtt_style_track_id_ordinal ON subtitle_track_vtt_style(subtitle_track_id, ordinal);

CREATE TABLE IF NOT EXISTS subtitle_track_ass_style (
    id                  BLOB    NOT NULL PRIMARY KEY,
    subtitle_track_id   BLOB    NOT NULL,
    name                TEXT    NOT NULL,
    fontname            TEXT    NOT NULL,
    fontsize            REAL    NOT NULL,
    primary_colour      INTEGER NOT NULL,
    secondary_colour    INTEGER NOT NULL,
    outline_colour      INTEGER NOT NULL,
    back_colour         INTEGER NOT NULL,
    bold                INTEGER NOT NULL,
    italic              INTEGER NOT NULL,
    underline           INTEGER NOT NULL,
    strikeout           INTEGER NOT NULL,
    scale_x             INTEGER NOT NULL,
    scale_y             INTEGER NOT NULL,
    spacing             INTEGER NOT NULL,
    angle               REAL    NOT NULL,
    border_style        INTEGER NOT NULL,
    outline             REAL    NOT NULL,
    shadow              REAL    NOT NULL,
    alignment           INTEGER NOT NULL,
    margin_l            INTEGER NOT NULL,
    margin_r            INTEGER NOT NULL,
    margin_v            INTEGER NOT NULL,
    encoding            INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_ass_style_track_id ON subtitle_track_ass_style(subtitle_track_id);

CREATE TABLE IF NOT EXISTS subtitle_track_lrc_metadata (
    subtitle_track_id   BLOB    NOT NULL PRIMARY KEY,
    title               TEXT    NOT NULL,
    artist              TEXT    NOT NULL,
    album               TEXT    NOT NULL,
    author              TEXT    NOT NULL,
    creator             TEXT    NOT NULL,
    length              TEXT    NOT NULL,
    offset_ms           INTEGER NOT NULL DEFAULT 0
);

-- ── Long-tail text-format detail tables (issue #56) ──────────────────────

CREATE TABLE IF NOT EXISTS subtitle_cue_micro_dvd (
    id            BLOB    NOT NULL PRIMARY KEY,
    styled_text   TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS subtitle_cue_sub_viewer (
    id            BLOB    NOT NULL PRIMARY KEY,
    styled_text   TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS subtitle_cue_sbv (
    id            BLOB    NOT NULL PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS subtitle_cue_ttml (
    id            BLOB    NOT NULL PRIMARY KEY,
    region_id     BLOB,
    style_id      BLOB,
    xml_id        TEXT    NOT NULL,
    styled_text   TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_ttml_region_id ON subtitle_cue_ttml(region_id);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_ttml_style_id  ON subtitle_cue_ttml(style_id);

CREATE TABLE IF NOT EXISTS subtitle_cue_sami (
    id            BLOB    NOT NULL PRIMARY KEY,
    class_name    TEXT    NOT NULL,
    styled_text   TEXT    NOT NULL
);

-- ── Long-tail bitmap / broadcast detail tables (issue #56) ────────────────

CREATE TABLE IF NOT EXISTS subtitle_cue_vob_sub (
    id                BLOB     NOT NULL PRIMARY KEY,
    palette_id        BLOB     NOT NULL,
    bitmap            BLOB     NOT NULL,
    width             INTEGER  NOT NULL,
    height            INTEGER  NOT NULL,
    pos_x             INTEGER  NOT NULL,
    pos_y             INTEGER  NOT NULL,
    color_indices     INTEGER  NOT NULL,
    contrast_indices  INTEGER  NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_vob_sub_palette_id ON subtitle_cue_vob_sub(palette_id);

CREATE TABLE IF NOT EXISTS subtitle_cue_pgs (
    id                 BLOB     NOT NULL PRIMARY KEY,
    bitmap             BLOB     NOT NULL,
    width              INTEGER  NOT NULL,
    height             INTEGER  NOT NULL,
    pos_x              INTEGER  NOT NULL,
    pos_y              INTEGER  NOT NULL,
    palette_bytes      BLOB     NOT NULL,
    composition_state  INTEGER  NOT NULL
);

CREATE TABLE IF NOT EXISTS subtitle_cue_cea_608 (
    id              BLOB    NOT NULL PRIMARY KEY,
    channel         INTEGER NOT NULL,
    pac_byte_pair   INTEGER NOT NULL,
    styled_text     TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS subtitle_cue_ebu_stl (
    id               BLOB    NOT NULL PRIMARY KEY,
    subtitle_number  INTEGER NOT NULL,
    cumulative       INTEGER NOT NULL DEFAULT 0,
    vertical_pos     INTEGER NOT NULL,
    justification    INTEGER NOT NULL,
    styled_text      TEXT    NOT NULL
);

-- ── Per-track long-tail aggregates (issue #56) ───────────────────────────

CREATE TABLE IF NOT EXISTS subtitle_track_ttml_region (
    id                  BLOB    NOT NULL PRIMARY KEY,
    subtitle_track_id   BLOB    NOT NULL,
    xml_id              TEXT    NOT NULL,
    xml_attrs           TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_ttml_region_track_id ON subtitle_track_ttml_region(subtitle_track_id);

CREATE TABLE IF NOT EXISTS subtitle_track_ttml_style (
    id                  BLOB    NOT NULL PRIMARY KEY,
    subtitle_track_id   BLOB    NOT NULL,
    xml_id              TEXT    NOT NULL,
    xml_attrs           TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_ttml_style_track_id ON subtitle_track_ttml_style(subtitle_track_id);

CREATE TABLE IF NOT EXISTS subtitle_track_sami_style (
    id                  BLOB    NOT NULL PRIMARY KEY,
    subtitle_track_id   BLOB    NOT NULL,
    class_name          TEXT    NOT NULL,
    css_text            TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_sami_style_track_id ON subtitle_track_sami_style(subtitle_track_id);

-- DVD VobSub palette: 16 fixed-position RGB entries; SQLite has no
-- native array, so each `0x00RRGGBB` u32 lives on its own INTEGER
-- column (`entry00 … entry15`).
CREATE TABLE IF NOT EXISTS subtitle_track_vob_sub_palette (
    id                  BLOB     NOT NULL PRIMARY KEY,
    subtitle_track_id   BLOB     NOT NULL,
    entry00             INTEGER  NOT NULL,
    entry01             INTEGER  NOT NULL,
    entry02             INTEGER  NOT NULL,
    entry03             INTEGER  NOT NULL,
    entry04             INTEGER  NOT NULL,
    entry05             INTEGER  NOT NULL,
    entry06             INTEGER  NOT NULL,
    entry07             INTEGER  NOT NULL,
    entry08             INTEGER  NOT NULL,
    entry09             INTEGER  NOT NULL,
    entry10             INTEGER  NOT NULL,
    entry11             INTEGER  NOT NULL,
    entry12             INTEGER  NOT NULL,
    entry13             INTEGER  NOT NULL,
    entry14             INTEGER  NOT NULL,
    entry15             INTEGER  NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_vob_sub_palette_track_id ON subtitle_track_vob_sub_palette(subtitle_track_id);
