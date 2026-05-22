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

CREATE TABLE IF NOT EXISTS speaker (
    id                  BLOB    NOT NULL PRIMARY KEY,
    parent              BLOB    NOT NULL,    -- FK -> audio_track.id
    cluster_id          INTEGER NOT NULL,
    name                TEXT    NOT NULL,
    speech_duration_ms  INTEGER
);
CREATE INDEX IF NOT EXISTS idx_speaker_parent ON speaker(parent);

CREATE TABLE IF NOT EXISTS user_tag (
    id            BLOB    NOT NULL PRIMARY KEY,
    name          TEXT    NOT NULL,
    color_rgba    INTEGER,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS scene_annotation (
    id              BLOB    NOT NULL PRIMARY KEY,
    scene           BLOB    NOT NULL,        -- FK -> scene.id
    favorite        INTEGER NOT NULL DEFAULT 0,
    rating          INTEGER,
    note            TEXT    NOT NULL DEFAULT '',
    updated_at_ms   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scene_annotation_scene ON scene_annotation(scene);

CREATE TABLE IF NOT EXISTS scene_annotation_user_tag (
    scene_annotation  BLOB    NOT NULL,
    user_tag          BLOB    NOT NULL,
    ordinal           INTEGER NOT NULL,
    PRIMARY KEY (scene_annotation, user_tag)
);
CREATE INDEX IF NOT EXISTS idx_saut_user_tag ON scene_annotation_user_tag (user_tag);

-- The facet + track + per-track-analysis tables (audio, video, subtitle,
-- audio_track, video_track, subtitle_track, audio_segment, scene,
-- keyframe, subtitle_cue) are tracked as a follow-up. Their schema
-- shape is documented in the corresponding `schema/*.md` locked specs;
-- the row mapping is deferred: the published mediaframe descriptor VOs
-- (`VideoCodec`, `ChannelLayout`, `color::Info`, `PixelFormat`, …) carry
-- no serde derives, so each needs a hand-rolled flat-column mapping (as
-- the capture `Device` / `GeoLocation` columns do for the Media row) —
-- tracked as a focused follow-up rather than landing alongside this revision.
