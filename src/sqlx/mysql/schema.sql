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

-- The facet + track + per-track-analysis tables (audio, video, subtitle,
-- audio_track, video_track, subtitle_track, audio_segment, scene,
-- keyframe, subtitle_cue) are tracked as a follow-up — see the SQLite
-- schema for the same scope note.
