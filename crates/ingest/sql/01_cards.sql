CREATE TABLE cards AS
WITH src AS (
    SELECT *
    FROM read_json($oracle_cards, format = 'newline_delimited', columns = {
        oracle_id: 'VARCHAR',
        name: 'VARCHAR',
        layout: 'VARCHAR',
        mana_cost: 'VARCHAR',
        cmc: 'DOUBLE',
        type_line: 'VARCHAR',
        oracle_text: 'VARCHAR',
        colors: 'VARCHAR[]',
        color_identity: 'VARCHAR[]',
        keywords: 'VARCHAR[]',
        power: 'VARCHAR',
        toughness: 'VARCHAR',
        loyalty: 'VARCHAR',
        legalities: 'MAP(VARCHAR, VARCHAR)',
        edhrec_rank: 'INTEGER',
        game_changer: 'BOOLEAN',
        all_parts: 'STRUCT(component VARCHAR, name VARCHAR)[]',
        card_faces: 'STRUCT(name VARCHAR, mana_cost VARCHAR, type_line VARCHAR, oracle_text VARCHAR, power VARCHAR, toughness VARCHAR, loyalty VARCHAR, colors VARCHAR[])[]'
    })
    WHERE layout NOT IN ('art_series', 'token', 'double_faced_token', 'emblem', 'planar', 'scheme', 'vanguard', 'front_card')
),
resolved AS (
    SELECT
        *,
        coalesce(card_faces[1].type_line, type_line) AS front_type_line,
        coalesce(oracle_text, array_to_string(list_transform(card_faces, lambda f: f.oracle_text), chr(10) || '//' || chr(10))) AS full_text,
        coalesce(power, card_faces[1].power) AS front_power,
        coalesce(toughness, card_faces[1].toughness) AS front_toughness,
        coalesce(loyalty, card_faces[1].loyalty) AS front_loyalty,
        coalesce(list_contains(list_transform(list_filter(all_parts, lambda p: p.component = 'meld_result'), lambda p: p.name), name), false) AS is_meld_result
    FROM src
)
SELECT
    oracle_id,
    name,
    coalesce(card_faces[1].name, name) AS front_name,
    coalesce(mana_cost, card_faces[1].mana_cost) AS mana_cost,
    cmc,
    type_line,
    full_text AS oracle_text,
    coalesce(colors, list_sort(list_distinct(flatten(list_transform(card_faces, lambda f: f.colors))))) AS colors,
    color_identity,
    keywords,
    front_power AS power,
    TRY_CAST(front_power AS DOUBLE) AS power_num,
    front_toughness AS toughness,
    TRY_CAST(front_toughness AS DOUBLE) AS toughness_num,
    front_loyalty AS loyalty,
    TRY_CAST(front_loyalty AS DOUBLE) AS loyalty_num,
    layout,
    card_faces,
    legalities,
    coalesce(legalities['commander'] = 'legal', false)
        AND NOT is_meld_result
        AND (
            regexp_matches(front_type_line, '\bLegendary\b.*\bCreature\b')
            OR regexp_matches(front_type_line, '\bBackground\b')
            OR (regexp_matches(front_type_line, '\bLegendary\b.*\b(Vehicle|Spacecraft)\b') AND front_power IS NOT NULL)
            OR contains(lower(full_text), 'can be your commander')
            OR regexp_matches(full_text, 'isn''t on the battlefield, it''s a .*creature')
        ) AS is_commander_eligible,
    edhrec_rank,
    coalesce(game_changer, false) AS game_changer
FROM resolved;
