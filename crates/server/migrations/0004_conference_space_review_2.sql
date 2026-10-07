-- Conference spaces, after the second review: revocations that haven't
-- reached every writer's PDS yet, so the background loop retries them.

-- When a revoked credential's revocation was delivered to every writer's
-- PDS. Revoked but not delivered is the outbox.
ALTER TABLE space_credentials ADD COLUMN revocation_sent_at BIGINT;
