-- Renewal's two queries, each paged by its own column: idle sessions, and
-- tokens due for a refresh.
DROP INDEX sessions_renewal;
CREATE INDEX sessions_idle ON sessions (ended_at, last_seen_at, id_hash);
CREATE INDEX sessions_renewal ON sessions (ended_at, token_expires_at, id_hash);
