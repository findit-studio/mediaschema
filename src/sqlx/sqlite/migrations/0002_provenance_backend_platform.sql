-- mediaschema — SQLite migration 0002: speaker voiceprint provenance
-- backend + host platform.
--
-- Additive, upgrade-safe follow-up to 0001. Adds the inference backend +
-- host platform that the voiceprint model ran on to the `speaker` table.
-- All four columns are nullable; NULL = not recorded (decodes to
-- Backend::Unspecified / empty Platform), so rows written before this
-- migration remain valid. `0001_init.sql` is left untouched so databases
-- that already applied it pick these columns up here instead.
--
-- NOTE: SQLite `ALTER TABLE ... ADD COLUMN` is not idempotent (no
-- `IF NOT EXISTS`); a migration runner must apply each file exactly once.

ALTER TABLE speaker ADD COLUMN voiceprint_provenance_backend             INTEGER;
ALTER TABLE speaker ADD COLUMN voiceprint_provenance_platform_os         TEXT;
ALTER TABLE speaker ADD COLUMN voiceprint_provenance_platform_arch       TEXT;
ALTER TABLE speaker ADD COLUMN voiceprint_provenance_platform_os_version TEXT;
