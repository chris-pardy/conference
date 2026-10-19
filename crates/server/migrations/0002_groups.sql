-- Groups of people, their feeds, the cards in them, and the actions people
-- take on cards. TEXT and BIGINT only; timestamps in epoch milliseconds.

CREATE TABLE groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    invite TEXT NOT NULL UNIQUE,
    owner_did TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE group_members (
    group_id TEXT NOT NULL,
    did TEXT NOT NULL,
    role TEXT NOT NULL,
    joined_at BIGINT NOT NULL,
    PRIMARY KEY (group_id, did)
);

CREATE TABLE feeds (
    id TEXT PRIMARY KEY,
    group_id TEXT NOT NULL,
    name TEXT NOT NULL,
    position BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);

-- `ui` is a card-kit card. `reducers` maps action ids to how the server
-- folds them into the card's data. `module` is an optional wasm bundle,
-- base64, that the client runs on each action.
CREATE TABLE cards (
    id TEXT PRIMARY KEY,
    feed_id TEXT NOT NULL,
    author_did TEXT NOT NULL,
    ui TEXT NOT NULL,
    reducers TEXT NOT NULL,
    module TEXT,
    version BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX cards_by_feed ON cards (feed_id, created_at);

CREATE TABLE card_actions (
    id TEXT PRIMARY KEY,
    card_id TEXT NOT NULL,
    did TEXT NOT NULL,
    block_id TEXT NOT NULL,
    action_id TEXT NOT NULL,
    value TEXT NOT NULL,
    item TEXT NOT NULL,
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
