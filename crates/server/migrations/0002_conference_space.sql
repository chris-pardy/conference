-- Conferences (features/conference-space.md, design rounds 1 and 2): each
-- conference is a space on its organization's own PDS, with eventside as
-- its managing app. Eventside's database decides who's in: the decisions
-- log, read in `seq` order, is authoritative, and the signed records in the
-- organization's repo are a copy of it, written through the outbox.
-- Portable across SQLite and Postgres: TEXT and BIGINT only, timestamps in
-- epoch milliseconds.

-- Sessions are an attendee's (behind a cookie), an admin's or an
-- organization's (cookieless, made by the admin CLI). A pending request's
-- `context` is the `admin_connects` row it finishes.
ALTER TABLE sessions ADD COLUMN kind TEXT NOT NULL DEFAULT 'attendee';
ALTER TABLE oauth_requests ADD COLUMN context TEXT;

-- An admin, or an organization's account, connecting from the CLI: the
-- browser finishes it, the CLI waits. `kind` is `admin` or `org`.
CREATE TABLE admin_connects (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    did TEXT NOT NULL,
    handle TEXT NOT NULL,
    scopes TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    completed_at BIGINT,
    error TEXT
);

-- Eventside's `#eventside_attest*` keys, which sign membership records. Its
-- DID document lists every one; the newest signs. Keys are only added.
CREATE TABLE attest_keys (
    fragment TEXT PRIMARY KEY,
    private_jwk TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

-- A conference: its space on the organization's PDS, and its public event.
-- `methods` is a comma-separated list of join methods (code, list, open);
-- `theme` is JSON.
CREATE TABLE conferences (
    space TEXT PRIMARY KEY,
    org TEXT NOT NULL,
    rkey TEXT NOT NULL,
    event TEXT NOT NULL,
    name TEXT NOT NULL,
    starts_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    city TEXT NOT NULL,
    description TEXT,
    theme TEXT NOT NULL,
    methods TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX conferences_event ON conferences (org, rkey);

-- Every decision about a person, in order. `action` is admit, remove,
-- leave, ban, unban or role; `rank` (owner, staff or self) is the actor's
-- weight when it was made, and never changes.
CREATE TABLE decisions (
    seq BIGINT PRIMARY KEY,
    conference TEXT NOT NULL,
    subject TEXT NOT NULL,
    action TEXT NOT NULL,
    role TEXT,
    method TEXT,
    actor TEXT NOT NULL,
    rank TEXT NOT NULL,
    decided_at BIGINT NOT NULL
);
CREATE INDEX decisions_subject ON decisions (conference, subject, seq);

-- What a decision (or a conference's creation, or a change of its apps)
-- still has to write into the organization's repo. Applied in order, by
-- whichever process holds the lease; the server replays what's left on start.
CREATE TABLE outbox (
    seq BIGINT PRIMARY KEY,
    conference TEXT NOT NULL,
    kind TEXT NOT NULL,
    subject TEXT,
    created_at BIGINT NOT NULL,
    applied_at BIGINT,
    attempts BIGINT NOT NULL,
    retry_at BIGINT,
    last_error TEXT
);
CREATE INDEX outbox_pending ON outbox (applied_at, seq);

-- Who's applying the outbox: one process at a time.
CREATE TABLE outbox_lease (
    slot BIGINT PRIMARY KEY CHECK (slot = 1),
    holder TEXT,
    until_ms BIGINT NOT NULL
);
INSERT INTO outbox_lease (slot, holder, until_ms) VALUES (1, NULL, 0);

-- Shared invite codes, by their SHA-256.
CREATE TABLE codes (
    conference TEXT NOT NULL,
    code_hash TEXT NOT NULL,
    expires_at BIGINT,
    max_uses BIGINT,
    uses BIGINT NOT NULL,
    created_by TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (conference, code_hash)
);

-- The attendee list, by the DID each handle named when it was imported.
CREATE TABLE attendee_list (
    conference TEXT NOT NULL,
    did TEXT NOT NULL,
    handle TEXT NOT NULL,
    imported_at BIGINT NOT NULL,
    PRIMARY KEY (conference, did)
);

-- Refused join attempts, for slowing down guessing.
CREATE TABLE join_attempts (
    conference TEXT NOT NULL,
    did TEXT NOT NULL,
    at BIGINT NOT NULL
);
CREATE INDEX join_attempts_by ON join_attempts (conference, did, at);

-- The other apps a conference allows, and for what (`read`; outside card
-- providers and feed generators later). Eventside is always allowed.
CREATE TABLE conference_apps (
    conference TEXT NOT NULL,
    client_id TEXT NOT NULL,
    uses TEXT NOT NULL,
    PRIMARY KEY (conference, client_id)
);
