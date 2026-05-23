-- mediaschema — PostgreSQL DDL (canonical).
--
-- Identity columns are native `uuid`.
-- Checksum columns are `BYTEA` (32 bytes).
-- Nested value-objects are flattened into real, individually-indexable
-- columns; many-to-many collections ride in dedicated join tables.
-- Wall-clock timestamps are BIGINT ms-since-epoch.

CREATE TABLE IF NOT EXISTS media (
    id                  uuid    NOT NULL PRIMARY KEY,
    checksum            bytea   NOT NULL,
    format              text    NOT NULL,
    size                bigint  NOT NULL,
    duration_raw        bigint,
    kind                smallint NOT NULL,
    video               uuid,
    audio               uuid,
    subtitle            uuid,
    error_flags         integer NOT NULL DEFAULT 0,
    probe_error_code    integer,
    probe_error_message text,
    capture_date_ms     bigint,
    device_make         text,
    device_model        text,
    gps_lat             double precision,
    gps_lon             double precision,
    gps_altitude        real
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
    last_error_code       integer,
    last_error_message    text
);

CREATE TABLE IF NOT EXISTS media_file (
    id                  uuid    NOT NULL PRIMARY KEY,
    media_id            uuid    NOT NULL,
    created_at_ms       bigint,
    location_volume     uuid    NOT NULL,
    location_path       text    NOT NULL,
    watched_location_id uuid    NOT NULL,
    watch_volume        uuid    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_file_media_id            ON media_file(media_id);
CREATE INDEX IF NOT EXISTS idx_media_file_watched_location_id ON media_file(watched_location_id);
-- Natural-key uniqueness of a copy: one path per volume. Postgres
-- UNIQUE-indexes TEXT natively, so the index uses the path column
-- directly; the mysql dialect carries an extra `location_path_hash`
-- column to dodge InnoDB's prefix-length requirement on variable-length
-- TEXT and indexes `(location_volume, location_path_hash)` there.
CREATE UNIQUE INDEX IF NOT EXISTS idx_media_file_path
    ON media_file(location_volume, location_path);

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
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_tag_name ON user_tag(name);

CREATE TABLE IF NOT EXISTS scene_annotation (
    id              uuid    NOT NULL PRIMARY KEY,
    scene           uuid    NOT NULL,
    favorite        boolean NOT NULL DEFAULT false,
    rating          smallint,
    note            text    NOT NULL,
    updated_at_ms   bigint  NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scene_annotation_scene ON scene_annotation(scene);

CREATE TABLE IF NOT EXISTS scene_annotation_user_tag (
    scene_annotation  uuid    NOT NULL,
    user_tag          uuid    NOT NULL,
    ordinal           integer NOT NULL,
    PRIMARY KEY (scene_annotation, user_tag)
);
CREATE INDEX IF NOT EXISTS idx_saut_user_tag ON scene_annotation_user_tag (user_tag);

-- Audio-cluster: the `Audio` facet + `AudioTrack` + `AudioSegment`
-- (+ the `Word` / `index_errors` child tables). Nested value-objects are
-- flattened into real columns; collections ride in child tables with an
-- `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS audio (
    id                     uuid    NOT NULL PRIMARY KEY,
    parent                 uuid    NOT NULL UNIQUE,
    total_segments         bigint  NOT NULL DEFAULT 0,
    track_progress_total   bigint  NOT NULL DEFAULT 0,
    track_progress_indexed bigint  NOT NULL DEFAULT 0,
    track_progress_failed  bigint  NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS audio_track (
    id                        uuid    NOT NULL PRIMARY KEY,
    audio_id                  uuid    NOT NULL,
    stream_index              bigint,
    container_track_id        bigint,
    codec                     text    NOT NULL,
    profile                   text    NOT NULL,
    sample_rate               bigint  NOT NULL DEFAULT 0,
    channels                  integer NOT NULL DEFAULT 0,
    channel_layout            text    NOT NULL,
    bit_rate                  bigint  NOT NULL DEFAULT 0,
    bit_rate_mode             integer,
    bits_per_sample           integer,
    is_lossless               boolean NOT NULL DEFAULT false,
    duration_pts              bigint,
    duration_tb_num           bigint,
    duration_tb_den           bigint,
    start_pts                 bigint,
    start_pts_tb_num          bigint,
    start_pts_tb_den          bigint,
    language                  text,
    detected_language         text,
    disposition               bigint  NOT NULL DEFAULT 0,
    is_primary                boolean NOT NULL DEFAULT false,
    auto_selected             boolean NOT NULL DEFAULT false,
    content                   smallint,
    speech_ratio              real,
    is_silent                 boolean NOT NULL DEFAULT false,
    has_loudness              boolean NOT NULL DEFAULT false,
    loudness_integrated_lufs  real,
    loudness_range_lu         real,
    loudness_true_peak_dbtp   real,
    loudness_sample_peak_dbfs real,
    fingerprint_algo          text,
    fingerprint_value         bytea,
    isrc                      text    NOT NULL,
    acoustid                  text    NOT NULL,
    musicbrainz_recording_id  text    NOT NULL,
    has_tags                  boolean NOT NULL DEFAULT false,
    tags_title                text,
    tags_artist               text,
    tags_album_artist         text,
    tags_album                text,
    tags_composer             text,
    tags_genre                text,
    tags_comment              text,
    tags_year                 integer,
    tags_track_number         integer,
    tags_track_total          integer,
    tags_disc_number          integer,
    tags_disc_total           integer,
    tags_language             text,
    cover_art_mime            text,
    cover_art_data            bytea,
    provenance_model_name     text    NOT NULL,
    provenance_model_version  text    NOT NULL,
    provenance_prompt_version text    NOT NULL,
    provenance_indexer_version text   NOT NULL,
    index_status              bigint  NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_audio_track_audio_id ON audio_track(audio_id);

CREATE TABLE IF NOT EXISTS audio_track_index_error (
    audio_track uuid    NOT NULL,
    ordinal     integer NOT NULL,
    code        integer NOT NULL,
    message     text    NOT NULL,
    PRIMARY KEY (audio_track, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_atie_audio_track ON audio_track_index_error(audio_track);

CREATE TABLE IF NOT EXISTS audio_segment (
    id              uuid    NOT NULL PRIMARY KEY,
    parent          uuid    NOT NULL,
    index           bigint  NOT NULL,
    span_start_pts  bigint  NOT NULL,
    span_end_pts    bigint  NOT NULL,
    speaker         uuid,
    text_src        text    NOT NULL,
    text_translated text    NOT NULL,
    language        text,
    no_speech_prob  real,
    avg_logprob     real,
    temperature     real
);
CREATE INDEX IF NOT EXISTS idx_audio_segment_parent ON audio_segment(parent);

CREATE TABLE IF NOT EXISTS audio_segment_word (
    audio_segment  uuid    NOT NULL,
    ordinal        integer NOT NULL,
    text           text    NOT NULL,
    span_start_pts bigint  NOT NULL,
    span_end_pts   bigint  NOT NULL,
    score          real    NOT NULL,
    language       text,
    PRIMARY KEY (audio_segment, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_asw_audio_segment ON audio_segment_word(audio_segment);

-- The video + subtitle facet/track/per-track-analysis tables (video,
-- subtitle, video_track, subtitle_track, scene, keyframe, subtitle_cue)
-- are tracked as a follow-up — see the SQLite schema for the same scope
-- note.
