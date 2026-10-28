CREATE TEMP TABLE edhrec_pages AS
SELECT
    regexp_extract(filename, 'commanders/([a-z]+)/[0-9]+\.json$', 1) AS period,
    filename,
    coalesce(cardviews, container.json_dict.cardlists[1].cardviews) AS cardviews
FROM read_json($edhrec_commander_pages, filename = true, columns = {
    cardviews: 'STRUCT(id VARCHAR, name VARCHAR, sanitized VARCHAR, num_decks INTEGER, rank INTEGER, trend_zscore DOUBLE, is_partner BOOLEAN, names VARCHAR[])[]',
    container: 'STRUCT(json_dict STRUCT(cardlists STRUCT(cardviews STRUCT(id VARCHAR, name VARCHAR, sanitized VARCHAR, num_decks INTEGER, rank INTEGER, trend_zscore DOUBLE, is_partner BOOLEAN, names VARCHAR[])[])[]))'
});

SELECT error('EDHREC page without cardviews, the format may have changed: ' || filename)
FROM edhrec_pages
WHERE cardviews IS NULL OR period = ''
LIMIT 1;

CREATE TEMP TABLE edhrec_entries AS
SELECT period, unnest(cardviews, recursive := true)
FROM edhrec_pages;

CREATE TEMP TABLE card_names AS
SELECT oracle_id, lower(name) AS key FROM cards
UNION
SELECT oracle_id, lower(front_name) FROM cards;

CREATE TEMP TABLE edhrec_resolved AS
SELECT
    e.period,
    e.rank,
    e.num_decks,
    e.trend_zscore,
    e.sanitized,
    e.name,
    e.id,
    coalesce(
        (SELECT p.oracle_id FROM prints AS p WHERE p.print_id = e.id),
        (SELECT min(n.oracle_id) FROM card_names AS n WHERE n.key = lower(e.name))
    ) AS solo_oracle_id,
    (SELECT min(n.oracle_id) FROM card_names AS n WHERE n.key = lower(e.names[1])) AS first_oracle_id,
    (SELECT min(n.oracle_id) FROM card_names AS n WHERE n.key = lower(e.names[2])) AS second_oracle_id,
    coalesce(e.is_partner, false) AND len(e.names) = 2 AS is_pair
FROM edhrec_entries AS e;

INSERT INTO edhrec_commander_ranks
SELECT DISTINCT ON (oracle_id, partner_oracle_id, period) *
FROM (
    SELECT solo_oracle_id AS oracle_id, '' AS partner_oracle_id, period, rank, num_decks, trend_zscore, sanitized
    FROM edhrec_resolved
    WHERE NOT is_pair AND solo_oracle_id IS NOT NULL
    UNION ALL
    SELECT first_oracle_id, second_oracle_id, period, rank, num_decks, trend_zscore, sanitized
    FROM edhrec_resolved
    WHERE is_pair AND first_oracle_id IS NOT NULL AND second_oracle_id IS NOT NULL
    UNION ALL
    SELECT second_oracle_id, first_oracle_id, period, rank, num_decks, trend_zscore, sanitized
    FROM edhrec_resolved
    WHERE is_pair AND first_oracle_id IS NOT NULL AND second_oracle_id IS NOT NULL
)
ORDER BY oracle_id, partner_oracle_id, period, rank;

INSERT INTO edhrec_unresolved
SELECT period, rank, name, id
FROM edhrec_resolved
WHERE (NOT is_pair AND solo_oracle_id IS NULL)
    OR (is_pair AND (first_oracle_id IS NULL OR second_oracle_id IS NULL));
