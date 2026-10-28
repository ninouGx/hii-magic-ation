CREATE TEMP TABLE raw_tags AS
SELECT *
FROM read_json([$oracle_tags, $art_tags], format = 'newline_delimited', columns = {
    id: 'VARCHAR',
    slug: 'VARCHAR',
    label: 'VARCHAR',
    type: 'VARCHAR',
    description: 'VARCHAR',
    aliases: 'VARCHAR[]',
    parent_ids: 'VARCHAR[]',
    taggings: 'STRUCT(oracle_id VARCHAR, illustration_id VARCHAR, weight VARCHAR, annotation VARCHAR)[]'
});

CREATE TABLE tags AS
SELECT
    id AS tag_id,
    slug,
    label,
    CASE type WHEN 'illustration' THEN 'art' ELSE type END AS kind,
    description,
    coalesce(aliases, []) AS aliases
FROM raw_tags;

CREATE TABLE tag_edges AS
SELECT DISTINCT unnest(parent_ids) AS parent_id, id AS child_id
FROM raw_tags;

CREATE TABLE tag_ancestors AS
WITH RECURSIVE ancestors(tag_id, ancestor_id) AS (
    SELECT tag_id, tag_id FROM tags
    UNION
    SELECT a.tag_id, e.parent_id
    FROM ancestors AS a
    JOIN tag_edges AS e ON e.child_id = a.ancestor_id
)
SELECT tag_id, ancestor_id FROM ancestors;

CREATE TABLE card_tags AS
SELECT
    id AS tag_id,
    CASE WHEN t.oracle_id IS NOT NULL THEN 'oracle' ELSE 'illustration' END AS target_kind,
    coalesce(t.oracle_id, t.illustration_id) AS target_id,
    t.weight,
    t.annotation
FROM (SELECT id, unnest(taggings) AS t FROM raw_tags);

CREATE TABLE card_tag_flat AS
SELECT DISTINCT oracle_id, tag_id
FROM (
    SELECT ct.target_id AS oracle_id, ct.tag_id
    FROM card_tags AS ct
    WHERE ct.target_kind = 'oracle'
    UNION ALL
    SELECT p.oracle_id, ct.tag_id
    FROM card_tags AS ct
    JOIN print_illustrations AS pi ON pi.illustration_id = ct.target_id
    JOIN prints AS p ON p.print_id = pi.print_id
    WHERE ct.target_kind = 'illustration'
)
WHERE oracle_id IN (SELECT oracle_id FROM cards);
