-- Signed decisions (design review round 5). An authority's attestation keys,
-- published in its DID document as `#eventside_attest`, `#eventside_attest_2`
-- and so on, with their private halves encrypted at rest like its other
-- keys. A removed key keeps its row (so its fragment isn't reused), without
-- its private half.
CREATE TABLE attest_keys (
    authority TEXT NOT NULL,
    fragment TEXT NOT NULL,
    private_key TEXT,
    public_key TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    removed_at BIGINT,
    PRIMARY KEY (authority, fragment)
);

-- The signing journal: operational state, not a log of record. Each
-- authority's next `seq` and the last `signedAt` it signed with, and each
-- signing's entry while its record is written: `pending`, then `committed`
-- or `void`. A pending entry blocks another decision about the same person,
-- and a code's pending and committed uses count against its limits. If it's
-- lost, the counter restarts above the highest `seq` found in the records.
CREATE TABLE signing_counters (
    authority TEXT PRIMARY KEY,
    next_seq BIGINT NOT NULL,
    last_signed_at BIGINT NOT NULL
);
CREATE TABLE signing_journal (
    authority TEXT NOT NULL,
    seq BIGINT NOT NULL,
    space TEXT,
    subject TEXT,
    code_hash TEXT,
    state TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (authority, seq)
);
CREATE INDEX signing_journal_subject ON signing_journal (authority, space, subject);
CREATE INDEX signing_journal_code ON signing_journal (authority, code_hash);
