-- Conference spaces: our server as the space host, admin sessions, and the
-- index of the records permissions are built from. Portable across SQLite
-- and Postgres: TEXT and BIGINT only, timestamps in epoch milliseconds.

-- Sessions are an attendee's (behind a cookie) or an admin's (cookieless,
-- used by the admin CLI, never idle). Pending requests already have a `kind`
-- (login, signup, admin or email); `context` is what the callback finishes
-- with: the admin connection, or the conference an email step is for.
ALTER TABLE sessions ADD COLUMN kind TEXT NOT NULL DEFAULT 'attendee';
ALTER TABLE oauth_requests ADD COLUMN context TEXT;

-- Secrets the server generates when none is configured (the key-encryption
-- and HMAC secret), kept like the OAuth signing key.
CREATE TABLE server_secrets (
    name TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

-- What we must keep for each organization: its space authority's keys
-- (encrypted at rest) and its super admin. Everything else is rebuilt by
-- crawling from the super admin.
CREATE TABLE authorities (
    did TEXT PRIMARY KEY,
    super_admin TEXT NOT NULL,
    name TEXT,
    rotation_key TEXT NOT NULL,
    space_key TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

-- An admin connecting from the CLI: the browser finishes it, the CLI waits.
CREATE TABLE admin_connects (
    id TEXT PRIMARY KEY,
    did TEXT NOT NULL,
    handle TEXT NOT NULL,
    scopes TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    completed_at BIGINT,
    error TEXT
);

-- The index: every record we've read from a repo in one of our spaces, with
-- the revision of the commit that last wrote it. A deleted record keeps its
-- row, with no value, so an older read can't bring it back. A code record's
-- HMAC (`codeHash`, admin spaces only) is kept on its row, so a join by code
-- alone looks up the organization it belongs to.
CREATE TABLE space_records (
    space TEXT NOT NULL,
    repo TEXT NOT NULL,
    collection TEXT NOT NULL,
    rkey TEXT NOT NULL,
    rev TEXT NOT NULL,
    cid TEXT,
    value TEXT,
    code_hmac TEXT,
    PRIMARY KEY (space, repo, collection, rkey)
);
CREATE INDEX space_records_collection ON space_records (space, collection);
CREATE INDEX space_records_code_hmac ON space_records (code_hmac);

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

-- How far each repo in a space has been read.
CREATE TABLE space_repos (
    space TEXT NOT NULL,
    repo TEXT NOT NULL,
    synced_rev TEXT NOT NULL,
    PRIMARY KEY (space, repo)
);

-- The host's writer set (listRepos), from accepted write notifications.
CREATE TABLE space_writers (
    space TEXT NOT NULL,
    did TEXT NOT NULL,
    repo_rev TEXT NOT NULL,
    hash TEXT NOT NULL,
    space_rev TEXT NOT NULL,
    first_seen_at BIGINT NOT NULL,
    PRIMARY KEY (space, did)
);
CREATE INDEX space_writers_rev ON space_writers (space, space_rev);

-- Syncers registered for a space's write notifications, with who registered
-- them, so revoking their access drops the registration.
CREATE TABLE space_notify (
    space TEXT NOT NULL,
    service TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    delegator TEXT,
    client_id TEXT,
    PRIMARY KEY (space, service)
);

-- Credentials issued, so removing someone revokes the ones they delegated.
-- A revoked one's revocation is sent to every writer's PDS; revoked but not
-- yet delivered (`revocation_sent_at`) is the outbox the background loop
-- retries.
CREATE TABLE space_credentials (
    jti TEXT PRIMARY KEY,
    space TEXT NOT NULL,
    delegator TEXT NOT NULL,
    client_id TEXT,
    expires_at BIGINT NOT NULL,
    revoked_at BIGINT,
    revocation_sent_at BIGINT
);
CREATE INDEX space_credentials_delegator ON space_credentials (space, delegator);

-- A conference's public facts (name, dates, place, theme), cached from its
-- event and settings records so its page doesn't depend on the super
-- admin's PDS being up. Rebuilt by reindex.
CREATE TABLE conferences (
    space TEXT PRIMARY KEY,
    org TEXT NOT NULL,
    intake TEXT NOT NULL,
    super_admin TEXT NOT NULL,
    event TEXT,
    invite_only BIGINT NOT NULL,
    info TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX conferences_event ON conferences (event);
CREATE INDEX conferences_org ON conferences (org);
