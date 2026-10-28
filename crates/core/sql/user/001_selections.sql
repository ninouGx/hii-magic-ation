CREATE TABLE selections (
    selection_id VARCHAR PRIMARY KEY DEFAULT uuid()::VARCHAR,
    name VARCHAR NOT NULL,
    commander_oracle_id VARCHAR,
    partner_oracle_id VARCHAR,
    notes VARCHAR NOT NULL DEFAULT '',
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

CREATE TABLE selection_cards (
    selection_id VARCHAR NOT NULL,
    oracle_id VARCHAR NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1 CHECK (quantity > 0),
    print_id VARCHAR,
    category VARCHAR,
    added_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (selection_id, oracle_id)
);
