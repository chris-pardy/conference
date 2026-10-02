-- Sign-in: pending authorization requests, sessions, and the client's own
-- signing keys. Portable across SQLite and Postgres: TEXT and BIGINT only,
-- timestamps in epoch milliseconds.

CREATE TABLE oauth_requests (
    state TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    dpop_key TEXT NOT NULL,
    issuer TEXT NOT NULL,
    expected_did TEXT,
    preauth_hash TEXT NOT NULL,
    return_to TEXT NOT NULL,
    expires_at BIGINT NOT NULL
);

CREATE TABLE sessions (
    id_hash TEXT PRIMARY KEY,
    did TEXT NOT NULL,
    handle TEXT NOT NULL,
    display_name TEXT,
    avatar TEXT,
    pds TEXT,
    dpop_key TEXT,
    issuer TEXT,
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at BIGINT,
    refresh_lease_until BIGINT,
    scopes TEXT NOT NULL,
    csrf_token TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    last_seen_at BIGINT NOT NULL,
    ended_at BIGINT
);

CREATE INDEX sessions_did ON sessions (did);
CREATE INDEX sessions_renewal ON sessions (ended_at, token_expires_at);

CREATE TABLE client_keys (
    kid TEXT PRIMARY KEY,
    private_jwk TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
