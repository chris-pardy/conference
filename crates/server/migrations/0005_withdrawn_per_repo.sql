-- Review round 19: a withdrawn decision is withdrawn in the repo (and
-- space) it was withdrawn from, by the `seq` its verifying signature
-- carries, so a record signed elsewhere (with a leaked key, say) can't
-- withdraw someone else's decision.
DROP TABLE withdrawn_seqs;
CREATE TABLE withdrawn_seqs (
    authority TEXT NOT NULL,
    space TEXT NOT NULL,
    repo TEXT NOT NULL,
    seq BIGINT NOT NULL,
    withdrawn_at BIGINT NOT NULL,
    PRIMARY KEY (authority, space, repo, seq)
);
