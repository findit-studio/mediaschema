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
    location_path_hash  bytea   NOT NULL,
    watched_location_id uuid    NOT NULL,
    watch_volume        uuid    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_file_media_id            ON media_file(media_id);
CREATE INDEX IF NOT EXISTS idx_media_file_watched_location_id ON media_file(watched_location_id);
-- Natural-key uniqueness of a copy: one path per volume. Hashing the path
-- with SHA-256 sidesteps MySQL/InnoDB's prefix-length requirement for
-- `UNIQUE` indexes over variable-length `TEXT` (where a truncated prefix
-- would falsely collide two long paths sharing a prefix); same shape
-- across pg/mysql/sqlite. `location_path_hash` is computed at write time
-- from the same canonical `location_path` string.
CREATE UNIQUE INDEX IF NOT EXISTS idx_media_file_path
    ON media_file(location_volume, location_path_hash);
-- Prefix-lookup index on the plain path for `LIKE 'prefix/%'` scans.
CREATE INDEX        IF NOT EXISTS idx_media_file_path_prefix
    ON media_file(location_volume, location_path);

CREATE TABLE IF NOT EXISTS speaker (
    id                                  uuid    NOT NULL PRIMARY KEY,
    parent                              uuid    NOT NULL,
    cluster_id                          integer NOT NULL,
    name                                text    NOT NULL,
    speech_duration_ms                  bigint,
    -- Per-track aggregated voiceprint. `voiceprint_vector_id IS NOT NULL`
    -- is the discriminator: when present, the other voiceprint_* columns
    -- carry the full flattened VO; when NULL, they are all NULL.
    voiceprint_vector_id                uuid,
    voiceprint_dimensions               integer,
    voiceprint_extracted_at_ms          bigint,
    voiceprint_confidence               real,
    voiceprint_provenance_model_name    text,
    voiceprint_provenance_model_version text,
    voiceprint_provenance_prompt_version text,
    voiceprint_provenance_indexer_version text,
    -- Cross-track identity FK -> person.id; NULL = not yet identified.
    person                              uuid
);
CREATE INDEX IF NOT EXISTS idx_speaker_parent ON speaker(parent);
CREATE INDEX IF NOT EXISTS idx_speaker_person ON speaker(person);

CREATE TABLE IF NOT EXISTS person (
    id                                    uuid     NOT NULL PRIMARY KEY,
    name                                  text     NOT NULL,
    -- 0 = AutoMatched, 1 = UserConfirmed.
    confidence                            smallint NOT NULL,
    -- Aggregated canonical voiceprint (centroid across linked Speakers).
    -- `voiceprint_vector_id IS NOT NULL` discriminates presence of the
    -- flattened VoiceFingerprint VO.
    voiceprint_vector_id                  uuid,
    voiceprint_dimensions                 integer,
    voiceprint_extracted_at_ms            bigint,
    voiceprint_confidence                 real,
    voiceprint_provenance_model_name      text,
    voiceprint_provenance_model_version   text,
    voiceprint_provenance_prompt_version  text,
    voiceprint_provenance_indexer_version text,
    created_at_ms                         bigint   NOT NULL,
    updated_at_ms                         bigint   NOT NULL
);
-- Supports "find Persons by embedding model" during re-extraction.
CREATE INDEX IF NOT EXISTS idx_person_voiceprint_model
    ON person(voiceprint_provenance_model_name, voiceprint_provenance_model_version);

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
    span_tb_num     bigint  NOT NULL,
    span_tb_den     bigint  NOT NULL,
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
CREATE INDEX IF NOT EXISTS idx_audio_segment_parent ON audio_segment(parent);

CREATE TABLE IF NOT EXISTS audio_segment_word (
    audio_segment  uuid    NOT NULL,
    ordinal        integer NOT NULL,
    text           text    NOT NULL,
    span_start_pts bigint  NOT NULL,
    span_end_pts   bigint  NOT NULL,
    span_tb_num    bigint  NOT NULL,
    span_tb_den    bigint  NOT NULL,
    score          real    NOT NULL,
    language       text,
    PRIMARY KEY (audio_segment, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_asw_audio_segment ON audio_segment_word(audio_segment);

-- Video-cluster: the `Video` facet + `VideoTrack` + `Scene` + `Keyframe`
-- (+ per-detection child tables). Nested mediaframe descriptor VOs are
-- flattened into real columns; collections ride in child tables with an
-- `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS video (
    id                     uuid    NOT NULL PRIMARY KEY,
    parent                 uuid    NOT NULL UNIQUE,
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
    video_track uuid    NOT NULL,
    ordinal     integer NOT NULL,
    code        integer NOT NULL,
    message     text    NOT NULL,
    PRIMARY KEY (video_track, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_vtie_video_track ON video_track_index_error(video_track);

CREATE TABLE IF NOT EXISTS scene (
    id              uuid    NOT NULL PRIMARY KEY,
    parent          uuid    NOT NULL,
    index           bigint  NOT NULL,
    span_start_pts  bigint  NOT NULL,
    span_end_pts    bigint  NOT NULL,
    span_tb_num     bigint  NOT NULL,
    span_tb_den     bigint  NOT NULL,
    detector        text    NOT NULL,
    description     text    NOT NULL
);
CREATE INDEX        IF NOT EXISTS idx_scene_parent   ON scene(parent);
CREATE INDEX        IF NOT EXISTS idx_scene_detector ON scene(detector);
CREATE UNIQUE INDEX IF NOT EXISTS idx_scene_parent_index ON scene(parent, index);

CREATE TABLE IF NOT EXISTS keyframe (
    id                        uuid   NOT NULL PRIMARY KEY,
    parent                    uuid   NOT NULL,
    pts                       bigint NOT NULL,
    pts_tb_num                bigint NOT NULL,
    pts_tb_den                bigint NOT NULL,
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
CREATE INDEX IF NOT EXISTS idx_keyframe_parent ON keyframe(parent);

-- detection child tables (per-kind, keyed by (keyframe, ordinal))
CREATE TABLE IF NOT EXISTS keyframe_classification (
    keyframe   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    label      text    NOT NULL,
    confidence real    NOT NULL,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_classification_keyframe ON keyframe_classification(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_object (
    keyframe   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    label      text    NOT NULL,
    confidence real    NOT NULL,
    has_bbox   boolean NOT NULL DEFAULT false,
    bbox_x     real,
    bbox_y     real,
    bbox_w     real,
    bbox_h     real,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_object_keyframe ON keyframe_object(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_action (
    keyframe   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    label      text    NOT NULL,
    confidence real    NOT NULL,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_action_keyframe ON keyframe_action(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_text_detection (
    keyframe   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    text       text    NOT NULL,
    confidence real    NOT NULL,
    bbox_x     real    NOT NULL,
    bbox_y     real    NOT NULL,
    bbox_w     real    NOT NULL,
    bbox_h     real    NOT NULL,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_text_detection_keyframe ON keyframe_text_detection(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_barcode (
    keyframe   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    payload    text    NOT NULL,
    symbology  text    NOT NULL,
    confidence real    NOT NULL,
    bbox_x     real    NOT NULL,
    bbox_y     real    NOT NULL,
    bbox_w     real    NOT NULL,
    bbox_h     real    NOT NULL,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_barcode_keyframe ON keyframe_barcode(keyframe);

-- attention / objectness saliency share this table; `kind` discriminates
-- (0 = attention, 1 = objectness).
CREATE TABLE IF NOT EXISTS keyframe_saliency (
    keyframe   uuid     NOT NULL,
    kind       smallint NOT NULL,
    ordinal    integer  NOT NULL,
    bbox_x     real     NOT NULL,
    bbox_y     real     NOT NULL,
    bbox_w     real     NOT NULL,
    bbox_h     real     NOT NULL,
    confidence real     NOT NULL,
    PRIMARY KEY (keyframe, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_saliency_keyframe ON keyframe_saliency(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_document_segment (
    keyframe   uuid    NOT NULL,
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
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_document_segment_keyframe ON keyframe_document_segment(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_color (
    keyframe   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    rgba       bigint  NOT NULL,
    name       text    NOT NULL,
    percentage real    NOT NULL,
    population bigint  NOT NULL,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_color_keyframe ON keyframe_color(keyframe);

-- humans + animals share these shapes; `scope` (0 = human, 1 = animal)
-- discriminates.
CREATE TABLE IF NOT EXISTS keyframe_subject (
    keyframe   uuid     NOT NULL,
    scope      smallint NOT NULL,
    ordinal    integer  NOT NULL,
    label      text     NOT NULL,
    confidence real     NOT NULL,
    bbox_x     real     NOT NULL,
    bbox_y     real     NOT NULL,
    bbox_w     real     NOT NULL,
    bbox_h     real     NOT NULL,
    PRIMARY KEY (keyframe, scope, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_subject_keyframe ON keyframe_subject(keyframe);

-- humans faces + face_rectangles share this shape; `kind`
-- (0 = faces, 1 = face_rectangles) discriminates.
CREATE TABLE IF NOT EXISTS keyframe_face (
    keyframe        uuid     NOT NULL,
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
    PRIMARY KEY (keyframe, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_keyframe ON keyframe_face(keyframe);

-- 2-D body pose: humans + animals share this shape; `scope` discriminates.
CREATE TABLE IF NOT EXISTS keyframe_body_pose (
    keyframe   uuid     NOT NULL,
    scope      smallint NOT NULL,
    ordinal    integer  NOT NULL,
    bbox_x     real     NOT NULL,
    bbox_y     real     NOT NULL,
    bbox_w     real     NOT NULL,
    bbox_h     real     NOT NULL,
    confidence real     NOT NULL,
    PRIMARY KEY (keyframe, scope, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_keyframe ON keyframe_body_pose(keyframe);

-- joints for 2-D body / hand pose rows; `scope`
-- (0 = human-body, 1 = animal-body, 2 = hand) discriminates which parent.
CREATE TABLE IF NOT EXISTS keyframe_body_pose_joint (
    keyframe       uuid     NOT NULL,
    scope          smallint NOT NULL,
    parent_ordinal integer  NOT NULL,
    ordinal        integer  NOT NULL,
    name           text     NOT NULL,
    x              real     NOT NULL,
    y              real     NOT NULL,
    confidence     real     NOT NULL,
    PRIMARY KEY (keyframe, scope, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_joint_keyframe ON keyframe_body_pose_joint(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_hand_pose (
    keyframe   uuid     NOT NULL,
    ordinal    integer  NOT NULL,
    bbox_x     real     NOT NULL,
    bbox_y     real     NOT NULL,
    bbox_w     real     NOT NULL,
    bbox_h     real     NOT NULL,
    confidence real     NOT NULL,
    chirality  smallint NOT NULL,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_hand_pose_keyframe ON keyframe_hand_pose(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d (
    keyframe          uuid     NOT NULL,
    ordinal           integer  NOT NULL,
    confidence        real     NOT NULL,
    body_height       real     NOT NULL,
    height_estimation smallint NOT NULL,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_3d_keyframe ON keyframe_body_pose_3d(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_body_pose_3d_joint (
    keyframe       uuid    NOT NULL,
    parent_ordinal integer NOT NULL,
    ordinal        integer NOT NULL,
    name           text    NOT NULL,
    x              real    NOT NULL,
    y              real    NOT NULL,
    z              real    NOT NULL,
    confidence     real    NOT NULL,
    PRIMARY KEY (keyframe, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_body_pose_3d_joint_keyframe ON keyframe_body_pose_3d_joint(keyframe);

-- instance + whole-frame segmentation masks share this shape; `kind`
-- (0 = instance, 1 = segmentation) discriminates.
CREATE TABLE IF NOT EXISTS keyframe_mask (
    keyframe       uuid     NOT NULL,
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
    PRIMARY KEY (keyframe, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_mask_keyframe ON keyframe_mask(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_face_landmarks (
    keyframe   uuid    NOT NULL,
    ordinal    integer NOT NULL,
    bbox_x     real    NOT NULL,
    bbox_y     real    NOT NULL,
    bbox_w     real    NOT NULL,
    bbox_h     real    NOT NULL,
    confidence real    NOT NULL,
    PRIMARY KEY (keyframe, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmarks_keyframe ON keyframe_face_landmarks(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_region (
    keyframe       uuid    NOT NULL,
    parent_ordinal integer NOT NULL,
    ordinal        integer NOT NULL,
    name           text    NOT NULL,
    PRIMARY KEY (keyframe, parent_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmark_region_keyframe ON keyframe_face_landmark_region(keyframe);

CREATE TABLE IF NOT EXISTS keyframe_face_landmark_point (
    keyframe       uuid    NOT NULL,
    parent_ordinal integer NOT NULL,
    region_ordinal integer NOT NULL,
    ordinal        integer NOT NULL,
    x              real    NOT NULL,
    y              real    NOT NULL,
    PRIMARY KEY (keyframe, parent_ordinal, region_ordinal, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_face_landmark_point_keyframe ON keyframe_face_landmark_point(keyframe);

-- VLM open-vocab labels — `kind` (0 = categories, 1 = tags, 2 = objects,
-- 3 = subjects, 4 = mood, 5 = emotion, 6 = lighting) discriminates the
-- source `Vec<LocalizedText>` slice.
CREATE TABLE IF NOT EXISTS keyframe_vlm_label (
    keyframe   uuid     NOT NULL,
    kind       smallint NOT NULL,
    ordinal    integer  NOT NULL,
    src        text     NOT NULL,
    translated text     NOT NULL,
    PRIMARY KEY (keyframe, kind, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_kf_vlm_label_keyframe ON keyframe_vlm_label(keyframe);

-- Subtitle-cluster: the `Subtitle` facet + `SubtitleTrack` +
-- `SubtitleCue` (+ the `index_errors` child table). Nested value-objects
-- are flattened into real columns; collections ride in child tables with
-- an `ordinal` order column; reverse-FK `Vec<Id>` fields are not stored.

CREATE TABLE IF NOT EXISTS subtitle (
    id                     uuid    NOT NULL PRIMARY KEY,
    parent                 uuid    NOT NULL,
    track_progress_total   bigint  NOT NULL DEFAULT 0,
    track_progress_indexed bigint  NOT NULL DEFAULT 0,
    track_progress_failed  bigint  NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_subtitle_parent ON subtitle(parent);

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
    subtitle_track uuid    NOT NULL,
    ordinal        integer NOT NULL,
    code           integer NOT NULL,
    message        text    NOT NULL,
    PRIMARY KEY (subtitle_track, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_stie_subtitle_track ON subtitle_track_index_error(subtitle_track);

CREATE TABLE IF NOT EXISTS subtitle_cue (
    id                  uuid    NOT NULL PRIMARY KEY,
    parent              uuid    NOT NULL,
    index               bigint  NOT NULL,
    span_start_pts      bigint  NOT NULL,
    span_end_pts        bigint  NOT NULL,
    span_tb_num         bigint  NOT NULL,
    span_tb_den         bigint  NOT NULL,
    text_src            text    NOT NULL,
    text_translated     text    NOT NULL,
    styled_text         text    NOT NULL,
    image               bytea   NOT NULL,
    ocr_text_src        text    NOT NULL,
    ocr_text_translated text    NOT NULL
);
CREATE INDEX        IF NOT EXISTS idx_subtitle_cue_parent       ON subtitle_cue(parent);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subtitle_cue_parent_index ON subtitle_cue(parent, index);
