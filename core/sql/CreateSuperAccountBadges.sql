CREATE TABLE badge_claims (
    badge_id INT NOT NULL,
    account TEXT NOT NULL REFERENCES super_accounts(account) ON DELETE CASCADE,
    tier INT,
    points INT,
    block_number INT NOT NULL,
    tx_hash TEXT NOT NULL,
    claimed_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (badge_id, tier, account, block_number)
);