-- Review round 1 of conference-space: conferences recorded as pending until
-- their creation finishes, and the records index the folded-in space-sync
-- keeps. Portable across SQLite and Postgres: TEXT and BIGINT only,
-- timestamps in epoch milliseconds.

-- `pending` until `conference create` has made the space, the event and the
-- first owner; a rerun finishes it. Only `ready` conferences are served.
ALTER TABLE conferences ADD COLUMN status TEXT NOT NULL DEFAULT 'ready';

-- Pruning refused join attempts outside the window.
CREATE INDEX join_attempts_at ON join_attempts (at);

-- The index: the current version of every record in a conference space,
-- by repo, with the revision its PDS gave the commit that wrote it.
-- `value` is NULL for a deleted record, so an older read can't bring it
-- back. `counted` is what the ingest hook decided when it was written: the
-- author was a member at that revision (or is the authority, with a valid
-- signature where one is needed). Serving also needs the author to be a
-- member now.
CREATE TABLE space_records (
    space TEXT NOT NULL,
    repo TEXT NOT NULL,
    collection TEXT NOT NULL,
    rkey TEXT NOT NULL,
    rev TEXT NOT NULL,
    value TEXT,
    counted BIGINT NOT NULL,
    indexed_at BIGINT NOT NULL,
    PRIMARY KEY (space, repo, collection, rkey)
);
CREATE INDEX space_records_collection ON space_records (space, collection);

-- How far each writer's repo in a space has been read.
CREATE TABLE space_repos (
    space TEXT NOT NULL,
    repo TEXT NOT NULL,
    synced_rev TEXT NOT NULL,
    synced_at BIGINT NOT NULL,
    PRIMARY KEY (space, repo)
);

-- When eventside last registered for a space's write notifications, and
-- last backfilled it.
CREATE TABLE space_sync (
    space TEXT PRIMARY KEY,
    registered_at BIGINT NOT NULL,
    synced_at BIGINT NOT NULL
);
