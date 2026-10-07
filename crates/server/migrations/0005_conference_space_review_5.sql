-- Conference spaces, after the fifth review: which conference a code is
-- for, found without deriving every organization's index.

-- A code record's HMAC (`codeHash`), on its row in the index, so a join by
-- code alone looks up the organization it belongs to. Set whenever a record
-- is written into the index; `reindex` fills it in for an index made before.
ALTER TABLE space_records ADD COLUMN code_hmac TEXT;
CREATE INDEX space_records_code_hmac ON space_records (code_hmac);
