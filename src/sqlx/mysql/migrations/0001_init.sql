-- mediaschema — MySQL DDL (canonical).
--
-- Identity columns are BINARY(16) (Uuid7).
-- Checksum columns are BINARY(32).
-- Nested value-objects are flattened into real, individually-indexable
-- columns; many-to-many collections ride in dedicated join tables.
-- Wall-clock timestamps are BIGINT ms-since-epoch.

CREATE TABLE IF NOT EXISTS media (
    id                  BINARY(16)  NOT NULL,
    checksum            BINARY(32)  NOT NULL,
    format              VARCHAR(64)   NOT NULL,
    size                BIGINT UNSIGNED NOT NULL,
    duration_raw        BIGINT,
    kind                SMALLINT    NOT NULL,
    -- Verbatim AVFormatContext.nb_streams / nb_chapters (rev 11).
    nb_streams          INT UNSIGNED NOT NULL DEFAULT 0,
    nb_chapters         INT UNSIGNED NOT NULL DEFAULT 0,
    video               BINARY(16),
    audio               BINARY(16),
    subtitle            BINARY(16),
    error_flags         SMALLINT UNSIGNED NOT NULL DEFAULT 0,
    probe_error_code    INT,
    probe_error_message TEXT,
    capture_date_ms     BIGINT,
    device_make         TEXT,
    device_model        TEXT,
    gps_lat             DOUBLE,
    gps_lon             DOUBLE,
    gps_altitude        FLOAT,
    PRIMARY KEY (id),
    UNIQUE KEY idx_media_checksum (checksum),
    KEY idx_media_video (video),
    KEY idx_media_audio (audio),
    KEY idx_media_subtitle (subtitle)
);

-- Container-level chapters (AVFormatContext.chapters[i]). See
-- schema/chapter.md (rev 1). `title` hoisted from AVDictionary's "title"
-- key (any case; first match wins); remaining metadata in
-- `chapter_metadata`, keyed by ordinal.
CREATE TABLE IF NOT EXISTS chapter (
    id                  BINARY(16)      NOT NULL,
    media_id            BINARY(16)      NOT NULL,
    chapter_index       INT UNSIGNED    NOT NULL,
    source_id           BIGINT          NOT NULL,
    start_pts           BIGINT          NOT NULL,
    end_pts             BIGINT          NOT NULL,
    timebase_num        BIGINT          NOT NULL,
    timebase_den        BIGINT          NOT NULL,
    -- MySQL has no functional index on LOWER(title); use a generated
    -- column for the case-insensitive lookup index.
    title               VARCHAR(4096)   NOT NULL DEFAULT '',
    title_lower         VARCHAR(4096) GENERATED ALWAYS AS (LOWER(title)) STORED,
    PRIMARY KEY (id),
    UNIQUE KEY idx_chapter_media_id_index (media_id, chapter_index),
    KEY idx_chapter_media_id     (media_id),
    KEY idx_chapter_title_lower  (title_lower(255))
);

-- AVDictionary entries per chapter, **excluding** the "title" key.
-- `ordinal` preserves IndexMap insertion order.
CREATE TABLE IF NOT EXISTS chapter_metadata (
    chapter_id  BINARY(16)      NOT NULL,
    ordinal     INT UNSIGNED    NOT NULL,
    `key`       VARCHAR(255)    NOT NULL,
    value       TEXT            NOT NULL,
    PRIMARY KEY (chapter_id, ordinal),
    KEY idx_chapter_metadata_chapter_id (chapter_id)
);

CREATE TABLE IF NOT EXISTS watched_location (
    id                    BINARY(16) NOT NULL,
    volume                BINARY(16) NOT NULL UNIQUE,
    recursive             TINYINT    NOT NULL DEFAULT 0,
    enabled               TINYINT    NOT NULL DEFAULT 0,
    is_ejectable          TINYINT    NOT NULL DEFAULT 0,
    added_at_ms           BIGINT     NOT NULL,
    last_reconciled_at_ms BIGINT,
    last_reconcile_status SMALLINT,
    last_error_code       INT,
    last_error_message    TEXT,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS media_file (
    id                  BINARY(16) NOT NULL,
    media_id            BINARY(16) NOT NULL,
    created_at_ms       BIGINT,
    location_volume     BINARY(16) NOT NULL,
    location_path       TEXT       NOT NULL,
    location_path_hash  BINARY(32) NOT NULL,
    watched_location_id BINARY(16) NOT NULL,
    watch_volume        BINARY(16) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_media_file_media_id (media_id),
    KEY idx_media_file_watched_location_id (watched_location_id),
    -- Natural-key uniqueness of a copy: one path per volume. Hashing the
    -- path with SHA-256 sidesteps InnoDB's prefix-length requirement for
    -- `UNIQUE` indexes over variable-length `TEXT` (a truncated prefix
    -- would falsely collide two long paths sharing a prefix). Pg and
    -- sqlite UNIQUE-index TEXT natively and don't need this column.
    -- Inline `UNIQUE KEY` / `KEY` because MySQL has no
    -- `CREATE INDEX IF NOT EXISTS` (mirrors `idx_media_checksum`).
    UNIQUE KEY idx_media_file_path        (location_volume, location_path_hash),
    -- Prefix-lookup index on the plain path for `LIKE 'prefix/%'` scans.
    KEY        idx_media_file_path_prefix (location_volume, location_path)
);

CREATE TABLE IF NOT EXISTS speaker (
    id                                    BINARY(16) NOT NULL,
    audio_track_id                        BINARY(16) NOT NULL,
    cluster_id                            INT UNSIGNED NOT NULL,
    name                                  VARCHAR(256) NOT NULL,
    speech_duration_ms                    BIGINT,
    -- Per-track aggregated voiceprint. `voiceprint_vector_id IS NOT NULL`
    -- is the discriminator: when present, the other voiceprint_* columns
    -- carry the full flattened VO; when NULL, they are all NULL.
    voiceprint_vector_id                  BINARY(16),
    voiceprint_dimensions                 INT UNSIGNED,
    voiceprint_extracted_at_ms            BIGINT,
    voiceprint_confidence                 FLOAT,
    voiceprint_provenance_model_name      VARCHAR(256),
    voiceprint_provenance_model_version   VARCHAR(256),
    voiceprint_provenance_prompt_version  VARCHAR(256),
    voiceprint_provenance_indexer_version VARCHAR(256),
    -- Cross-track identity FK -> person.id; NULL = not yet identified.
    person_id                             BINARY(16),
    PRIMARY KEY (id),
    KEY idx_speaker_audio_track_id (audio_track_id),
    KEY idx_speaker_person_id (person_id)
);
);

CREATE TABLE IF NOT EXISTS user_tag (
    id            BINARY(16) NOT NULL,
    name          VARCHAR(256) NOT NULL,
    color_rgba    INT UNSIGNED,
    created_at_ms BIGINT NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY idx_user_tag_name (name)
);

CREATE TABLE IF NOT EXISTS scene_annotation (
    id              BINARY(16) NOT NULL,
    scene_id           BINARY(16) NOT NULL,
    favorite        TINYINT    NOT NULL DEFAULT 0,
    rating          TINYINT UNSIGNED,
    note            TEXT       NOT NULL,
    updated_at_ms   BIGINT     NOT NULL,
    PRIMARY KEY (id),
    KEY idx_scene_annotation_scene_id (scene_id)
);

CREATE TABLE IF NOT EXISTS scene_annotation_user_tag (
    scene_annotation_id  BINARY(16) NOT NULL,
    user_tag_id          BINARY(16) NOT NULL,
    ordinal           INT        NOT NULL,
    PRIMARY KEY (scene_annotation_id, user_tag_id),
    KEY idx_saut_user_tag_id (user_tag_id)
);

-- Audio-cluster: the `Audio` facet + `AudioTrack` + `AudioSegment`
-- (+ the `Word` / `index_errors` child tables). Nested value-objects are
-- flattened into real columns; collections ride in child tables with an
-- `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS audio (
    id                     BINARY(16) NOT NULL,
    media_id                 BINARY(16) NOT NULL,
    total_segments         BIGINT     NOT NULL DEFAULT 0,
    track_progress_total   BIGINT     NOT NULL DEFAULT 0,
    track_progress_indexed BIGINT     NOT NULL DEFAULT 0,
    track_progress_failed  BIGINT     NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    UNIQUE KEY uq_audio_media_id (media_id)
);

CREATE TABLE IF NOT EXISTS audio_track (
    id                        BINARY(16)   NOT NULL,
    audio_id                  BINARY(16)   NOT NULL,
    stream_index              BIGINT,
    container_track_id        BIGINT,
    codec                     VARCHAR(64)  NOT NULL,
    profile                   VARCHAR(64)  NOT NULL,
    sample_rate               BIGINT       NOT NULL DEFAULT 0,
    channels                  INT          NOT NULL DEFAULT 0,
    channel_layout            VARCHAR(64)  NOT NULL,
    -- `SampleFormat::to_u32` (FFmpeg `AV_SAMPLE_FMT_*` code). Default
    -- `4294967295` (`u32::MAX`) so a freshly-inserted row decodes back as
    -- `SampleFormat::Unknown(u32::MAX)` == `SampleFormat::default()`.
    sample_format             BIGINT       NOT NULL DEFAULT 4294967295,
    bit_rate                  BIGINT       NOT NULL DEFAULT 0,
    bit_rate_mode             INT,
    bits_per_sample           INT,
    is_lossless               TINYINT      NOT NULL DEFAULT 0,
    duration_pts              BIGINT,
    duration_tb_num           BIGINT,
    duration_tb_den           BIGINT,
    start_pts                 BIGINT,
    start_pts_tb_num          BIGINT,
    start_pts_tb_den          BIGINT,
    language                  VARCHAR(64),
    detected_language         VARCHAR(64),
    disposition               BIGINT       NOT NULL DEFAULT 0,
    is_primary                TINYINT      NOT NULL DEFAULT 0,
    auto_selected             TINYINT      NOT NULL DEFAULT 0,
    content                   SMALLINT,
    speech_ratio              FLOAT,
    is_silent                 TINYINT      NOT NULL DEFAULT 0,
    has_loudness              TINYINT      NOT NULL DEFAULT 0,
    loudness_integrated_lufs  FLOAT,
    loudness_range_lu         FLOAT,
    loudness_true_peak_dbtp   FLOAT,
    loudness_sample_peak_dbfs FLOAT,
    has_replay_gain           TINYINT      NOT NULL DEFAULT 0,
    replay_gain_track_gain_db FLOAT,
    replay_gain_track_peak    FLOAT,
    replay_gain_album_gain_db FLOAT,
    replay_gain_album_peak    FLOAT,
    fingerprint_algo          VARCHAR(64),
    fingerprint_value         BLOB,
    isrc                      VARCHAR(64)  NOT NULL,
    acoustid                  VARCHAR(64)  NOT NULL,
    musicbrainz_recording_id  VARCHAR(64)  NOT NULL,
    has_tags                  TINYINT      NOT NULL DEFAULT 0,
    tags_title                TEXT,
    tags_artist               TEXT,
    tags_album_artist         TEXT,
    tags_album                TEXT,
    tags_composer             TEXT,
    tags_genre                TEXT,
    tags_comment              TEXT,
    tags_year                 INT,
    tags_track_number         INT,
    tags_track_total          INT,
    tags_disc_number          INT,
    tags_disc_total           INT,
    tags_language             VARCHAR(64),
    cover_art_mime            VARCHAR(255),
    cover_art_data            LONGBLOB,
    provenance_model_name     VARCHAR(255) NOT NULL,
    provenance_model_version  VARCHAR(255) NOT NULL,
    provenance_prompt_version VARCHAR(255) NOT NULL,
    provenance_indexer_version VARCHAR(255) NOT NULL,
    vad_provenance_model_name     VARCHAR(255) NOT NULL,
    vad_provenance_model_version  VARCHAR(255) NOT NULL,
    vad_provenance_prompt_version VARCHAR(255) NOT NULL,
    vad_provenance_indexer_version VARCHAR(255) NOT NULL,
    index_status              BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    KEY idx_audio_track_audio_id (audio_id)
);

-- AVDictionary entries per audio_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS audio_track_metadata (
    audio_track_id  BINARY(16)   NOT NULL,
    ordinal         INT          NOT NULL,
    `key`           VARCHAR(255) NOT NULL,
    value           TEXT         NOT NULL,
    PRIMARY KEY (audio_track_id, ordinal),
    KEY idx_audio_track_metadata_audio_track_id (audio_track_id)
);

CREATE TABLE IF NOT EXISTS audio_track_index_error (
    audio_track_id BINARY(16) NOT NULL,
    ordinal     INT        NOT NULL,
    code        INT        NOT NULL,
    message     TEXT       NOT NULL,
    PRIMARY KEY (audio_track_id, ordinal),
    KEY idx_atie_audio_track_id (audio_track_id)
);

CREATE TABLE IF NOT EXISTS audio_segment (
    id              BINARY(16)   NOT NULL,
    audio_track_id          BINARY(16)   NOT NULL,
    `index`         BIGINT       NOT NULL,
    span_start_pts  BIGINT       NOT NULL,
    span_end_pts    BIGINT       NOT NULL,
    speaker         BINARY(16),
    text_src        TEXT         NOT NULL,
    text_translated TEXT         NOT NULL,
    language        VARCHAR(64),
    no_speech_prob  FLOAT,
    avg_logprob     FLOAT,
    temperature     FLOAT,
    -- Per-segment voice embedding. `voice_fingerprint_vector_id IS NOT NULL`
    -- discriminates presence of the flattened VoiceFingerprint VO.
    voice_fingerprint_vector_id                  BINARY(16),
    voice_fingerprint_dimensions                 INT UNSIGNED,
    voice_fingerprint_extracted_at_ms            BIGINT,
    voice_fingerprint_confidence                 FLOAT,
    voice_fingerprint_provenance_model_name      VARCHAR(255),
    voice_fingerprint_provenance_model_version   VARCHAR(255),
    voice_fingerprint_provenance_prompt_version  VARCHAR(255),
    voice_fingerprint_provenance_indexer_version VARCHAR(255),
    PRIMARY KEY (id),
    KEY idx_audio_segment_audio_track_id (audio_track_id)
);

CREATE TABLE IF NOT EXISTS audio_segment_word (
    audio_segment_id  BINARY(16) NOT NULL,
    ordinal        INT        NOT NULL,
    text           TEXT       NOT NULL,
    span_start_pts BIGINT     NOT NULL,
    span_end_pts   BIGINT     NOT NULL,
    score          FLOAT      NOT NULL,
    language       VARCHAR(64),
    PRIMARY KEY (audio_segment_id, ordinal),
    KEY idx_asw_audio_segment_id (audio_segment_id)
);

-- Video-cluster: the `Video` facet + `VideoTrack` + `Scene` + `Keyframe`
-- (+ per-detection child tables).

CREATE TABLE IF NOT EXISTS video (
    id                     BINARY(16) NOT NULL,
    media_id                 BINARY(16) NOT NULL,
    total_scenes           BIGINT     NOT NULL DEFAULT 0,
    track_progress_total   BIGINT     NOT NULL DEFAULT 0,
    track_progress_indexed BIGINT     NOT NULL DEFAULT 0,
    track_progress_failed  BIGINT     NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    UNIQUE KEY uq_video_media_id (media_id)
);

CREATE TABLE IF NOT EXISTS video_track (
    id                       BINARY(16)   NOT NULL,
    video_id                 BINARY(16)   NOT NULL,
    stream_index             BIGINT,
    container_track_id       BIGINT,
    start_pts                BIGINT,
    start_pts_tb_num         BIGINT,
    start_pts_tb_den         BIGINT,
    duration_pts             BIGINT,
    duration_tb_num          BIGINT,
    duration_tb_den          BIGINT,
    codec                    VARCHAR(64)  NOT NULL,
    profile                  VARCHAR(64),
    level                    INT,
    bit_rate                 BIGINT       NOT NULL DEFAULT 0,
    nb_frames                BIGINT,
    has_b_frames             TINYINT      NOT NULL DEFAULT 0,
    closed_gop               TINYINT,
    bits_per_raw_sample      SMALLINT,
    width                    BIGINT       NOT NULL DEFAULT 0,
    height                   BIGINT       NOT NULL DEFAULT 0,
    has_visible_rect         TINYINT      NOT NULL DEFAULT 0,
    visible_rect_x           BIGINT,
    visible_rect_y           BIGINT,
    visible_rect_w           BIGINT,
    visible_rect_h           BIGINT,
    sar_num                  BIGINT       NOT NULL DEFAULT 1,
    sar_den                  BIGINT       NOT NULL DEFAULT 1,
    pixel_format             BIGINT       NOT NULL DEFAULT 0,
    color_primaries          BIGINT       NOT NULL DEFAULT 0,
    color_transfer           BIGINT       NOT NULL DEFAULT 0,
    color_matrix             BIGINT       NOT NULL DEFAULT 0,
    color_range              BIGINT       NOT NULL DEFAULT 0,
    color_chroma_location    BIGINT       NOT NULL DEFAULT 0,
    has_hdr_static           TINYINT      NOT NULL DEFAULT 0,
    hdr_has_mastering        TINYINT      NOT NULL DEFAULT 0,
    hdr_primary_r_x          BIGINT,
    hdr_primary_r_y          BIGINT,
    hdr_primary_g_x          BIGINT,
    hdr_primary_g_y          BIGINT,
    hdr_primary_b_x          BIGINT,
    hdr_primary_b_y          BIGINT,
    hdr_white_point_x        BIGINT,
    hdr_white_point_y        BIGINT,
    hdr_max_luminance        BIGINT,
    hdr_min_luminance        BIGINT,
    hdr_has_content_light    TINYINT      NOT NULL DEFAULT 0,
    hdr_max_cll              BIGINT,
    hdr_max_fall             BIGINT,
    rotation                 BIGINT       NOT NULL DEFAULT 0,
    fr_num                   BIGINT       NOT NULL DEFAULT 1,
    fr_den                   BIGINT       NOT NULL DEFAULT 1,
    fr_is_vfr                TINYINT      NOT NULL DEFAULT 0,
    -- `AVStream.avg_frame_rate` — defaults `0/1` (= `FrameRate::default()`,
    -- absent). For CFR content this equals (fr_num, fr_den); for VFR the
    -- two diverge.
    avg_fr_num               BIGINT       NOT NULL DEFAULT 0,
    avg_fr_den               BIGINT       NOT NULL DEFAULT 1,
    field_order              BIGINT       NOT NULL DEFAULT 0,
    stereo_mode              BIGINT,
    has_dovi                 TINYINT      NOT NULL DEFAULT 0,
    dovi_profile             SMALLINT,
    dovi_level               SMALLINT,
    dovi_rpu_present         TINYINT,
    dovi_el_present          TINYINT,
    dovi_bl_signal_compat_id SMALLINT,
    has_embedded_captions    TINYINT      NOT NULL DEFAULT 0,
    disposition              BIGINT       NOT NULL DEFAULT 0,
    is_primary               TINYINT      NOT NULL DEFAULT 0,
    auto_selected            TINYINT      NOT NULL DEFAULT 0,
    provenance_model_name    VARCHAR(255) NOT NULL,
    provenance_model_version VARCHAR(255) NOT NULL,
    provenance_prompt_version VARCHAR(255) NOT NULL,
    provenance_indexer_version VARCHAR(255) NOT NULL,
    index_status             BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    KEY idx_video_track_video_id (video_id)
);

-- AVDictionary entries per video_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS video_track_metadata (
    video_track_id  BINARY(16)   NOT NULL,
    ordinal         INT          NOT NULL,
    `key`           VARCHAR(255) NOT NULL,
    value           TEXT         NOT NULL,
    PRIMARY KEY (video_track_id, ordinal),
    KEY idx_video_track_metadata_video_track_id (video_track_id)
);

CREATE TABLE IF NOT EXISTS video_track_index_error (
    video_track_id BINARY(16) NOT NULL,
    ordinal     INT        NOT NULL,
    code        INT        NOT NULL,
    message     TEXT       NOT NULL,
    PRIMARY KEY (video_track_id, ordinal),
    KEY idx_vtie_video_track_id (video_track_id)
);

CREATE TABLE IF NOT EXISTS scene (
    id              BINARY(16)  NOT NULL,
    video_track_id          BINARY(16)  NOT NULL,
    `index`         BIGINT      NOT NULL,
    span_start_pts  BIGINT      NOT NULL,
    span_end_pts    BIGINT      NOT NULL,
    detector        VARCHAR(64) NOT NULL,
    description     TEXT        NOT NULL,
    PRIMARY KEY (id),
    KEY idx_scene_video_track_id (video_track_id),
    KEY idx_scene_detector (detector),
    UNIQUE KEY idx_scene_video_track_id_index (video_track_id, `index`)
);

CREATE TABLE IF NOT EXISTS keyframe (
    id                         BINARY(16)   NOT NULL,
    scene_id                     BINARY(16)   NOT NULL,
    pts                        BIGINT       NOT NULL,
    data                       LONGBLOB     NOT NULL,
    mime                       VARCHAR(255) NOT NULL,
    width                      BIGINT       NOT NULL,
    height                     BIGINT       NOT NULL,
    extractor                  VARCHAR(64)  NOT NULL,
    vlm_description_src        TEXT         NOT NULL,
    vlm_description_translated TEXT         NOT NULL,
    vlm_shot_type              VARCHAR(64)  NOT NULL,
    horizon_angle              FLOAT        NOT NULL,
    horizon_confidence         FLOAT        NOT NULL,
    aesthetics_overall_score   FLOAT        NOT NULL,
    aesthetics_is_utility      TINYINT      NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    KEY idx_keyframe_scene_id (scene_id)
);

CREATE TABLE IF NOT EXISTS keyframe_classification (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    label      TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_classification_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_object (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    label      TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    has_bbox   TINYINT    NOT NULL DEFAULT 0,
    bbox_x     FLOAT,
    bbox_y     FLOAT,
    bbox_w     FLOAT,
    bbox_h     FLOAT,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_object_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_action (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    label      TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_action_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_text_detection (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    text       TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_text_detection_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_barcode (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    payload    TEXT       NOT NULL,
    symbology  VARCHAR(64) NOT NULL,
    confidence FLOAT      NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_barcode_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_saliency (
    keyframe_id   BINARY(16) NOT NULL,
    kind       SMALLINT   NOT NULL,
    ordinal    INT        NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal),
    KEY idx_kf_saliency_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_document_segment (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    tl_x       FLOAT      NOT NULL,
    tl_y       FLOAT      NOT NULL,
    tr_x       FLOAT      NOT NULL,
    tr_y       FLOAT      NOT NULL,
    br_x       FLOAT      NOT NULL,
    br_y       FLOAT      NOT NULL,
    bl_x       FLOAT      NOT NULL,
    bl_y       FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_document_segment_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_color (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    rgba       BIGINT     NOT NULL,
    name       VARCHAR(64) NOT NULL,
    percentage FLOAT      NOT NULL,
    population BIGINT     NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_color_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_subject (
    keyframe_id   BINARY(16) NOT NULL,
    scope      SMALLINT   NOT NULL,
    ordinal    INT        NOT NULL,
    label      TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, scope, ordinal),
    KEY idx_kf_subject_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_face (
    keyframe_id        BINARY(16) NOT NULL,
    kind            SMALLINT   NOT NULL,
    ordinal         INT        NOT NULL,
    bbox_x          FLOAT      NOT NULL,
    bbox_y          FLOAT      NOT NULL,
    bbox_w          FLOAT      NOT NULL,
    bbox_h          FLOAT      NOT NULL,
    confidence      FLOAT      NOT NULL,
    capture_quality FLOAT      NOT NULL,
    roll            FLOAT      NOT NULL,
    yaw             FLOAT      NOT NULL,
    pitch           FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal),
    KEY idx_kf_face_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_body_pose (
    keyframe_id   BINARY(16) NOT NULL,
    scope      SMALLINT   NOT NULL,
    ordinal    INT        NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, scope, ordinal),
    KEY idx_kf_body_pose_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_joint (
    keyframe_id       BINARY(16) NOT NULL,
    scope          SMALLINT   NOT NULL,
    parent_ordinal INT        NOT NULL,
    ordinal        INT        NOT NULL,
    name           VARCHAR(128) NOT NULL,
    x              FLOAT      NOT NULL,
    y              FLOAT      NOT NULL,
    confidence     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, scope, parent_ordinal, ordinal),
    KEY idx_kf_body_pose_joint_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_hand_pose (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    chirality  SMALLINT   NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_hand_pose_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d (
    keyframe_id          BINARY(16) NOT NULL,
    ordinal           INT        NOT NULL,
    confidence        FLOAT      NOT NULL,
    body_height       FLOAT      NOT NULL,
    height_estimation SMALLINT   NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_body_pose_3d_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d_joint (
    keyframe_id       BINARY(16) NOT NULL,
    parent_ordinal INT        NOT NULL,
    ordinal        INT        NOT NULL,
    name           VARCHAR(128) NOT NULL,
    x              FLOAT      NOT NULL,
    y              FLOAT      NOT NULL,
    z              FLOAT      NOT NULL,
    confidence     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, ordinal),
    KEY idx_kf_body_pose_3d_joint_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_mask (
    keyframe_id       BINARY(16) NOT NULL,
    kind           SMALLINT   NOT NULL,
    ordinal        INT        NOT NULL,
    bbox_x         FLOAT      NOT NULL,
    bbox_y         FLOAT      NOT NULL,
    bbox_w         FLOAT      NOT NULL,
    bbox_h         FLOAT      NOT NULL,
    confidence     FLOAT      NOT NULL,
    instance_index BIGINT,
    width          BIGINT     NOT NULL,
    height         BIGINT     NOT NULL,
    data           LONGBLOB   NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal),
    KEY idx_kf_mask_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_face_landmarks (
    keyframe_id   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal),
    KEY idx_kf_face_landmarks_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_region (
    keyframe_id       BINARY(16)   NOT NULL,
    parent_ordinal INT          NOT NULL,
    ordinal        INT          NOT NULL,
    name           VARCHAR(128) NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, ordinal),
    KEY idx_kf_face_landmark_region_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_point (
    keyframe_id       BINARY(16) NOT NULL,
    parent_ordinal INT        NOT NULL,
    region_ordinal INT        NOT NULL,
    ordinal        INT        NOT NULL,
    x              FLOAT      NOT NULL,
    y              FLOAT      NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, region_ordinal, ordinal),
    KEY idx_kf_face_landmark_point_keyframe_id (keyframe_id)
);

CREATE TABLE IF NOT EXISTS keyframe_vlm_label (
    keyframe_id   BINARY(16) NOT NULL,
    kind       SMALLINT   NOT NULL,
    ordinal    INT        NOT NULL,
    src        TEXT       NOT NULL,
    translated TEXT       NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal),
    KEY idx_kf_vlm_label_keyframe_id (keyframe_id)
);

-- Subtitle-cluster: the `Subtitle` facet + `SubtitleTrack` +
-- `SubtitleCue` (+ the `index_errors` child table). Nested value-objects
-- are flattened into real columns; collections ride in child tables with
-- an `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS subtitle (
    id                     BINARY(16) NOT NULL,
    media_id                 BINARY(16) NOT NULL,
    track_progress_total   BIGINT     NOT NULL DEFAULT 0,
    track_progress_indexed BIGINT     NOT NULL DEFAULT 0,
    track_progress_failed  BIGINT     NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    KEY idx_subtitle_media_id (media_id)
);

CREATE TABLE IF NOT EXISTS subtitle_track (
    id                         BINARY(16)   NOT NULL,
    subtitle_id                BINARY(16)   NOT NULL,
    stream_index               BIGINT,
    container_track_id         BIGINT,
    codec                      VARCHAR(64)  NOT NULL,
    format                     VARCHAR(64)  NOT NULL,
    origin                     INT          NOT NULL DEFAULT 0,
    language                   VARCHAR(64),
    title                      TEXT         NOT NULL,
    disposition                BIGINT       NOT NULL DEFAULT 0,
    is_primary                 TINYINT      NOT NULL DEFAULT 0,
    auto_selected              TINYINT      NOT NULL DEFAULT 0,
    duration_pts               BIGINT,
    duration_tb_num            BIGINT,
    duration_tb_den            BIGINT,
    cue_count                  BIGINT       NOT NULL DEFAULT 0,
    provenance_model_name      VARCHAR(255) NOT NULL,
    provenance_model_version   VARCHAR(255) NOT NULL,
    provenance_prompt_version  VARCHAR(255) NOT NULL,
    provenance_indexer_version VARCHAR(255) NOT NULL,
    source_checksum            BINARY(32),
    character_encoding         VARCHAR(64)  NOT NULL,
    bom_present                TINYINT      NOT NULL DEFAULT 0,
    is_sdh                     TINYINT      NOT NULL DEFAULT 0,
    is_closed_caption          TINYINT      NOT NULL DEFAULT 0,
    is_translation             TINYINT      NOT NULL DEFAULT 0,
    kind                       SMALLINT     NOT NULL DEFAULT 0,
    coverage_ratio             FLOAT,
    is_empty                   TINYINT      NOT NULL DEFAULT 0,
    first_cue_pts              BIGINT,
    first_cue_tb_num           BIGINT,
    first_cue_tb_den           BIGINT,
    last_cue_pts               BIGINT,
    last_cue_tb_num            BIGINT,
    last_cue_tb_den            BIGINT,
    index_status               BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    KEY idx_subtitle_track_subtitle_id (subtitle_id),
    KEY idx_subtitle_track_codec (codec),
    KEY idx_subtitle_track_language (language),
    KEY idx_subtitle_track_origin (origin)
);

-- AVDictionary entries per subtitle_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS subtitle_track_metadata (
    subtitle_track_id  BINARY(16)   NOT NULL,
    ordinal            INT          NOT NULL,
    `key`              VARCHAR(255) NOT NULL,
    value              TEXT         NOT NULL,
    PRIMARY KEY (subtitle_track_id, ordinal),
    KEY idx_subtitle_track_metadata_subtitle_track_id (subtitle_track_id)
);

CREATE TABLE IF NOT EXISTS subtitle_track_index_error (
    subtitle_track_id BINARY(16) NOT NULL,
    ordinal        INT        NOT NULL,
    code           INT        NOT NULL,
    message        TEXT       NOT NULL,
    PRIMARY KEY (subtitle_track_id, ordinal),
    KEY idx_stie_subtitle_track_id (subtitle_track)
);

-- ── subtitle_cue: polymorphic base (schema/subtitle_cues.md rev 5) ──────
CREATE TABLE IF NOT EXISTS subtitle_cue (
    id                  BINARY(16) NOT NULL,
    subtitle_track_id   BINARY(16) NOT NULL,
    ordinal             BIGINT     NOT NULL,
    span_start_pts      BIGINT     NOT NULL,
    span_end_pts        BIGINT     NOT NULL,
    text_src            TEXT       NOT NULL,
    text_translated     TEXT       NOT NULL,
    kind                SMALLINT   NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_cue_subtitle_track_id (subtitle_track_id),
    UNIQUE KEY idx_subtitle_cue_subtitle_track_id_ordinal (subtitle_track_id, ordinal),
    KEY idx_subtitle_cue_kind (kind)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_vtt (
    id              BINARY(16) NOT NULL,
    cue_identifier  TEXT       NOT NULL,
    vertical        SMALLINT,
    line_value      TEXT       NOT NULL,
    line_align      SMALLINT,
    position_value  TEXT       NOT NULL,
    position_align  SMALLINT,
    size_value      FLOAT,
    text_align      SMALLINT,
    region_id       BINARY(16),
    voice           TEXT       NOT NULL,
    styled_text     TEXT       NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_ass (
    id            BINARY(16) NOT NULL,
    layer         INT        NOT NULL DEFAULT 0,
    style_id      BINARY(16) NOT NULL,
    name          TEXT       NOT NULL,
    margin_l      INT        NOT NULL DEFAULT 0,
    margin_r      INT        NOT NULL DEFAULT 0,
    margin_v      INT        NOT NULL DEFAULT 0,
    effect        TEXT       NOT NULL,
    styled_text   TEXT       NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_cue_ass_style_id (style_id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_lrc (
    id                BINARY(16) NOT NULL,
    has_word_timing   BOOLEAN    NOT NULL DEFAULT FALSE,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_lrc_word (
    subtitle_cue_id   BINARY(16) NOT NULL,
    ordinal           INT        NOT NULL,
    text              TEXT       NOT NULL,
    start_pts         BIGINT     NOT NULL,
    PRIMARY KEY (subtitle_cue_id, ordinal),
    KEY idx_subtitle_cue_lrc_word_cue_id (subtitle_cue_id)
);

CREATE TABLE IF NOT EXISTS subtitle_track_vtt_region (
    id                  BINARY(16) NOT NULL,
    subtitle_track_id   BINARY(16) NOT NULL,
    name                TEXT       NOT NULL,
    width               FLOAT      NOT NULL,
    lines               BIGINT     NOT NULL,
    region_anchor_x     FLOAT      NOT NULL,
    region_anchor_y     FLOAT      NOT NULL,
    viewport_anchor_x   FLOAT      NOT NULL,
    viewport_anchor_y   FLOAT      NOT NULL,
    scroll_up           BOOLEAN    NOT NULL DEFAULT FALSE,
    PRIMARY KEY (id),
    KEY idx_subtitle_track_vtt_region_track_id (subtitle_track_id)
);

CREATE TABLE IF NOT EXISTS subtitle_track_vtt_style (
    id                  BINARY(16) NOT NULL,
    subtitle_track_id   BINARY(16) NOT NULL,
    ordinal             INT        NOT NULL,
    css_text            TEXT       NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_track_vtt_style_track_id (subtitle_track_id),
    UNIQUE KEY idx_subtitle_track_vtt_style_track_id_ordinal (subtitle_track_id, ordinal)
);

CREATE TABLE IF NOT EXISTS subtitle_track_ass_style (
    id                  BINARY(16) NOT NULL,
    subtitle_track_id   BINARY(16) NOT NULL,
    name                TEXT       NOT NULL,
    fontname            TEXT       NOT NULL,
    fontsize            FLOAT      NOT NULL,
    primary_colour      BIGINT     NOT NULL,
    secondary_colour    BIGINT     NOT NULL,
    outline_colour      BIGINT     NOT NULL,
    back_colour         BIGINT     NOT NULL,
    bold                BOOLEAN    NOT NULL,
    italic              BOOLEAN    NOT NULL,
    underline           BOOLEAN    NOT NULL,
    strikeout           BOOLEAN    NOT NULL,
    scale_x             INT        NOT NULL,
    scale_y             INT        NOT NULL,
    spacing             INT        NOT NULL,
    angle               FLOAT      NOT NULL,
    border_style        SMALLINT   NOT NULL,
    outline             FLOAT      NOT NULL,
    shadow              FLOAT      NOT NULL,
    alignment           SMALLINT   NOT NULL,
    margin_l            INT        NOT NULL,
    margin_r            INT        NOT NULL,
    margin_v            INT        NOT NULL,
    encoding            INT        NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_track_ass_style_track_id (subtitle_track_id)
);

CREATE TABLE IF NOT EXISTS subtitle_track_lrc_metadata (
    subtitle_track_id   BINARY(16) NOT NULL,
    title               TEXT       NOT NULL,
    artist              TEXT       NOT NULL,
    album               TEXT       NOT NULL,
    author              TEXT       NOT NULL,
    creator             TEXT       NOT NULL,
    length              TEXT       NOT NULL,
    offset_ms           INT        NOT NULL DEFAULT 0,
    PRIMARY KEY (subtitle_track_id)
);

-- ── Long-tail text-format detail tables (issue #56) ──────────────────────

CREATE TABLE IF NOT EXISTS subtitle_cue_micro_dvd (
    id            BINARY(16) NOT NULL,
    styled_text   TEXT       NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_sub_viewer (
    id            BINARY(16) NOT NULL,
    styled_text   TEXT       NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_sbv (
    id            BINARY(16) NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_ttml (
    id            BINARY(16) NOT NULL,
    region_id     BINARY(16),
    style_id      BINARY(16),
    xml_id        TEXT       NOT NULL,
    styled_text   TEXT       NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_cue_ttml_region_id (region_id),
    KEY idx_subtitle_cue_ttml_style_id  (style_id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_sami (
    id            BINARY(16) NOT NULL,
    class_name    TEXT       NOT NULL,
    styled_text   TEXT       NOT NULL,
    PRIMARY KEY (id)
);

-- ── Long-tail bitmap / broadcast detail tables (issue #56) ────────────────

CREATE TABLE IF NOT EXISTS subtitle_cue_vob_sub (
    id                BINARY(16) NOT NULL,
    palette_id        BINARY(16) NOT NULL,
    bitmap            MEDIUMBLOB NOT NULL,
    width             BIGINT     NOT NULL,
    height            BIGINT     NOT NULL,
    pos_x             INT        NOT NULL,
    pos_y             INT        NOT NULL,
    color_indices     BIGINT     NOT NULL,
    contrast_indices  BIGINT     NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_cue_vob_sub_palette_id (palette_id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_pgs (
    id                 BINARY(16) NOT NULL,
    bitmap             MEDIUMBLOB NOT NULL,
    width              BIGINT     NOT NULL,
    height             BIGINT     NOT NULL,
    pos_x              INT        NOT NULL,
    pos_y              INT        NOT NULL,
    palette_bytes      BLOB       NOT NULL,
    composition_state  SMALLINT   NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_cea_608 (
    id              BINARY(16) NOT NULL,
    channel         SMALLINT   NOT NULL,
    pac_byte_pair   BIGINT     NOT NULL,
    styled_text     TEXT       NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS subtitle_cue_ebu_stl (
    id               BINARY(16) NOT NULL,
    subtitle_number  BIGINT     NOT NULL,
    cumulative       BOOLEAN    NOT NULL DEFAULT FALSE,
    vertical_pos     INT        NOT NULL,
    justification    SMALLINT   NOT NULL,
    styled_text      TEXT       NOT NULL,
    PRIMARY KEY (id)
);

-- ── Per-track long-tail aggregates (issue #56) ───────────────────────────

CREATE TABLE IF NOT EXISTS subtitle_track_ttml_region (
    id                  BINARY(16) NOT NULL,
    subtitle_track_id   BINARY(16) NOT NULL,
    xml_id              TEXT       NOT NULL,
    xml_attrs           TEXT       NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_track_ttml_region_track_id (subtitle_track_id)
);

CREATE TABLE IF NOT EXISTS subtitle_track_ttml_style (
    id                  BINARY(16) NOT NULL,
    subtitle_track_id   BINARY(16) NOT NULL,
    xml_id              TEXT       NOT NULL,
    xml_attrs           TEXT       NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_track_ttml_style_track_id (subtitle_track_id)
);

CREATE TABLE IF NOT EXISTS subtitle_track_sami_style (
    id                  BINARY(16) NOT NULL,
    subtitle_track_id   BINARY(16) NOT NULL,
    class_name          TEXT       NOT NULL,
    css_text            TEXT       NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_track_sami_style_track_id (subtitle_track_id)
);

-- DVD VobSub palette: 16 fixed-position RGB entries. MySQL has no
-- native array, so each `0x00RRGGBB` u32 lives on its own `BIGINT`
-- column (`entry00 … entry15`).
CREATE TABLE IF NOT EXISTS subtitle_track_vob_sub_palette (
    id                  BINARY(16) NOT NULL,
    subtitle_track_id   BINARY(16) NOT NULL,
    entry00             BIGINT     NOT NULL,
    entry01             BIGINT     NOT NULL,
    entry02             BIGINT     NOT NULL,
    entry03             BIGINT     NOT NULL,
    entry04             BIGINT     NOT NULL,
    entry05             BIGINT     NOT NULL,
    entry06             BIGINT     NOT NULL,
    entry07             BIGINT     NOT NULL,
    entry08             BIGINT     NOT NULL,
    entry09             BIGINT     NOT NULL,
    entry10             BIGINT     NOT NULL,
    entry11             BIGINT     NOT NULL,
    entry12             BIGINT     NOT NULL,
    entry13             BIGINT     NOT NULL,
    entry14             BIGINT     NOT NULL,
    entry15             BIGINT     NOT NULL,
    PRIMARY KEY (id),
    KEY idx_subtitle_track_vob_sub_palette_track_id (subtitle_track_id)
);
