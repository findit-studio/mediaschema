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
    -- Verbatim AVFormatContext.nb_streams / nb_chapters (rev 11).
    nb_streams          integer NOT NULL DEFAULT 0,
    nb_chapters         integer NOT NULL DEFAULT 0,
    video               uuid,
    audio               uuid,
    subtitle            uuid,
    data                uuid,
    attachment          uuid,
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

-- Container-level chapters (AVFormatContext.chapters[i]). See
-- schema/chapter.md (rev 1). `title` is hoisted from the
-- AVDictionary's "title" key (any case; first match wins);
-- remaining metadata lives in `chapter_metadata` keyed by ordinal.
CREATE TABLE IF NOT EXISTS chapter (
    id                  uuid    NOT NULL PRIMARY KEY,
    media_id            uuid    NOT NULL,
    chapter_index       integer NOT NULL,
    source_id           bigint  NOT NULL,
    start_pts           bigint  NOT NULL,
    end_pts             bigint  NOT NULL,
    timebase_num        bigint  NOT NULL,
    timebase_den        bigint  NOT NULL,
    title               text    NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_chapter_media_id_index ON chapter(media_id, chapter_index);
CREATE INDEX        IF NOT EXISTS idx_chapter_media_id       ON chapter(media_id);
-- Case-insensitive title lookup.
CREATE INDEX        IF NOT EXISTS idx_chapter_title_lower    ON chapter(LOWER(title));

-- AVDictionary entries per chapter, **excluding** the "title" key
-- (consumed into chapter.title). `ordinal` preserves IndexMap insertion
-- order: ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS chapter_metadata (
    chapter_id  uuid    NOT NULL,
    ordinal     integer NOT NULL,
    key         text    NOT NULL,
    value       text    NOT NULL,
    PRIMARY KEY (chapter_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_chapter_metadata_chapter_id ON chapter_metadata(chapter_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_media_checksum ON media(checksum);
CREATE INDEX        IF NOT EXISTS idx_media_video    ON media(video);
CREATE INDEX        IF NOT EXISTS idx_media_audio    ON media(audio);
CREATE INDEX        IF NOT EXISTS idx_media_subtitle ON media(subtitle);
CREATE INDEX        IF NOT EXISTS idx_media_data     ON media(data);
CREATE INDEX        IF NOT EXISTS idx_media_attachment ON media(attachment);

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
    id                                    uuid    NOT NULL PRIMARY KEY,
    audio_track_id                        uuid    NOT NULL,
    cluster_id                            integer NOT NULL,
    name                                  text    NOT NULL,
    speech_duration_ms                    bigint,
    -- Per-track aggregated voiceprint. `voiceprint_vector_id IS NOT NULL`
    -- is the discriminator: when present, the other voiceprint_* columns
    -- carry the full flattened VO; when NULL, they are all NULL.
    voiceprint_vector_id                  uuid,
    voiceprint_dimensions                 integer,
    voiceprint_extracted_at_ms            bigint,
    voiceprint_confidence                 real,
    voiceprint_provenance_model_name      text,
    voiceprint_provenance_model_version   text,
    voiceprint_provenance_prompt_version  text,
    voiceprint_provenance_indexer_version text,
    -- Cross-track identity FK -> person.id; NULL = not yet identified.
    person_id                             uuid
);
CREATE INDEX IF NOT EXISTS idx_speaker_audio_track_id ON speaker(audio_track_id);
CREATE INDEX IF NOT EXISTS idx_speaker_person_id ON speaker(person_id);

CREATE TABLE IF NOT EXISTS user_tag (
    id            uuid   NOT NULL PRIMARY KEY,
    name          text   NOT NULL,
    color_rgba    bigint,
    created_at_ms bigint NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_tag_name ON user_tag(name);

CREATE TABLE IF NOT EXISTS scene_annotation (
    id              uuid    NOT NULL PRIMARY KEY,
    scene_id           uuid    NOT NULL,
    favorite        boolean NOT NULL DEFAULT false,
    rating          smallint,
    note            text    NOT NULL,
    updated_at_ms   bigint  NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scene_annotation_scene_id ON scene_annotation(scene_id);

CREATE TABLE IF NOT EXISTS scene_annotation_user_tag (
    scene_annotation_id  uuid    NOT NULL,
    user_tag_id          uuid    NOT NULL,
    ordinal           integer NOT NULL,
    PRIMARY KEY (scene_annotation_id, user_tag_id)
);
CREATE INDEX IF NOT EXISTS idx_saut_user_tag_id ON scene_annotation_user_tag (user_tag_id);

-- Audio-cluster: the `Audio` facet + `AudioTrack` + `AudioSegment`
-- (+ the `Word` / `index_errors` child tables). Nested value-objects are
-- flattened into real columns; collections ride in child tables with an
-- `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS audio (
    id                     uuid    NOT NULL PRIMARY KEY,
    media_id                 uuid    NOT NULL UNIQUE,
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
    -- `SampleFormat::to_u32` (FFmpeg `AV_SAMPLE_FMT_*` code). Default
    -- `4294967295` (`u32::MAX`) so a freshly-inserted row decodes back as
    -- `SampleFormat::Unknown(u32::MAX)` == `SampleFormat::default()`.
    sample_format             bigint  NOT NULL DEFAULT 4294967295,
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
    has_replay_gain           boolean NOT NULL DEFAULT false,
    replay_gain_track_gain_db real,
    replay_gain_track_peak    real,
    replay_gain_album_gain_db real,
    replay_gain_album_peak    real,
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
    vad_provenance_model_name     text    NOT NULL,
    vad_provenance_model_version  text    NOT NULL,
    vad_provenance_prompt_version text    NOT NULL,
    vad_provenance_indexer_version text   NOT NULL,
    ced_provenance_model_name     text    NOT NULL,
    ced_provenance_model_version  text    NOT NULL,
    ced_provenance_prompt_version text    NOT NULL,
    ced_provenance_indexer_version text   NOT NULL,
    index_status              bigint  NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_audio_track_audio_id ON audio_track(audio_id);

-- AVDictionary entries per audio_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS audio_track_metadata (
    audio_track_id  uuid    NOT NULL,
    ordinal         integer NOT NULL,
    key             text    NOT NULL,
    value           text    NOT NULL,
    PRIMARY KEY (audio_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_audio_track_metadata_audio_track_id ON audio_track_metadata(audio_track_id);

CREATE TABLE IF NOT EXISTS audio_track_index_error (
    audio_track_id uuid    NOT NULL,
    ordinal     integer NOT NULL,
    code        integer NOT NULL,
    message     text    NOT NULL,
    PRIMARY KEY (audio_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_atie_audio_track_id ON audio_track_index_error(audio_track_id);

CREATE TABLE IF NOT EXISTS audio_segment (
    id              uuid    NOT NULL PRIMARY KEY,
    audio_track_id          uuid    NOT NULL,
    index           bigint  NOT NULL,
    span_start_pts  bigint  NOT NULL,
    span_end_pts    bigint  NOT NULL,
    speaker         uuid,
    text_src        text    NOT NULL,
    text_translated text    NOT NULL,
    language        text,
    no_speech_prob  real,
    avg_logprob     real,
    temperature     real,
    -- Per-segment voice embedding. `voice_fingerprint_vector_id IS NOT NULL`
    -- discriminates presence of the flattened VoiceFingerprint VO.
    voice_fingerprint_vector_id                uuid,
    voice_fingerprint_dimensions               integer,
    voice_fingerprint_extracted_at_ms          bigint,
    voice_fingerprint_confidence               real,
    voice_fingerprint_provenance_model_name    text,
    voice_fingerprint_provenance_model_version text,
    voice_fingerprint_provenance_prompt_version text,
    voice_fingerprint_provenance_indexer_version text
);
CREATE INDEX IF NOT EXISTS idx_audio_segment_audio_track_id ON audio_segment(audio_track_id);

CREATE TABLE IF NOT EXISTS audio_segment_word (
    audio_segment_id  uuid    NOT NULL,
    ordinal        integer NOT NULL,
    text           text    NOT NULL,
    span_start_pts bigint  NOT NULL,
    span_end_pts   bigint  NOT NULL,
    score          real    NOT NULL,
    language       text,
    PRIMARY KEY (audio_segment_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_asw_audio_segment_id ON audio_segment_word(audio_segment_id);

-- Data-cluster: the `Data` facet + `DataTrack` (+ the metadata / index_error
-- child tables). Timed-metadata streams (codec_type=data: Sony rtmd / GoPro
-- GPMF / MISB KLV / timecode); presence + descriptor + metadata only — no
-- sample payloads. `codec` / `codec_tag` are plain slugs.

CREATE TABLE IF NOT EXISTS data (
    id                     uuid    NOT NULL PRIMARY KEY,
    media_id               uuid    NOT NULL UNIQUE,
    track_progress_total   bigint  NOT NULL DEFAULT 0,
    track_progress_indexed bigint  NOT NULL DEFAULT 0,
    track_progress_failed  bigint  NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS data_track (
    id                 uuid    NOT NULL PRIMARY KEY,
    data_id            uuid    NOT NULL,
    stream_index       bigint,
    container_track_id bigint,
    codec              text    NOT NULL,
    codec_tag          text    NOT NULL,
    start_pts          bigint,
    start_pts_tb_num   bigint,
    start_pts_tb_den   bigint,
    duration_pts       bigint,
    duration_tb_num    bigint,
    duration_tb_den    bigint,
    nb_packets         bigint,
    byte_size          bigint  NOT NULL DEFAULT 0,
    disposition        bigint  NOT NULL DEFAULT 0,
    index_status       bigint  NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_data_track_data_id ON data_track(data_id);

CREATE TABLE IF NOT EXISTS data_track_metadata (
    data_track_id uuid    NOT NULL,
    ordinal       integer NOT NULL,
    key           text    NOT NULL,
    value         text    NOT NULL,
    PRIMARY KEY (data_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_data_track_metadata_data_track_id ON data_track_metadata(data_track_id);

CREATE TABLE IF NOT EXISTS data_track_index_error (
    data_track_id uuid    NOT NULL,
    ordinal       integer NOT NULL,
    code          integer NOT NULL,
    message       text    NOT NULL,
    PRIMARY KEY (data_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_dtie_data_track_id ON data_track_index_error(data_track_id);

-- Attachment-cluster: the `Attachment` facet + `AttachmentTrack` (+ the
-- metadata / index_error child tables). Attachment streams
-- (codec_type=attachment: fonts / cover art / thumbnails); presence +
-- descriptor + metadata only — NO attachment bytes are stored. The
-- `blob_*` columns are RESERVED and always NULL in v1.

CREATE TABLE IF NOT EXISTS attachment (
    id                     uuid    NOT NULL PRIMARY KEY,
    media_id               uuid    NOT NULL UNIQUE,
    track_progress_total   bigint  NOT NULL DEFAULT 0,
    track_progress_indexed bigint  NOT NULL DEFAULT 0,
    track_progress_failed  bigint  NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS attachment_track (
    id                 uuid    NOT NULL PRIMARY KEY,
    attachment_id      uuid    NOT NULL,
    stream_index       bigint,
    codec              text    NOT NULL,
    filename           text    NOT NULL,
    mimetype           text    NOT NULL,
    byte_size          bigint  NOT NULL DEFAULT 0,
    disposition        bigint  NOT NULL DEFAULT 0,
    index_status       bigint  NOT NULL DEFAULT 0,
    blob_uri           text,
    blob_byte_size     bigint,
    blob_content_type  text
);
CREATE INDEX IF NOT EXISTS idx_attachment_track_attachment_id ON attachment_track(attachment_id);

CREATE TABLE IF NOT EXISTS attachment_track_metadata (
    attachment_track_id uuid    NOT NULL,
    ordinal             integer NOT NULL,
    key                 text    NOT NULL,
    value               text    NOT NULL,
    PRIMARY KEY (attachment_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_attachment_track_metadata_attachment_track_id ON attachment_track_metadata(attachment_track_id);

CREATE TABLE IF NOT EXISTS attachment_track_index_error (
    attachment_track_id uuid    NOT NULL,
    ordinal             integer NOT NULL,
    code                integer NOT NULL,
    message             text    NOT NULL,
    PRIMARY KEY (attachment_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_attie_attachment_track_id ON attachment_track_index_error(attachment_track_id);

-- Video-cluster: the `Video` facet + `VideoTrack` + `Scene` + `Keyframe`
-- (+ per-detection child tables). Nested mediaframe descriptor VOs are
-- flattened into real columns; collections ride in child tables with an
-- `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS video (
    id                     uuid    NOT NULL PRIMARY KEY,
    media_id                 uuid    NOT NULL UNIQUE,
    total_scenes           bigint  NOT NULL DEFAULT 0,
    track_progress_total   bigint  NOT NULL DEFAULT 0,
    track_progress_indexed bigint  NOT NULL DEFAULT 0,
    track_progress_failed  bigint  NOT NULL DEFAULT 0,
    -- FK -> cover_keyframe.id (the video's poster); NULL = no cover yet.
    cover_keyframe_id      uuid
);

CREATE TABLE IF NOT EXISTS video_track (
    id                       uuid    NOT NULL PRIMARY KEY,
    video_id                 uuid    NOT NULL,
    stream_index             bigint,
    container_track_id       bigint,
    start_pts                bigint,
    start_pts_tb_num         bigint,
    start_pts_tb_den         bigint,
    duration_pts             bigint,
    duration_tb_num          bigint,
    duration_tb_den          bigint,
    codec                    text    NOT NULL,
    profile                  text,
    level                    integer,
    bit_rate                 bigint  NOT NULL DEFAULT 0,
    nb_frames                bigint,
    has_b_frames             boolean NOT NULL DEFAULT false,
    closed_gop               boolean,
    bits_per_raw_sample      smallint,
    width                    bigint  NOT NULL DEFAULT 0,
    height                   bigint  NOT NULL DEFAULT 0,
    has_visible_rect         boolean NOT NULL DEFAULT false,
    visible_rect_x           bigint,
    visible_rect_y           bigint,
    visible_rect_w           bigint,
    visible_rect_h           bigint,
    sar_num                  bigint  NOT NULL DEFAULT 1,
    sar_den                  bigint  NOT NULL DEFAULT 1,
    pixel_format             bigint  NOT NULL DEFAULT 0,
    color_primaries          bigint  NOT NULL DEFAULT 0,
    color_transfer           bigint  NOT NULL DEFAULT 0,
    color_matrix             bigint  NOT NULL DEFAULT 0,
    color_range              bigint  NOT NULL DEFAULT 0,
    color_chroma_location    bigint  NOT NULL DEFAULT 0,
    has_hdr_static           boolean NOT NULL DEFAULT false,
    hdr_has_mastering        boolean NOT NULL DEFAULT false,
    hdr_primary_r_x          bigint,
    hdr_primary_r_y          bigint,
    hdr_primary_g_x          bigint,
    hdr_primary_g_y          bigint,
    hdr_primary_b_x          bigint,
    hdr_primary_b_y          bigint,
    hdr_white_point_x        bigint,
    hdr_white_point_y        bigint,
    hdr_max_luminance        bigint,
    hdr_min_luminance        bigint,
    hdr_has_content_light    boolean NOT NULL DEFAULT false,
    hdr_max_cll              bigint,
    hdr_max_fall             bigint,
    rotation                 bigint  NOT NULL DEFAULT 0,
    fr_num                   bigint  NOT NULL DEFAULT 1,
    fr_den                   bigint  NOT NULL DEFAULT 1,
    fr_is_vfr                boolean NOT NULL DEFAULT false,
    -- `AVStream.avg_frame_rate` — defaults `0/1` (= `FrameRate::default()`,
    -- absent). For CFR content this equals (fr_num, fr_den); for VFR the
    -- two diverge.
    avg_fr_num               bigint  NOT NULL DEFAULT 0,
    avg_fr_den               bigint  NOT NULL DEFAULT 1,
    field_order              bigint  NOT NULL DEFAULT 0,
    stereo_mode              bigint,
    has_dovi                 boolean NOT NULL DEFAULT false,
    dovi_profile             smallint,
    dovi_level               smallint,
    dovi_rpu_present         boolean,
    dovi_el_present          boolean,
    dovi_bl_signal_compat_id smallint,
    has_embedded_captions    boolean NOT NULL DEFAULT false,
    disposition              bigint  NOT NULL DEFAULT 0,
    is_primary               boolean NOT NULL DEFAULT false,
    auto_selected            boolean NOT NULL DEFAULT false,
    provenance_model_name    text    NOT NULL,
    provenance_model_version text    NOT NULL,
    provenance_prompt_version text   NOT NULL,
    provenance_indexer_version text  NOT NULL,
    index_status             bigint  NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_video_track_video_id ON video_track(video_id);

-- AVDictionary entries per video_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS video_track_metadata (
    video_track_id  uuid    NOT NULL,
    ordinal         integer NOT NULL,
    key             text    NOT NULL,
    value           text    NOT NULL,
    PRIMARY KEY (video_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_video_track_metadata_video_track_id ON video_track_metadata(video_track_id);

CREATE TABLE IF NOT EXISTS video_track_index_error (
    video_track_id uuid    NOT NULL,
    ordinal     integer NOT NULL,
    code        integer NOT NULL,
    message     text    NOT NULL,
    PRIMARY KEY (video_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_vtie_video_track_id ON video_track_index_error(video_track_id);

CREATE TABLE IF NOT EXISTS scene (
    id              uuid    NOT NULL PRIMARY KEY,
    video_track_id          uuid    NOT NULL,
    index           bigint  NOT NULL,
    span_start_pts  bigint  NOT NULL,
    span_end_pts    bigint  NOT NULL,
    detector        text    NOT NULL,
    description     text    NOT NULL
);
CREATE INDEX        IF NOT EXISTS idx_scene_video_track_id   ON scene(video_track_id);
CREATE INDEX        IF NOT EXISTS idx_scene_detector ON scene(detector);
CREATE UNIQUE INDEX IF NOT EXISTS idx_scene_video_track_id_index ON scene(video_track_id, index);

-- Thumbnail image + storage descriptor. FK target of keyframe.thumbnail_id,
-- so it is declared BEFORE keyframe. `kind` is a ThumbnailKind slug
-- (`filesystem`/`database`/`remote`); exactly one payload slot is
-- populated per kind: `data` (bytea) for `database`, `location` (text)
-- for `filesystem`/`remote` — the other is NULL.
CREATE TABLE IF NOT EXISTS thumbnail (
    id        uuid   NOT NULL PRIMARY KEY,
    kind      text   NOT NULL,
    data      bytea,
    location  text,
    mime      text   NOT NULL,
    width     bigint NOT NULL,
    height    bigint NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_thumbnail_kind ON thumbnail(kind);

CREATE TABLE IF NOT EXISTS keyframe (
    id                        uuid   NOT NULL PRIMARY KEY,
    -- Nullable: a scene keyframe carries its `Scene` FK; a cover keyframe
    -- (role = 'cover') has no scene parent (it is stored in cover_keyframe,
    -- keyed by video_id). `role` self-describes which case a row is.
    scene_id                    uuid,
    role                      text   NOT NULL DEFAULT 'scene',
    pts                       bigint NOT NULL,
    thumbnail_id              uuid   NOT NULL,
    width                     bigint NOT NULL,
    height                    bigint NOT NULL,
    extractor                 text   NOT NULL,
    vlm_description_src       text   NOT NULL,
    vlm_description_translated text  NOT NULL,
    vlm_shot_type             text   NOT NULL,
    horizon_angle             real   NOT NULL,
    horizon_confidence        real   NOT NULL,
    aesthetics_overall_score  real   NOT NULL,
    aesthetics_is_utility     boolean NOT NULL DEFAULT false
);
CREATE INDEX IF NOT EXISTS idx_keyframe_scene_id ON keyframe(scene_id);
CREATE INDEX IF NOT EXISTS idx_keyframe_thumbnail_id ON keyframe(thumbnail_id);

-- The video's cover/poster keyframe. Mirrors `keyframe` but parented by
-- `video_id` (FK -> video.id), NOT by a scene — a cover keyframe attaches
-- at the video level. It reuses the existing `Thumbnail` entity
-- (thumbnail_id) and the existing `keyframe_*` detection child tables,
-- keyed by this row's `id` (a cover keyframe id is a valid keyframe_id).
CREATE TABLE IF NOT EXISTS cover_keyframe (
    id                        uuid   NOT NULL PRIMARY KEY,
    video_id                  uuid   NOT NULL,        -- FK -> video.id (NOT scene)
    pts                       bigint NOT NULL,
    thumbnail_id              uuid   NOT NULL,        -- FK -> thumbnail.id
    width                     bigint NOT NULL,
    height                    bigint NOT NULL,
    extractor                 text   NOT NULL,
    role                      text   NOT NULL DEFAULT 'cover',
    vlm_description_src       text   NOT NULL,
    vlm_description_translated text  NOT NULL,
    vlm_shot_type             text   NOT NULL,
    horizon_angle             real   NOT NULL,
    horizon_confidence        real   NOT NULL,
    aesthetics_overall_score  real   NOT NULL,
    aesthetics_is_utility     boolean NOT NULL DEFAULT false
);
CREATE INDEX IF NOT EXISTS idx_cover_keyframe_video_id ON cover_keyframe(video_id);
CREATE INDEX IF NOT EXISTS idx_cover_keyframe_thumbnail_id ON cover_keyframe(thumbnail_id);

-- detection child tables (per-kind, keyed by (keyframe, ordinal))
CREATE TABLE IF NOT EXISTS keyframe_classification (
    keyframe_id   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    label      text    NOT NULL,
    confidence real    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_classification_keyframe_id ON keyframe_classification(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_object (
    keyframe_id   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    label      text    NOT NULL,
    confidence real    NOT NULL,
    has_bbox   boolean NOT NULL DEFAULT false,
    bbox_x     real,
    bbox_y     real,
    bbox_w     real,
    bbox_h     real,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_object_keyframe_id ON keyframe_object(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_action (
    keyframe_id   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    label      text    NOT NULL,
    confidence real    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_action_keyframe_id ON keyframe_action(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_text_detection (
    keyframe_id   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    text       text    NOT NULL,
    confidence real    NOT NULL,
    bbox_x     real    NOT NULL,
    bbox_y     real    NOT NULL,
    bbox_w     real    NOT NULL,
    bbox_h     real    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_text_detection_keyframe_id ON keyframe_text_detection(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_barcode (
    keyframe_id   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    payload    text    NOT NULL,
    symbology  text    NOT NULL,
    confidence real    NOT NULL,
    bbox_x     real    NOT NULL,
    bbox_y     real    NOT NULL,
    bbox_w     real    NOT NULL,
    bbox_h     real    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_barcode_keyframe_id ON keyframe_barcode(keyframe_id);

-- attention / objectness saliency share this table; `kind` discriminates
-- (0 = attention, 1 = objectness).
CREATE TABLE IF NOT EXISTS keyframe_saliency (
    keyframe_id   uuid     NOT NULL,
    kind       smallint NOT NULL,
    ordinal    integer  NOT NULL,
    bbox_x     real     NOT NULL,
    bbox_y     real     NOT NULL,
    bbox_w     real     NOT NULL,
    bbox_h     real     NOT NULL,
    confidence real     NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_saliency_keyframe_id ON keyframe_saliency(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_document_segment (
    keyframe_id   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    tl_x       real    NOT NULL,
    tl_y       real    NOT NULL,
    tr_x       real    NOT NULL,
    tr_y       real    NOT NULL,
    br_x       real    NOT NULL,
    br_y       real    NOT NULL,
    bl_x       real    NOT NULL,
    bl_y       real    NOT NULL,
    confidence real    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_document_segment_keyframe_id ON keyframe_document_segment(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_color (
    keyframe_id   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    rgba       bigint  NOT NULL,
    name       text    NOT NULL,
    percentage real    NOT NULL,
    population bigint  NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_color_keyframe_id ON keyframe_color(keyframe_id);

-- humans + animals share these shapes; `scope` (0 = human, 1 = animal)
-- discriminates.
CREATE TABLE IF NOT EXISTS keyframe_subject (
    keyframe_id   uuid     NOT NULL,
    scope      smallint NOT NULL,
    ordinal    integer  NOT NULL,
    label      text     NOT NULL,
    confidence real     NOT NULL,
    bbox_x     real     NOT NULL,
    bbox_y     real     NOT NULL,
    bbox_w     real     NOT NULL,
    bbox_h     real     NOT NULL,
    PRIMARY KEY (keyframe_id, scope, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_subject_keyframe_id ON keyframe_subject(keyframe_id);

-- humans faces + face_rectangles share this shape; `kind`
-- (0 = faces, 1 = face_rectangles) discriminates.
CREATE TABLE IF NOT EXISTS keyframe_face (
    keyframe_id        uuid     NOT NULL,
    kind            smallint NOT NULL,
    ordinal         integer  NOT NULL,
    bbox_x          real     NOT NULL,
    bbox_y          real     NOT NULL,
    bbox_w          real     NOT NULL,
    bbox_h          real     NOT NULL,
    confidence      real     NOT NULL,
    capture_quality real     NOT NULL,
    roll            real     NOT NULL,
    yaw             real     NOT NULL,
    pitch           real     NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_keyframe_id ON keyframe_face(keyframe_id);

-- 2-D body pose: humans + animals share this shape; `scope` discriminates.
CREATE TABLE IF NOT EXISTS keyframe_body_pose (
    keyframe_id   uuid     NOT NULL,
    scope      smallint NOT NULL,
    ordinal    integer  NOT NULL,
    bbox_x     real     NOT NULL,
    bbox_y     real     NOT NULL,
    bbox_w     real     NOT NULL,
    bbox_h     real     NOT NULL,
    confidence real     NOT NULL,
    PRIMARY KEY (keyframe_id, scope, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_keyframe_id ON keyframe_body_pose(keyframe_id);

-- joints for 2-D body / hand pose rows; `scope`
-- (0 = human-body, 1 = animal-body, 2 = hand) discriminates which parent.
CREATE TABLE IF NOT EXISTS keyframe_body_pose_joint (
    keyframe_id       uuid     NOT NULL,
    scope          smallint NOT NULL,
    parent_ordinal integer  NOT NULL,
    ordinal        integer  NOT NULL,
    name           text     NOT NULL,
    x              real     NOT NULL,
    y              real     NOT NULL,
    confidence     real     NOT NULL,
    PRIMARY KEY (keyframe_id, scope, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_joint_keyframe_id ON keyframe_body_pose_joint(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_hand_pose (
    keyframe_id   uuid     NOT NULL,
    ordinal    integer  NOT NULL,
    bbox_x     real     NOT NULL,
    bbox_y     real     NOT NULL,
    bbox_w     real     NOT NULL,
    bbox_h     real     NOT NULL,
    confidence real     NOT NULL,
    chirality  smallint NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_hand_pose_keyframe_id ON keyframe_hand_pose(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d (
    keyframe_id          uuid     NOT NULL,
    ordinal           integer  NOT NULL,
    confidence        real     NOT NULL,
    body_height       real     NOT NULL,
    height_estimation smallint NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_3d_keyframe_id ON keyframe_body_pose_3d(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d_joint (
    keyframe_id       uuid    NOT NULL,
    parent_ordinal integer NOT NULL,
    ordinal        integer NOT NULL,
    name           text    NOT NULL,
    x              real    NOT NULL,
    y              real    NOT NULL,
    z              real    NOT NULL,
    confidence     real    NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_3d_joint_keyframe_id ON keyframe_body_pose_3d_joint(keyframe_id);

-- instance + whole-frame segmentation masks share this shape; `kind`
-- (0 = instance, 1 = segmentation) discriminates.
CREATE TABLE IF NOT EXISTS keyframe_mask (
    keyframe_id       uuid     NOT NULL,
    kind           smallint NOT NULL,
    ordinal        integer  NOT NULL,
    bbox_x         real     NOT NULL,
    bbox_y         real     NOT NULL,
    bbox_w         real     NOT NULL,
    bbox_h         real     NOT NULL,
    confidence     real     NOT NULL,
    instance_index bigint,
    width          bigint   NOT NULL,
    height         bigint   NOT NULL,
    data           bytea    NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_mask_keyframe_id ON keyframe_mask(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_face_landmarks (
    keyframe_id   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    bbox_x     real    NOT NULL,
    bbox_y     real    NOT NULL,
    bbox_w     real    NOT NULL,
    bbox_h     real    NOT NULL,
    confidence real    NOT NULL,
    PRIMARY KEY (keyframe_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmarks_keyframe_id ON keyframe_face_landmarks(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_region (
    keyframe_id       uuid    NOT NULL,
    parent_ordinal integer NOT NULL,
    ordinal        integer NOT NULL,
    name           text    NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmark_region_keyframe_id ON keyframe_face_landmark_region(keyframe_id);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_point (
    keyframe_id       uuid    NOT NULL,
    parent_ordinal integer NOT NULL,
    region_ordinal integer NOT NULL,
    ordinal        integer NOT NULL,
    x              real    NOT NULL,
    y              real    NOT NULL,
    PRIMARY KEY (keyframe_id, parent_ordinal, region_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmark_point_keyframe_id ON keyframe_face_landmark_point(keyframe_id);

-- VLM open-vocab labels — `kind` (0 = categories, 1 = tags, 2 = objects,
-- 3 = subjects, 4 = mood, 5 = emotion, 6 = lighting) discriminates the
-- source `Vec<LocalizedText>` slice.
CREATE TABLE IF NOT EXISTS keyframe_vlm_label (
    keyframe_id   uuid     NOT NULL,
    kind       smallint NOT NULL,
    ordinal    integer  NOT NULL,
    src        text     NOT NULL,
    translated text     NOT NULL,
    PRIMARY KEY (keyframe_id, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_vlm_label_keyframe_id ON keyframe_vlm_label(keyframe_id);

-- Subtitle-cluster: the `Subtitle` facet + `SubtitleTrack` +
-- `SubtitleCue` (+ the `index_errors` child table). Nested value-objects
-- are flattened into real columns; collections ride in child tables with
-- an `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS subtitle (
    id                     uuid    NOT NULL PRIMARY KEY,
    media_id                 uuid    NOT NULL,
    track_progress_total   bigint  NOT NULL DEFAULT 0,
    track_progress_indexed bigint  NOT NULL DEFAULT 0,
    track_progress_failed  bigint  NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_subtitle_media_id ON subtitle(media_id);

CREATE TABLE IF NOT EXISTS subtitle_track (
    id                         uuid    NOT NULL PRIMARY KEY,
    subtitle_id                uuid    NOT NULL,
    stream_index               bigint,
    container_track_id         bigint,
    codec                      text    NOT NULL,
    format                     text    NOT NULL,
    origin                     integer NOT NULL DEFAULT 0,
    language                   text,
    title                      text    NOT NULL,
    disposition                bigint  NOT NULL DEFAULT 0,
    is_primary                 boolean NOT NULL DEFAULT false,
    auto_selected              boolean NOT NULL DEFAULT false,
    duration_pts               bigint,
    duration_tb_num            bigint,
    duration_tb_den            bigint,
    cue_count                  bigint  NOT NULL DEFAULT 0,
    provenance_model_name      text    NOT NULL,
    provenance_model_version   text    NOT NULL,
    provenance_prompt_version  text    NOT NULL,
    provenance_indexer_version text    NOT NULL,
    source_checksum            bytea,
    character_encoding         text    NOT NULL,
    bom_present                boolean NOT NULL DEFAULT false,
    is_sdh                     boolean NOT NULL DEFAULT false,
    is_closed_caption          boolean NOT NULL DEFAULT false,
    is_translation             boolean NOT NULL DEFAULT false,
    kind                       smallint NOT NULL DEFAULT 0,
    coverage_ratio             real,
    is_empty                   boolean NOT NULL DEFAULT false,
    first_cue_pts              bigint,
    first_cue_tb_num           bigint,
    first_cue_tb_den           bigint,
    last_cue_pts               bigint,
    last_cue_tb_num            bigint,
    last_cue_tb_den            bigint,
    index_status               bigint  NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_subtitle_id ON subtitle_track(subtitle_id);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_codec       ON subtitle_track(codec);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_language    ON subtitle_track(language);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_origin      ON subtitle_track(origin);

-- AVDictionary entries per subtitle_track. `ordinal` preserves IndexMap
-- insertion order; ORDER BY ordinal yields the original sequence.
CREATE TABLE IF NOT EXISTS subtitle_track_metadata (
    subtitle_track_id  uuid    NOT NULL,
    ordinal            integer NOT NULL,
    key                text    NOT NULL,
    value              text    NOT NULL,
    PRIMARY KEY (subtitle_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_metadata_subtitle_track_id ON subtitle_track_metadata(subtitle_track_id);

CREATE TABLE IF NOT EXISTS subtitle_track_index_error (
    subtitle_track_id uuid    NOT NULL,
    ordinal        integer NOT NULL,
    code           integer NOT NULL,
    message        text    NOT NULL,
    PRIMARY KEY (subtitle_track_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_stie_subtitle_track_id ON subtitle_track_index_error(subtitle_track_id);

-- ── subtitle_cue: polymorphic base (schema/subtitle_cues.md rev 5) ──────
-- The `kind` SMALLINT discriminates which detail table (if any) the cue
-- joins to. `text_src`/`text_translated` are the plain (style-stripped)
-- text — `""` legal for un-OCR'd bitmap cues and ASS-style display.
CREATE TABLE IF NOT EXISTS subtitle_cue (
    id                  uuid    NOT NULL PRIMARY KEY,
    subtitle_track_id   uuid    NOT NULL,
    ordinal             bigint  NOT NULL,
    span_start_pts      bigint  NOT NULL,
    span_end_pts        bigint  NOT NULL,
    text_src            text    NOT NULL,
    text_translated     text    NOT NULL,
    kind                smallint NOT NULL
);
CREATE INDEX        IF NOT EXISTS idx_subtitle_cue_subtitle_track_id         ON subtitle_cue(subtitle_track_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subtitle_cue_subtitle_track_id_ordinal ON subtitle_cue(subtitle_track_id, ordinal);
CREATE INDEX        IF NOT EXISTS idx_subtitle_cue_kind                      ON subtitle_cue(kind);

-- ── WebVTT detail ───────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS subtitle_cue_vtt (
    id              uuid     NOT NULL PRIMARY KEY,
    cue_identifier  text     NOT NULL,
    vertical        smallint,
    line_value      text     NOT NULL,
    line_align      smallint,
    position_value  text     NOT NULL,
    position_align  smallint,
    size_value      real,
    text_align      smallint,
    region_id       uuid,
    voice           text     NOT NULL,
    styled_text     text     NOT NULL
);

-- ── ASS / SSA detail ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS subtitle_cue_ass (
    id            uuid     NOT NULL PRIMARY KEY,
    layer         integer  NOT NULL DEFAULT 0,
    style_id      uuid     NOT NULL,
    name          text     NOT NULL,
    margin_l      integer  NOT NULL DEFAULT 0,
    margin_r      integer  NOT NULL DEFAULT 0,
    margin_v      integer  NOT NULL DEFAULT 0,
    effect        text     NOT NULL,
    styled_text   text     NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_ass_style_id ON subtitle_cue_ass(style_id);

-- ── LRC detail + Enhanced word-level child ──────────────────────────────
CREATE TABLE IF NOT EXISTS subtitle_cue_lrc (
    id                uuid     NOT NULL PRIMARY KEY,
    has_word_timing   boolean  NOT NULL DEFAULT false
);

CREATE TABLE IF NOT EXISTS subtitle_cue_lrc_word (
    subtitle_cue_id   uuid     NOT NULL,
    ordinal           integer  NOT NULL,
    text              text     NOT NULL,
    start_pts         bigint   NOT NULL,
    PRIMARY KEY (subtitle_cue_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_lrc_word_cue_id ON subtitle_cue_lrc_word(subtitle_cue_id);

-- ── Per-track WebVTT regions / style blocks ─────────────────────────────
CREATE TABLE IF NOT EXISTS subtitle_track_vtt_region (
    id                  uuid    NOT NULL PRIMARY KEY,
    subtitle_track_id   uuid    NOT NULL,
    name                text    NOT NULL,
    width               real    NOT NULL,
    lines               bigint  NOT NULL,
    region_anchor_x     real    NOT NULL,
    region_anchor_y     real    NOT NULL,
    viewport_anchor_x   real    NOT NULL,
    viewport_anchor_y   real    NOT NULL,
    scroll_up           boolean NOT NULL DEFAULT false
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_vtt_region_track_id ON subtitle_track_vtt_region(subtitle_track_id);

CREATE TABLE IF NOT EXISTS subtitle_track_vtt_style (
    id                  uuid    NOT NULL PRIMARY KEY,
    subtitle_track_id   uuid    NOT NULL,
    ordinal             integer NOT NULL,
    css_text            text    NOT NULL
);
CREATE INDEX        IF NOT EXISTS idx_subtitle_track_vtt_style_track_id         ON subtitle_track_vtt_style(subtitle_track_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subtitle_track_vtt_style_track_id_ordinal ON subtitle_track_vtt_style(subtitle_track_id, ordinal);

-- ── Per-track ASS V4+ Style rows ────────────────────────────────────────
CREATE TABLE IF NOT EXISTS subtitle_track_ass_style (
    id                  uuid     NOT NULL PRIMARY KEY,
    subtitle_track_id   uuid     NOT NULL,
    name                text     NOT NULL,
    fontname            text     NOT NULL,
    fontsize            real     NOT NULL,
    primary_colour      bigint   NOT NULL,
    secondary_colour    bigint   NOT NULL,
    outline_colour      bigint   NOT NULL,
    back_colour         bigint   NOT NULL,
    bold                boolean  NOT NULL,
    italic              boolean  NOT NULL,
    underline           boolean  NOT NULL,
    strikeout           boolean  NOT NULL,
    scale_x             integer  NOT NULL,
    scale_y             integer  NOT NULL,
    spacing             integer  NOT NULL,
    angle               real     NOT NULL,
    border_style        smallint NOT NULL,
    outline             real     NOT NULL,
    shadow              real     NOT NULL,
    alignment           smallint NOT NULL,
    margin_l            integer  NOT NULL,
    margin_r            integer  NOT NULL,
    margin_v            integer  NOT NULL,
    encoding            integer  NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_ass_style_track_id ON subtitle_track_ass_style(subtitle_track_id);

-- ── Per-track LRC header metadata (1:1) ─────────────────────────────────
CREATE TABLE IF NOT EXISTS subtitle_track_lrc_metadata (
    subtitle_track_id   uuid     NOT NULL PRIMARY KEY,
    title               text     NOT NULL,
    artist              text     NOT NULL,
    album               text     NOT NULL,
    author              text     NOT NULL,
    creator             text     NOT NULL,
    length              text     NOT NULL,
    offset_ms           integer  NOT NULL DEFAULT 0
);

-- ── Long-tail text-format detail tables (issue #56) ──────────────────────

-- MicroDVD: inline `{y:i}` style codes on the cue.
CREATE TABLE IF NOT EXISTS subtitle_cue_micro_dvd (
    id            uuid     NOT NULL PRIMARY KEY,
    styled_text   text     NOT NULL
);

-- SubViewer: `[br]`/`[b]`/`[i]`/`[u]` inline tags.
CREATE TABLE IF NOT EXISTS subtitle_cue_sub_viewer (
    id            uuid     NOT NULL PRIMARY KEY,
    styled_text   text     NOT NULL
);

-- SBV: unit marker (no payload columns); FK PK only.
CREATE TABLE IF NOT EXISTS subtitle_cue_sbv (
    id            uuid     NOT NULL PRIMARY KEY
);

-- TTML: optional region/style FKs + XML id + raw XML fragment.
CREATE TABLE IF NOT EXISTS subtitle_cue_ttml (
    id            uuid     NOT NULL PRIMARY KEY,
    region_id     uuid,
    style_id      uuid,
    xml_id        text     NOT NULL,
    styled_text   text     NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_ttml_region_id ON subtitle_cue_ttml(region_id);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_ttml_style_id  ON subtitle_cue_ttml(style_id);

-- SAMI: class selector + raw HTML-like fragment.
CREATE TABLE IF NOT EXISTS subtitle_cue_sami (
    id            uuid     NOT NULL PRIMARY KEY,
    class_name    text     NOT NULL,
    styled_text   text     NOT NULL
);

-- ── Long-tail bitmap / broadcast detail tables (issue #56) ────────────────

-- VobSub: bitmap blob + per-cue geometry + colour/contrast indices into
-- the per-track palette. Indices ride as packed LE u32 to keep the row
-- fixed-arity; bitmap is `bytea`.
CREATE TABLE IF NOT EXISTS subtitle_cue_vob_sub (
    id                uuid     NOT NULL PRIMARY KEY,
    palette_id        uuid     NOT NULL,
    bitmap            bytea    NOT NULL,
    width             bigint   NOT NULL,
    height            bigint   NOT NULL,
    pos_x             integer  NOT NULL,
    pos_y             integer  NOT NULL,
    color_indices     bigint   NOT NULL,
    contrast_indices  bigint   NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_cue_vob_sub_palette_id ON subtitle_cue_vob_sub(palette_id);

-- PGS: bitmap + per-cue palette bytes + geometry + composition state.
CREATE TABLE IF NOT EXISTS subtitle_cue_pgs (
    id                 uuid      NOT NULL PRIMARY KEY,
    bitmap             bytea     NOT NULL,
    width              bigint    NOT NULL,
    height             bigint    NOT NULL,
    pos_x              integer   NOT NULL,
    pos_y              integer   NOT NULL,
    palette_bytes      bytea     NOT NULL,
    composition_state  smallint  NOT NULL
);

-- CEA-608: channel + PAC byte pair + decoded line text.
CREATE TABLE IF NOT EXISTS subtitle_cue_cea_608 (
    id              uuid      NOT NULL PRIMARY KEY,
    channel         smallint  NOT NULL,
    pac_byte_pair   bigint    NOT NULL,
    styled_text     text      NOT NULL
);

-- EBU STL: TTI block fields + decoded line text.
CREATE TABLE IF NOT EXISTS subtitle_cue_ebu_stl (
    id               uuid      NOT NULL PRIMARY KEY,
    subtitle_number  bigint    NOT NULL,
    cumulative       boolean   NOT NULL DEFAULT false,
    vertical_pos     integer   NOT NULL,
    justification    smallint  NOT NULL,
    styled_text      text      NOT NULL
);

-- ── Per-track long-tail aggregates (issue #56) ───────────────────────────

-- TTML `<region>` rows referenced by `subtitle_cue_ttml.region_id`.
CREATE TABLE IF NOT EXISTS subtitle_track_ttml_region (
    id                  uuid    NOT NULL PRIMARY KEY,
    subtitle_track_id   uuid    NOT NULL,
    xml_id              text    NOT NULL,
    xml_attrs           text    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_ttml_region_track_id ON subtitle_track_ttml_region(subtitle_track_id);

-- TTML `<style>` rows referenced by `subtitle_cue_ttml.style_id`.
CREATE TABLE IF NOT EXISTS subtitle_track_ttml_style (
    id                  uuid    NOT NULL PRIMARY KEY,
    subtitle_track_id   uuid    NOT NULL,
    xml_id              text    NOT NULL,
    xml_attrs           text    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_ttml_style_track_id ON subtitle_track_ttml_style(subtitle_track_id);

-- SAMI `<STYLE>` class rows referenced by `subtitle_cue_sami.class_name`.
CREATE TABLE IF NOT EXISTS subtitle_track_sami_style (
    id                  uuid    NOT NULL PRIMARY KEY,
    subtitle_track_id   uuid    NOT NULL,
    class_name          text    NOT NULL,
    css_text            text    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_sami_style_track_id ON subtitle_track_sami_style(subtitle_track_id);

-- DVD VobSub palette: 16-entry RGB lookup table. The array column rides
-- as `BIGINT[]` (one `BIGINT` per `0x00RRGGBB` u32). Each row is
-- referenced by `subtitle_cue_vob_sub.palette_id`.
CREATE TABLE IF NOT EXISTS subtitle_track_vob_sub_palette (
    id                  uuid     NOT NULL PRIMARY KEY,
    subtitle_track_id   uuid     NOT NULL,
    entries             bigint[] NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_subtitle_track_vob_sub_palette_track_id ON subtitle_track_vob_sub_palette(subtitle_track_id);
