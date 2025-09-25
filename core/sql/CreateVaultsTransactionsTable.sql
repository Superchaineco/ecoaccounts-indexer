CREATE TABLE vaults_transactions (
    id BIGSERIAL PRIMARY KEY,
    account TEXT NOT NULL,
    token TEXT NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
    tx_hash TEXT NOT NULL,
    tx_block BIGINT NOT NULL,
    block_time TIMESTAMPTZ NOT NULL,
    UNIQUE (account, token, tx_hash, direction)
);