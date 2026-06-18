-- Public tag code for the QR/scan resolver path.
--
-- A machine's QR encodes this opaque, non-enumerable code (not its UUID, which is
-- time-sortable and guessable). It is rotatable: issuing a new code revokes the old
-- tag without touching the machine's identity. NULL = no active tag.
ALTER TABLE machines
  ADD COLUMN public_code CHAR(16) NULL UNIQUE AFTER asset_tag;

-- Backfill existing rows with a code derived from the id's hex (uppercase, 16 chars).
-- New rows get a random Crockford-base32 code generated in Rust at create time.
UPDATE machines
   SET public_code = UPPER(SUBSTRING(REPLACE(id, '-', ''), 1, 16))
 WHERE public_code IS NULL;
