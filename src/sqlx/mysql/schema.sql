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
    -- would falsely collide two long paths sharing a prefix); same shape
    -- across pg/mysql/sqlite. Inline `UNIQUE KEY` / `KEY` because MySQL
    -- has no `CREATE INDEX IF NOT EXISTS` (mirrors `idx_media_checksum`).
    UNIQUE KEY idx_media_file_path        (location_volume, location_path_hash),
    -- Prefix-lookup index on the plain path for `LIKE 'prefix/%'` scans.
    KEY        idx_media_file_path_prefix (location_volume, location_path)
);

CREATE TABLE IF NOT EXISTS speaker (
    id                  BINARY(16) NOT NULL,
    parent              BINARY(16) NOT NULL,
    cluster_id          INT UNSIGNED NOT NULL,
    name                VARCHAR(256) NOT NULL,
    speech_duration_ms  BIGINT,
    PRIMARY KEY (id),
    KEY idx_speaker_parent (parent)
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
    scene           BINARY(16) NOT NULL,
    favorite        TINYINT    NOT NULL DEFAULT 0,
    rating          TINYINT UNSIGNED,
    note            TEXT       NOT NULL,
    updated_at_ms   BIGINT     NOT NULL,
    PRIMARY KEY (id),
    KEY idx_scene_annotation_scene (scene)
);

CREATE TABLE IF NOT EXISTS scene_annotation_user_tag (
    scene_annotation  BINARY(16) NOT NULL,
    user_tag          BINARY(16) NOT NULL,
    ordinal           INT        NOT NULL,
    PRIMARY KEY (scene_annotation, user_tag),
    KEY idx_saut_user_tag (user_tag)
);

-- Audio-cluster: the `Audio` facet + `AudioTrack` + `AudioSegment`
-- (+ the `Word` / `index_errors` child tables). Nested value-objects are
-- flattened into real columns; collections ride in child tables with an
-- `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS audio (
    id                     BINARY(16) NOT NULL,
    parent                 BINARY(16) NOT NULL,
    total_segments         BIGINT     NOT NULL DEFAULT 0,
    track_progress_total   BIGINT     NOT NULL DEFAULT 0,
    track_progress_indexed BIGINT     NOT NULL DEFAULT 0,
    track_progress_failed  BIGINT     NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    UNIQUE KEY uq_audio_parent (parent)
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
    index_status              BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    KEY idx_audio_track_audio_id (audio_id)
);

CREATE TABLE IF NOT EXISTS audio_track_index_error (
    audio_track BINARY(16) NOT NULL,
    ordinal     INT        NOT NULL,
    code        INT        NOT NULL,
    message     TEXT       NOT NULL,
    PRIMARY KEY (audio_track, ordinal),
    KEY idx_atie_audio_track (audio_track)
);

CREATE TABLE IF NOT EXISTS audio_segment (
    id              BINARY(16)   NOT NULL,
    parent          BINARY(16)   NOT NULL,
    `index`         BIGINT       NOT NULL,
    span_start_pts  BIGINT       NOT NULL,
    span_end_pts    BIGINT       NOT NULL,
    span_tb_num     BIGINT       NOT NULL,
    span_tb_den     BIGINT       NOT NULL,
    speaker         BINARY(16),
    text_src        TEXT         NOT NULL,
    text_translated TEXT         NOT NULL,
    language        VARCHAR(64),
    no_speech_prob  FLOAT,
    avg_logprob     FLOAT,
    temperature     FLOAT,
    PRIMARY KEY (id),
    KEY idx_audio_segment_parent (parent)
);

CREATE TABLE IF NOT EXISTS audio_segment_word (
    audio_segment  BINARY(16) NOT NULL,
    ordinal        INT        NOT NULL,
    text           TEXT       NOT NULL,
    span_start_pts BIGINT     NOT NULL,
    span_end_pts   BIGINT     NOT NULL,
    span_tb_num    BIGINT     NOT NULL,
    span_tb_den    BIGINT     NOT NULL,
    score          FLOAT      NOT NULL,
    language       VARCHAR(64),
    PRIMARY KEY (audio_segment, ordinal),
    KEY idx_asw_audio_segment (audio_segment)
);

-- Video-cluster: the `Video` facet + `VideoTrack` + `Scene` + `Keyframe`
-- (+ per-detection child tables).

CREATE TABLE IF NOT EXISTS video (
    id                     BINARY(16) NOT NULL,
    parent                 BINARY(16) NOT NULL,
    total_scenes           BIGINT     NOT NULL DEFAULT 0,
    track_progress_total   BIGINT     NOT NULL DEFAULT 0,
    track_progress_indexed BIGINT     NOT NULL DEFAULT 0,
    track_progress_failed  BIGINT     NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    UNIQUE KEY uq_video_parent (parent)
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

CREATE TABLE IF NOT EXISTS video_track_index_error (
    video_track BINARY(16) NOT NULL,
    ordinal     INT        NOT NULL,
    code        INT        NOT NULL,
    message     TEXT       NOT NULL,
    PRIMARY KEY (video_track, ordinal),
    KEY idx_vtie_video_track (video_track)
);

CREATE TABLE IF NOT EXISTS scene (
    id              BINARY(16)  NOT NULL,
    parent          BINARY(16)  NOT NULL,
    `index`         BIGINT      NOT NULL,
    span_start_pts  BIGINT      NOT NULL,
    span_end_pts    BIGINT      NOT NULL,
    span_tb_num     BIGINT      NOT NULL,
    span_tb_den     BIGINT      NOT NULL,
    detector        VARCHAR(64) NOT NULL,
    description     TEXT        NOT NULL,
    PRIMARY KEY (id),
    KEY idx_scene_parent (parent),
    KEY idx_scene_detector (detector),
    UNIQUE KEY idx_scene_parent_index (parent, `index`)
);

CREATE TABLE IF NOT EXISTS keyframe (
    id                         BINARY(16)   NOT NULL,
    parent                     BINARY(16)   NOT NULL,
    pts                        BIGINT       NOT NULL,
    pts_tb_num                 BIGINT       NOT NULL,
    pts_tb_den                 BIGINT       NOT NULL,
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
    KEY idx_keyframe_parent (parent)
);

CREATE TABLE IF NOT EXISTS keyframe_classification (
    keyframe   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    label      TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_classification_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_object (
    keyframe   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    label      TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    has_bbox   TINYINT    NOT NULL DEFAULT 0,
    bbox_x     FLOAT,
    bbox_y     FLOAT,
    bbox_w     FLOAT,
    bbox_h     FLOAT,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_object_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_action (
    keyframe   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    label      TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_action_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_text_detection (
    keyframe   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    text       TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_text_detection_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_barcode (
    keyframe   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    payload    TEXT       NOT NULL,
    symbology  VARCHAR(64) NOT NULL,
    confidence FLOAT      NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_barcode_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_saliency (
    keyframe   BINARY(16) NOT NULL,
    kind       SMALLINT   NOT NULL,
    ordinal    INT        NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, kind, ordinal),
    KEY idx_kf_saliency_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_document_segment (
    keyframe   BINARY(16) NOT NULL,
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
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_document_segment_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_color (
    keyframe   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    rgba       BIGINT     NOT NULL,
    name       VARCHAR(64) NOT NULL,
    percentage FLOAT      NOT NULL,
    population BIGINT     NOT NULL,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_color_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_subject (
    keyframe   BINARY(16) NOT NULL,
    scope      SMALLINT   NOT NULL,
    ordinal    INT        NOT NULL,
    label      TEXT       NOT NULL,
    confidence FLOAT      NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, scope, ordinal),
    KEY idx_kf_subject_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_face (
    keyframe        BINARY(16) NOT NULL,
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
    PRIMARY KEY (keyframe, kind, ordinal),
    KEY idx_kf_face_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_body_pose (
    keyframe   BINARY(16) NOT NULL,
    scope      SMALLINT   NOT NULL,
    ordinal    INT        NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, scope, ordinal),
    KEY idx_kf_body_pose_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_joint (
    keyframe       BINARY(16) NOT NULL,
    scope          SMALLINT   NOT NULL,
    parent_ordinal INT        NOT NULL,
    ordinal        INT        NOT NULL,
    name           VARCHAR(128) NOT NULL,
    x              FLOAT      NOT NULL,
    y              FLOAT      NOT NULL,
    confidence     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, scope, parent_ordinal, ordinal),
    KEY idx_kf_body_pose_joint_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_hand_pose (
    keyframe   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    chirality  SMALLINT   NOT NULL,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_hand_pose_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d (
    keyframe          BINARY(16) NOT NULL,
    ordinal           INT        NOT NULL,
    confidence        FLOAT      NOT NULL,
    body_height       FLOAT      NOT NULL,
    height_estimation SMALLINT   NOT NULL,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_body_pose_3d_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d_joint (
    keyframe       BINARY(16) NOT NULL,
    parent_ordinal INT        NOT NULL,
    ordinal        INT        NOT NULL,
    name           VARCHAR(128) NOT NULL,
    x              FLOAT      NOT NULL,
    y              FLOAT      NOT NULL,
    z              FLOAT      NOT NULL,
    confidence     FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, parent_ordinal, ordinal),
    KEY idx_kf_body_pose_3d_joint_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_mask (
    keyframe       BINARY(16) NOT NULL,
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
    PRIMARY KEY (keyframe, kind, ordinal),
    KEY idx_kf_mask_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_face_landmarks (
    keyframe   BINARY(16) NOT NULL,
    ordinal    INT        NOT NULL,
    bbox_x     FLOAT      NOT NULL,
    bbox_y     FLOAT      NOT NULL,
    bbox_w     FLOAT      NOT NULL,
    bbox_h     FLOAT      NOT NULL,
    confidence FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, ordinal),
    KEY idx_kf_face_landmarks_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_region (
    keyframe       BINARY(16)   NOT NULL,
    parent_ordinal INT          NOT NULL,
    ordinal        INT          NOT NULL,
    name           VARCHAR(128) NOT NULL,
    PRIMARY KEY (keyframe, parent_ordinal, ordinal),
    KEY idx_kf_face_landmark_region_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_point (
    keyframe       BINARY(16) NOT NULL,
    parent_ordinal INT        NOT NULL,
    region_ordinal INT        NOT NULL,
    ordinal        INT        NOT NULL,
    x              FLOAT      NOT NULL,
    y              FLOAT      NOT NULL,
    PRIMARY KEY (keyframe, parent_ordinal, region_ordinal, ordinal),
    KEY idx_kf_face_landmark_point_keyframe (keyframe)
);

CREATE TABLE IF NOT EXISTS keyframe_vlm_label (
    keyframe   BINARY(16) NOT NULL,
    kind       SMALLINT   NOT NULL,
    ordinal    INT        NOT NULL,
    src        TEXT       NOT NULL,
    translated TEXT       NOT NULL,
    PRIMARY KEY (keyframe, kind, ordinal),
    KEY idx_kf_vlm_label_keyframe (keyframe)
);

-- The subtitle facet/track/per-track-analysis tables (subtitle,
-- subtitle_track, subtitle_cue) are tracked as a follow-up — same scope
-- note as in the SQLite schema.
