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
    audio_track_id              uuid    NOT NULL,
    cluster_id          integer NOT NULL,
    name                text    NOT NULL,
    speech_duration_ms  bigint
);
CREATE INDEX IF NOT EXISTS idx_speaker_audio_track_id ON speaker(audio_track_id);

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
    temperature     real
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
    track_progress_failed  bigint  NOT NULL DEFAULT 0
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

CREATE TABLE IF NOT EXISTS keyframe (
    id                        uuid   NOT NULL PRIMARY KEY,
    scene_id                    uuid   NOT NULL,
    pts                       bigint NOT NULL,
    data                      bytea  NOT NULL,
    mime                      text   NOT NULL,
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
    source_path_volume         uuid,
    source_path                text,
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
