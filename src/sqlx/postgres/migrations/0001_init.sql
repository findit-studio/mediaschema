-- mediaschema — PostgreSQL DDL (canonical).
--
-- Identity columns are native `uuid`.
-- Checksum columns are `BYTEA` (32 bytes).
-- Nested VOs ride as `JSONB`. Row structs decode them as `String`;
-- queries SELECTing those columns should append `::text`.
-- Wall-clock timestamps are BIGINT ms-since-epoch.

CREATE TABLE IF NOT EXISTS media (
    id              uuid    NOT NULL PRIMARY KEY,
    checksum        bytea   NOT NULL,
    name            text    NOT NULL,
    format          text    NOT NULL,
    size            bigint  NOT NULL,
    duration_raw    bigint,
    created_at_ms   bigint  NOT NULL,
    kind            smallint NOT NULL,
    video           uuid,
    audio           uuid,
    subtitle        uuid,
    error_flags     integer NOT NULL DEFAULT 0,
    probe_error_json jsonb,
    capture_date_ms bigint,
    device_json     jsonb,
    gps_json        jsonb
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_media_checksum ON media(checksum);
CREATE INDEX        IF NOT EXISTS idx_media_video    ON media(video);
CREATE INDEX        IF NOT EXISTS idx_media_audio    ON media(audio);
CREATE INDEX        IF NOT EXISTS idx_media_subtitle ON media(subtitle);

CREATE TABLE IF NOT EXISTS watched_location (
    id                    uuid    NOT NULL PRIMARY KEY,
    volume                uuid    NOT NULL UNIQUE,
    recursive             boolean NOT NULL DEFAULT false,
    enabled               boolean NOT NULL DEFAULT false,
    is_ejectable          boolean NOT NULL DEFAULT false,
    added_at_ms           bigint  NOT NULL,
    last_reconciled_at_ms bigint,
    last_reconcile_status smallint,
    last_error_json       jsonb
);

CREATE TABLE IF NOT EXISTS speaker (
    id                  uuid    NOT NULL PRIMARY KEY,
    parent              uuid    NOT NULL,
    cluster_id          integer NOT NULL,
    name                text    NOT NULL,
    speech_duration_ms  bigint
);
CREATE INDEX IF NOT EXISTS idx_speaker_parent ON speaker(parent);

CREATE TABLE IF NOT EXISTS user_tag (
    id            uuid   NOT NULL PRIMARY KEY,
    name          text   NOT NULL,
    color_rgba    bigint,
    created_at_ms bigint NOT NULL
);

CREATE TABLE IF NOT EXISTS scene_annotation (
    id              uuid    NOT NULL PRIMARY KEY,
    scene           uuid    NOT NULL,
    favorite        boolean NOT NULL DEFAULT false,
    user_tags_json  jsonb   NOT NULL,
    rating          smallint,
    note            text    NOT NULL,
    updated_at_ms   bigint  NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scene_annotation_scene ON scene_annotation(scene);

-- The facet + track + per-track-analysis tables (audio, video, subtitle,
-- audio_track, video_track, subtitle_track, audio_segment, scene,
-- keyframe, subtitle_cue) are tracked as a follow-up — see the SQLite
-- schema for the same scope note.
