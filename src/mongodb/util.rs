//! Shared bson encode/decode helpers for the primitive + VO types every
//! aggregate maps over.
//!
//! Every helper here is `pub(super)` — these are implementation glue,
//! not part of the public surface.

use ::bson::{spec::BinarySubtype, Binary, Bson, DateTime as BsonDateTime, Document};
use core::num::NonZeroU32;
use jiff::Timestamp as JiffTimestamp;
use mediatime::{TimeRange, Timebase, Timestamp as MediaTimestamp};
use smol_str::SmolStr;

use crate::domain::{
  primitives::ErrorCode,
  vo::{LocalizedText, Provenance},
  ErrorInfo, FileChecksum, Location, Rgba, Uuid7,
};

use super::error::MongoError;

// ---------------------------------------------------------------------------
// Field-extraction helpers (Document → Bson → T)
// ---------------------------------------------------------------------------

/// Pull a required field from a `Document`, returning `MissingField` if
/// absent. `Null` is *not* treated as absent — the caller decides.
pub(super) fn take(doc: &mut Document, field: &'static str) -> Result<Bson, MongoError> {
  doc.remove(field).ok_or_else(|| MongoError::missing(field))
}

/// Pull an optional field. Missing **or** explicit `Null` ⇒ `None`.
pub(super) fn take_opt(doc: &mut Document, field: &'static str) -> Option<Bson> {
  match doc.remove(field) {
    Some(Bson::Null) | None => None,
    Some(v) => Some(v),
  }
}

fn bson_kind(b: &Bson) -> &'static str {
  match b {
    Bson::Double(_) => "Double",
    Bson::String(_) => "String",
    Bson::Array(_) => "Array",
    Bson::Document(_) => "Document",
    Bson::Boolean(_) => "Boolean",
    Bson::Null => "Null",
    Bson::RegularExpression(_) => "RegularExpression",
    Bson::JavaScriptCode(_) => "JavaScriptCode",
    Bson::JavaScriptCodeWithScope(_) => "JavaScriptCodeWithScope",
    Bson::Int32(_) => "Int32",
    Bson::Int64(_) => "Int64",
    Bson::Timestamp(_) => "Timestamp",
    Bson::Binary(_) => "Binary",
    Bson::ObjectId(_) => "ObjectId",
    Bson::DateTime(_) => "DateTime",
    Bson::Symbol(_) => "Symbol",
    Bson::Decimal128(_) => "Decimal128",
    Bson::Undefined => "Undefined",
    Bson::MaxKey => "MaxKey",
    Bson::MinKey => "MinKey",
    Bson::DbPointer(_) => "DbPointer",
  }
}

// ---------------------------------------------------------------------------
// Primitive decoders (Bson → T)
// ---------------------------------------------------------------------------

pub(super) fn as_str(b: Bson, field: &'static str) -> Result<String, MongoError> {
  match b {
    Bson::String(s) => Ok(s),
    other => Err(MongoError::type_mismatch(
      field,
      "String",
      bson_kind(&other),
    )),
  }
}

pub(super) fn as_smol(b: Bson, field: &'static str) -> Result<SmolStr, MongoError> {
  as_str(b, field).map(SmolStr::from)
}

pub(super) fn as_bool(b: Bson, field: &'static str) -> Result<bool, MongoError> {
  match b {
    Bson::Boolean(v) => Ok(v),
    other => Err(MongoError::type_mismatch(
      field,
      "Boolean",
      bson_kind(&other),
    )),
  }
}

pub(super) fn as_i64(b: Bson, field: &'static str) -> Result<i64, MongoError> {
  match b {
    Bson::Int32(v) => Ok(v as i64),
    Bson::Int64(v) => Ok(v),
    other => Err(MongoError::type_mismatch(
      field,
      "Int32/Int64",
      bson_kind(&other),
    )),
  }
}

pub(super) fn as_f64(b: Bson, field: &'static str) -> Result<f64, MongoError> {
  match b {
    Bson::Double(v) => Ok(v),
    Bson::Int32(v) => Ok(v as f64),
    Bson::Int64(v) => Ok(v as f64),
    other => Err(MongoError::type_mismatch(
      field,
      "Double",
      bson_kind(&other),
    )),
  }
}

pub(super) fn as_f32(b: Bson, field: &'static str) -> Result<f32, MongoError> {
  as_f64(b, field).map(|v| v as f32)
}

pub(super) fn as_u8(b: Bson, field: &'static str) -> Result<u8, MongoError> {
  let v = as_i64(b, field)?;
  u8::try_from(v).map_err(|_| MongoError::IntOutOfRange {
    field: SmolStr::from(field),
    value: v,
  })
}

pub(super) fn as_u16(b: Bson, field: &'static str) -> Result<u16, MongoError> {
  let v = as_i64(b, field)?;
  u16::try_from(v).map_err(|_| MongoError::IntOutOfRange {
    field: SmolStr::from(field),
    value: v,
  })
}

pub(super) fn as_u32(b: Bson, field: &'static str) -> Result<u32, MongoError> {
  let v = as_i64(b, field)?;
  u32::try_from(v).map_err(|_| MongoError::IntOutOfRange {
    field: SmolStr::from(field),
    value: v,
  })
}

pub(super) fn as_u64(b: Bson, field: &'static str) -> Result<u64, MongoError> {
  let v = as_i64(b, field)?;
  u64::try_from(v).map_err(|_| MongoError::IntOutOfRange {
    field: SmolStr::from(field),
    value: v,
  })
}

pub(super) fn as_doc(b: Bson, field: &'static str) -> Result<Document, MongoError> {
  match b {
    Bson::Document(d) => Ok(d),
    other => Err(MongoError::type_mismatch(
      field,
      "Document",
      bson_kind(&other),
    )),
  }
}

pub(super) fn as_array(b: Bson, field: &'static str) -> Result<Vec<Bson>, MongoError> {
  match b {
    Bson::Array(v) => Ok(v),
    other => Err(MongoError::type_mismatch(field, "Array", bson_kind(&other))),
  }
}

pub(super) fn as_binary(b: Bson, field: &'static str) -> Result<Vec<u8>, MongoError> {
  match b {
    Bson::Binary(Binary { bytes, .. }) => Ok(bytes),
    other => Err(MongoError::type_mismatch(
      field,
      "Binary",
      bson_kind(&other),
    )),
  }
}

// ---------------------------------------------------------------------------
// `Uuid7` ↔ Binary subtype 4
// ---------------------------------------------------------------------------

pub(super) fn uuid7_to_bson(u: Uuid7) -> Bson {
  Bson::Binary(Binary {
    subtype: BinarySubtype::Uuid,
    bytes: u.as_bytes().to_vec(),
  })
}

pub(super) fn uuid7_from_bson(b: Bson, field: &'static str) -> Result<Uuid7, MongoError> {
  let bytes = as_binary(b, field)?;
  let arr: [u8; 16] = bytes
    .as_slice()
    .try_into()
    .map_err(|_| MongoError::WrongBinaryLen {
      field: SmolStr::from(field),
      want: 16,
      got: bytes.len(),
    })?;
  Ok(Uuid7::try_from_bytes(arr)?)
}

// ---------------------------------------------------------------------------
// `FileChecksum` ↔ Binary subtype 0 (32 bytes)
// ---------------------------------------------------------------------------

pub(super) fn checksum_to_bson(c: &FileChecksum) -> Bson {
  Bson::Binary(Binary {
    subtype: BinarySubtype::Generic,
    bytes: c.as_bytes().to_vec(),
  })
}

pub(super) fn checksum_from_bson(b: Bson, field: &'static str) -> Result<FileChecksum, MongoError> {
  let bytes = as_binary(b, field)?;
  let arr: [u8; 32] = bytes
    .as_slice()
    .try_into()
    .map_err(|_| MongoError::WrongBinaryLen {
      field: SmolStr::from(field),
      want: 32,
      got: bytes.len(),
    })?;
  Ok(FileChecksum::from_bytes(arr))
}

// ---------------------------------------------------------------------------
// Inline `Vec<u8>` ↔ Binary subtype 0
// ---------------------------------------------------------------------------

pub(super) fn bytes_to_bson(v: &[u8]) -> Bson {
  Bson::Binary(Binary {
    subtype: BinarySubtype::Generic,
    bytes: v.to_vec(),
  })
}

// ---------------------------------------------------------------------------
// `jiff::Timestamp` ↔ bson DateTime (nanos round-trip)
// ---------------------------------------------------------------------------

pub(super) fn jiff_to_bson(t: JiffTimestamp) -> Bson {
  // bson DateTime is milliseconds-since-epoch (i64). jiff Timestamp has
  // nanosecond resolution; we down-cast to ms for storage (matches every
  // other mongodb-style backend in the ecosystem) and reconstruct via
  // the same ms.
  let ms = t.as_millisecond();
  Bson::DateTime(BsonDateTime::from_millis(ms))
}

pub(super) fn jiff_from_bson(b: Bson, field: &'static str) -> Result<JiffTimestamp, MongoError> {
  match b {
    Bson::DateTime(dt) => {
      let ms = dt.timestamp_millis();
      JiffTimestamp::from_millisecond(ms).map_err(|_| MongoError::IntOutOfRange {
        field: SmolStr::from(field),
        value: ms,
      })
    }
    other => Err(MongoError::type_mismatch(
      field,
      "DateTime",
      bson_kind(&other),
    )),
  }
}

// ---------------------------------------------------------------------------
// `mediatime::Timebase` ↔ `{ num, den }`
// ---------------------------------------------------------------------------

fn timebase_to_doc(tb: Timebase) -> Document {
  let mut d = Document::new();
  d.insert("num", Bson::Int64(tb.num() as i64));
  d.insert("den", Bson::Int64(tb.den().get() as i64));
  d
}

fn timebase_from_doc(mut d: Document) -> Result<Timebase, MongoError> {
  let num = as_u32(take(&mut d, "num")?, "num")?;
  let den_v = as_u32(take(&mut d, "den")?, "den")?;
  let den = NonZeroU32::new(den_v).ok_or_else(|| MongoError::IntOutOfRange {
    field: SmolStr::from("timebase.den"),
    value: 0,
  })?;
  Ok(Timebase::new(num, den))
}

// ---------------------------------------------------------------------------
// `mediatime::Timestamp` ↔ `{ pts, timebase }`
// ---------------------------------------------------------------------------

pub(super) fn media_ts_to_bson(t: MediaTimestamp) -> Bson {
  let mut d = Document::new();
  d.insert("pts", Bson::Int64(t.pts()));
  d.insert("timebase", Bson::Document(timebase_to_doc(t.timebase())));
  Bson::Document(d)
}

pub(super) fn media_ts_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<MediaTimestamp, MongoError> {
  let mut d = as_doc(b, field)?;
  let pts = as_i64(take(&mut d, "pts")?, "pts")?;
  let tb = timebase_from_doc(as_doc(take(&mut d, "timebase")?, "timebase")?)?;
  Ok(MediaTimestamp::new(pts, tb))
}

// ---------------------------------------------------------------------------
// `mediatime::TimeRange` ↔ `{ start, end, timebase }`
// ---------------------------------------------------------------------------

pub(super) fn time_range_to_bson(r: &TimeRange) -> Bson {
  let mut d = Document::new();
  d.insert("start", Bson::Int64(r.start_pts()));
  d.insert("end", Bson::Int64(r.end_pts()));
  d.insert("timebase", Bson::Document(timebase_to_doc(r.timebase())));
  Bson::Document(d)
}

pub(super) fn time_range_from_bson(b: Bson, field: &'static str) -> Result<TimeRange, MongoError> {
  let mut d = as_doc(b, field)?;
  let start = as_i64(take(&mut d, "start")?, "start")?;
  let end = as_i64(take(&mut d, "end")?, "end")?;
  let tb = timebase_from_doc(as_doc(take(&mut d, "timebase")?, "timebase")?)?;
  TimeRange::try_new(start, end, tb).ok_or_else(|| MongoError::IntOutOfRange {
    field: SmolStr::from(field),
    value: start,
  })
}

// ---------------------------------------------------------------------------
// `Rgba` ↔ `{ r, g, b, a }`
// ---------------------------------------------------------------------------

pub(super) fn rgba_to_bson(c: Rgba) -> Bson {
  let mut d = Document::new();
  d.insert("r", Bson::Int32(c.r() as i32));
  d.insert("g", Bson::Int32(c.g() as i32));
  d.insert("b", Bson::Int32(c.b() as i32));
  d.insert("a", Bson::Int32(c.a() as i32));
  Bson::Document(d)
}

pub(super) fn rgba_from_bson(b: Bson, field: &'static str) -> Result<Rgba, MongoError> {
  let mut d = as_doc(b, field)?;
  let r = as_u8(take(&mut d, "r")?, "r")?;
  let g = as_u8(take(&mut d, "g")?, "g")?;
  let b_v = as_u8(take(&mut d, "b")?, "b")?;
  let a = as_u8(take(&mut d, "a")?, "a")?;
  Ok(Rgba::from_components(r, g, b_v, a))
}

// ---------------------------------------------------------------------------
// `mediaframe::lang::Language` ↔ BCP-47 `String`
// ---------------------------------------------------------------------------

pub(super) fn language_to_bson(l: &::mediaframe::lang::Language) -> Bson {
  Bson::String(l.to_bcp47())
}

pub(super) fn language_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<::mediaframe::lang::Language, MongoError> {
  let s = as_str(b, field)?;
  Ok(::mediaframe::lang::Language::from_bcp47(&s)?)
}

// ---------------------------------------------------------------------------
// `mediaframe::frame::Dimensions` ↔ `{ w, h }`
// ---------------------------------------------------------------------------

pub(super) fn dimensions_to_bson(d: ::mediaframe::frame::Dimensions) -> Bson {
  let mut doc = Document::new();
  doc.insert("w", Bson::Int64(d.width() as i64));
  doc.insert("h", Bson::Int64(d.height() as i64));
  Bson::Document(doc)
}

pub(super) fn dimensions_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<::mediaframe::frame::Dimensions, MongoError> {
  let mut d = as_doc(b, field)?;
  let w = as_u32(take(&mut d, "w")?, "w")?;
  let h = as_u32(take(&mut d, "h")?, "h")?;
  Ok(::mediaframe::frame::Dimensions::new(w, h))
}

// ---------------------------------------------------------------------------
// `Provenance` ↔ document with 4 string fields
// ---------------------------------------------------------------------------

pub(super) fn provenance_to_bson(p: &Provenance) -> Bson {
  let mut d = Document::new();
  d.insert("model_name", Bson::String(p.model_name().to_owned()));
  d.insert("model_version", Bson::String(p.model_version().to_owned()));
  d.insert(
    "prompt_version",
    Bson::String(p.prompt_version().to_owned()),
  );
  d.insert(
    "indexer_version",
    Bson::String(p.indexer_version().to_owned()),
  );
  Bson::Document(d)
}

pub(super) fn provenance_from_bson(b: Bson, field: &'static str) -> Result<Provenance, MongoError> {
  let mut d = as_doc(b, field)?;
  let model_name = as_smol(take(&mut d, "model_name")?, "model_name")?;
  let model_version = as_smol(take(&mut d, "model_version")?, "model_version")?;
  let prompt_version = as_smol(take(&mut d, "prompt_version")?, "prompt_version")?;
  let indexer_version = as_smol(take(&mut d, "indexer_version")?, "indexer_version")?;
  Ok(Provenance::from_parts(
    model_name,
    model_version,
    prompt_version,
    indexer_version,
  ))
}

// ---------------------------------------------------------------------------
// `LocalizedText` ↔ `{ src, translated }`
// ---------------------------------------------------------------------------

pub(super) fn loc_text_to_bson(t: &LocalizedText) -> Bson {
  let mut d = Document::new();
  d.insert("src", Bson::String(t.src().to_owned()));
  d.insert("translated", Bson::String(t.translated().to_owned()));
  Bson::Document(d)
}

pub(super) fn loc_text_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<LocalizedText, MongoError> {
  let mut d = as_doc(b, field)?;
  let src = as_smol(take(&mut d, "src")?, "src")?;
  let translated = as_smol(take(&mut d, "translated")?, "translated")?;
  Ok(LocalizedText::from_src_translated(src, translated))
}

pub(super) fn loc_text_vec_to_bson(v: &[LocalizedText]) -> Bson {
  Bson::Array(v.iter().map(loc_text_to_bson).collect())
}

pub(super) fn loc_text_vec_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<Vec<LocalizedText>, MongoError> {
  as_array(b, field)?
    .into_iter()
    .map(|v| loc_text_from_bson(v, field))
    .collect()
}

// ---------------------------------------------------------------------------
// `ErrorInfo` ↔ `{ code: i64, message: String }`
// ---------------------------------------------------------------------------

pub(super) fn error_info_to_bson(e: &ErrorInfo) -> Bson {
  let mut d = Document::new();
  d.insert("code", Bson::Int64(e.code().as_u32() as i64));
  d.insert("message", Bson::String(e.message().to_owned()));
  Bson::Document(d)
}

pub(super) fn error_info_from_bson(b: Bson, field: &'static str) -> Result<ErrorInfo, MongoError> {
  let mut d = as_doc(b, field)?;
  let code = as_u32(take(&mut d, "code")?, "code")?;
  let message = as_smol(take(&mut d, "message")?, "message")?;
  Ok(ErrorInfo::new(ErrorCode::from_u32(code), message))
}

pub(super) fn error_info_vec_to_bson(v: &[ErrorInfo]) -> Bson {
  Bson::Array(v.iter().map(error_info_to_bson).collect())
}

pub(super) fn error_info_vec_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<Vec<ErrorInfo>, MongoError> {
  as_array(b, field)?
    .into_iter()
    .map(|v| error_info_from_bson(v, field))
    .collect()
}

// ---------------------------------------------------------------------------
// `Location<Uuid7>` ↔ `{ kind: "local", volume: Binary, components: [String] }`
// ---------------------------------------------------------------------------

pub(super) fn location_to_bson(loc: &Location<Uuid7>) -> Bson {
  let local = loc.unwrap_local_ref();
  let mut d = Document::new();
  d.insert("kind", Bson::String("local".to_owned()));
  d.insert("volume", uuid7_to_bson(*local.volume_ref()));
  d.insert(
    "components",
    Bson::Array(
      local
        .components_slice()
        .iter()
        .map(|s| Bson::String(s.as_str().to_owned()))
        .collect(),
    ),
  );
  Bson::Document(d)
}

pub(super) fn location_from_bson(
  b: Bson,
  field: &'static str,
) -> Result<Location<Uuid7>, MongoError> {
  let mut d = as_doc(b, field)?;
  let kind = as_str(take(&mut d, "kind")?, "kind")?;
  if kind != "local" {
    return Err(MongoError::type_mismatch(field, "Location::Local", "other"));
  }
  let volume = uuid7_from_bson(take(&mut d, "volume")?, "volume")?;
  let components = as_array(take(&mut d, "components")?, "components")?
    .into_iter()
    .map(|v| as_smol(v, "components[]"))
    .collect::<Result<Vec<_>, _>>()?;
  Ok(Location::try_local_uuid7(volume, components)?)
}

// ---------------------------------------------------------------------------
// Vec<Id=Uuid7> ↔ Array of Binary
// ---------------------------------------------------------------------------

pub(super) fn uuid7_vec_to_bson(v: &[Uuid7]) -> Bson {
  Bson::Array(v.iter().copied().map(uuid7_to_bson).collect())
}

pub(super) fn uuid7_vec_from_bson(b: Bson, field: &'static str) -> Result<Vec<Uuid7>, MongoError> {
  as_array(b, field)?
    .into_iter()
    .map(|v| uuid7_from_bson(v, field))
    .collect()
}

// ---------------------------------------------------------------------------
// Optional helpers — pull-an-Option from an already-extracted Bson.
// ---------------------------------------------------------------------------

pub(super) fn opt<T, F>(b: Option<Bson>, f: F) -> Result<Option<T>, MongoError>
where
  F: FnOnce(Bson) -> Result<T, MongoError>,
{
  match b {
    None | Some(Bson::Null) => Ok(None),
    Some(v) => f(v).map(Some),
  }
}
