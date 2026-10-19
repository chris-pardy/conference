-- Events (groups of people with event details), RSVPs, feeds, cards and the
-- actions dispatched to cards. TEXT and BIGINT only; timestamps in epoch
-- milliseconds.

-- An event. `parent_id` makes it a sub-event: visible only to people going
-- to its parent.
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    starts_at BIGINT NOT NULL,
    ends_at BIGINT,
    time_zone TEXT NOT NULL,
    location TEXT NOT NULL,
    host_did TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX events_by_parent ON events (parent_id);

-- `status` is going, maybe or no. Going opens the event.
CREATE TABLE rsvps (
    event_id TEXT NOT NULL,
    did TEXT NOT NULL,
    status TEXT NOT NULL,
    updated_at BIGINT NOT NULL,
    PRIMARY KEY (event_id, did)
);

CREATE TABLE feeds (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    name TEXT NOT NULL,
    position BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);

-- Card modules, content-addressed: `hash` is the sha-256 of the wasm, and
-- `bytes` the wasm in base64.
CREATE TABLE modules (
    hash TEXT PRIMARY KEY,
    bytes TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

-- A card: its authored card-kit UI and config, its module, and the shared
-- state its server reducer keeps. `event_id` links an event card to the
-- event it shows.
CREATE TABLE cards (
    id TEXT PRIMARY KEY,
    feed_id TEXT NOT NULL,
    author_did TEXT NOT NULL,
    ui TEXT NOT NULL,
    config TEXT NOT NULL,
    state TEXT NOT NULL,
    module_hash TEXT NOT NULL,
    event_id TEXT,
    version BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX cards_by_feed ON cards (feed_id, created_at);
CREATE INDEX cards_by_event ON cards (event_id);

-- Every action the server reduced, as a log.
CREATE TABLE card_actions (
    id TEXT PRIMARY KEY,
    card_id TEXT NOT NULL,
    did TEXT NOT NULL,
    type TEXT NOT NULL,
    payload TEXT NOT NULL,
    meta TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX card_actions_by_card ON card_actions (card_id, created_at);

-- Who people are, as last seen at sign-in, for person blocks.
CREATE TABLE profiles (
    did TEXT PRIMARY KEY,
    handle TEXT NOT NULL,
    display_name TEXT,
    avatar TEXT,
    updated_at BIGINT NOT NULL
);
