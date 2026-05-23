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
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_tag_name ON user_tag(name);

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

-- Audio-cluster: the `Audio` facet + `AudioTrack` + `AudioSegment`
-- (+ the `Word` / `index_errors` child tables). Nested value-objects are
-- flattened into real columns; collections ride in child tables with an
-- `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS audio (
    id                     BLOB    NOT NULL PRIMARY KEY,
    parent                 BLOB    NOT NULL UNIQUE,
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
    index_status              INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_audio_track_audio_id ON audio_track(audio_id);

CREATE TABLE IF NOT EXISTS audio_track_index_error (
    audio_track BLOB    NOT NULL,   -- FK -> audio_track.id
    ordinal     INTEGER NOT NULL,
    code        INTEGER NOT NULL,
    message     TEXT    NOT NULL,
    PRIMARY KEY (audio_track, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_atie_audio_track ON audio_track_index_error(audio_track);

CREATE TABLE IF NOT EXISTS audio_segment (
    id              BLOB    NOT NULL PRIMARY KEY,
    parent          BLOB    NOT NULL,   -- FK -> audio_track.id
    "index"         INTEGER NOT NULL,
    span_start_pts  INTEGER NOT NULL,
    span_end_pts    INTEGER NOT NULL,
    span_tb_num     INTEGER NOT NULL,
    span_tb_den     INTEGER NOT NULL,
    speaker         BLOB,               -- FK -> speaker.id; NULL = not diarized
    text_src        TEXT    NOT NULL,
    text_translated TEXT    NOT NULL,
    language        TEXT,
    no_speech_prob  REAL,
    avg_logprob     REAL,
    temperature     REAL
);
CREATE INDEX IF NOT EXISTS idx_audio_segment_parent ON audio_segment(parent);

CREATE TABLE IF NOT EXISTS audio_segment_word (
    audio_segment  BLOB    NOT NULL,   -- FK -> audio_segment.id
    ordinal        INTEGER NOT NULL,
    text           TEXT    NOT NULL,
    span_start_pts INTEGER NOT NULL,
    span_end_pts   INTEGER NOT NULL,
    span_tb_num    INTEGER NOT NULL,
    span_tb_den    INTEGER NOT NULL,
    score          REAL    NOT NULL,
    language       TEXT,
    PRIMARY KEY (audio_segment, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_asw_audio_segment ON audio_segment_word(audio_segment);

-- The video + subtitle facet/track/per-track-analysis tables (video,
-- subtitle, video_track, subtitle_track, scene, keyframe, subtitle_cue)
-- are tracked as a follow-up. Their schema shape is documented in the
-- corresponding `schema/*.md` locked specs; the row mapping is deferred:
-- the remaining mediaframe descriptor VOs (`VideoCodec`, `color::Info`,
-- `PixelFormat`, …) carry no serde derives, so each needs a hand-rolled
-- flat-column mapping (as the capture `Device` / `GeoLocation` columns
-- do for the Media row) — tracked as a focused follow-up.
