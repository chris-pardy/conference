-- Conference spaces, after review: when records were first seen, a counter
-- that says when an organization's index changed, and who registered for a
-- space's write notifications.

-- When our host first saw each version of a record. A join's place in time
-- is its commit's revision, but never earlier than this, so a writer's PDS
-- can't backdate a join past a code's expiry or ahead of others. Kept state:
-- reindex doesn't clear it, so a rebuilt index judges joins the same way.
CREATE TABLE space_record_seen (
    space TEXT NOT NULL,
    repo TEXT NOT NULL,
    collection TEXT NOT NULL,
    rkey TEXT NOT NULL,
    rev TEXT NOT NULL,
    seen_at BIGINT NOT NULL,
    PRIMARY KEY (space, repo, collection, rkey, rev)
);

-- Bumped whenever an organization's index changes, by the server or the
-- admin CLI, so a derived view of it can be reused until then.
CREATE TABLE index_generations (
    org TEXT PRIMARY KEY,
    generation BIGINT NOT NULL
);

-- Who registered a syncer, so revoking their access drops the registration.
ALTER TABLE space_notify ADD COLUMN delegator TEXT;
ALTER TABLE space_notify ADD COLUMN client_id TEXT;
