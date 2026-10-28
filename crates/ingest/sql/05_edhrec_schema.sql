CREATE TABLE edhrec_commander_ranks (
    oracle_id VARCHAR NOT NULL,
    partner_oracle_id VARCHAR NOT NULL,
    period VARCHAR NOT NULL,
    rank INTEGER NOT NULL,
    num_decks INTEGER,
    trend_zscore DOUBLE,
    edhrec_slug VARCHAR,
    PRIMARY KEY (oracle_id, partner_oracle_id, period)
);

CREATE TABLE edhrec_unresolved (
    period VARCHAR NOT NULL,
    rank INTEGER,
    name VARCHAR,
    edhrec_id VARCHAR
);
