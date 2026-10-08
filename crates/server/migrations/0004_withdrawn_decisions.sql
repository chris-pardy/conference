-- Review rounds 17 and 18 of the signed decisions.
--
-- A signed decision withdrawn by deleting its record (or replaced by a
-- version with another `seq`) stays withdrawn: putting the same signed bytes
-- back doesn't revive it, since later decisions were checked without it.
-- Operational state like the signing journal, kept across reindexes.
CREATE TABLE withdrawn_seqs (
    authority TEXT NOT NULL,
    seq BIGINT NOT NULL,
    withdrawn_at BIGINT NOT NULL,
    PRIMARY KEY (authority, seq)
);

-- Where a signing's record is written (the space and the repo), so a
-- pending entry whose read-back failed can be read again before it's
-- settled.
ALTER TABLE signing_journal ADD COLUMN written_in TEXT;
ALTER TABLE signing_journal ADD COLUMN written_by TEXT;

-- `signedAt` is the clock at signing, no longer carried forward.
ALTER TABLE signing_counters DROP COLUMN last_signed_at;
