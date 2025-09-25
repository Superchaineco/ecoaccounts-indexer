CREATE TABLE IF NOT EXISTS indexed_ranges (
    strategy_name TEXT PRIMARY KEY,  -- Una fila por estrategia
    from_block BIGINT NOT NULL,      -- Rango acumulado: inicio
    to_block BIGINT NOT NULL,        -- Rango acumulado: fin
    last_updated TIMESTAMP DEFAULT NOW()
);