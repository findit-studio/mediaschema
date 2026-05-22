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
    total_segments         BIGINT     NOT NULL DEFAULT 0,
    track_progress_total   BIGINT     NOT NULL DEFAULT 0,
    track_progress_indexed BIGINT     NOT NULL DEFAULT 0,
    track_progress_failed  BIGINT     NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
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

-- The video + subtitle facet/track/per-track-analysis tables (video,
-- subtitle, video_track, subtitle_track, scene, keyframe, subtitle_cue)
-- are tracked as a follow-up — see the SQLite schema for the same scope
-- note.
