CREATE TABLE super_accounts (
    account TEXT PRIMARY KEY,
    nationality TEXT,
    username TEXT NOT NULL,
    eoas TEXT[] NOT NULL,
    level INTEGER NOT NULL,
    noun JSONB NOT NULL,
    total_points INTEGER NOT NULL,
    total_badges INTEGER NOT NULL,
    last_update_block_number INTEGER,
    last_update_tx_hash TEXT
);