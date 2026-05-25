# `SbvData` — YouTube SBV per-cue payload  *(rev 1)*

## Domain meaning

The `D` payload for `SubtitleCue<Id, SbvData>` (= [`SbvCue`]). YouTube
SBV is a plain-text-only format with no per-format inline markup or
styling; `SbvData` is a unit marker (no payload columns). The cue text
rides on the base `SubtitleCue.text`. The detail table exists per the
established polymorphic-cue pattern (uniform dispatch surface across
all formats).

## Cross-cutting (locked)

No per-track aggregate.

## Fields

None — `SbvData` is the unit `()` type at the wire / SQL boundary.

## Invariants

None beyond the base cue invariants.

## Projection notes

- **sqlx**: `subtitle_cue_sbv` detail table with the FK PK only (no
  payload columns).
- **mongodb**: only the base fields persist; the on-document `kind`
  discriminator carries the format identity.
- **wire**: an empty `SbvData` message inside the `SubtitleCue.data` oneof.

**Status: rev 1 — implemented in PR #86 (closes #56).**
