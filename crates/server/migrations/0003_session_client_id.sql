-- The client ID a session's grant was issued to. It changes with the scope
-- list, and a grant can't be refreshed or revoked as another client. NULL
-- for sessions from before this column, which are taken to be the current
-- client's.
ALTER TABLE sessions ADD COLUMN client_id TEXT;
