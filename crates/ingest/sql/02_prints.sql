CREATE TABLE prints AS
WITH src AS (
    SELECT *
    FROM read_json($default_cards, format = 'newline_delimited', columns = {
        id: 'VARCHAR',
        oracle_id: 'VARCHAR',
        lang: 'VARCHAR',
        set: 'VARCHAR',
        set_type: 'VARCHAR',
        collector_number: 'VARCHAR',
        digital: 'BOOLEAN',
        rarity: 'VARCHAR',
        released_at: 'DATE',
        border_color: 'VARCHAR',
        promo: 'BOOLEAN',
        full_art: 'BOOLEAN',
        frame_effects: 'VARCHAR[]',
        artist: 'VARCHAR',
        flavor_text: 'VARCHAR',
        illustration_id: 'VARCHAR',
        image_uris: 'STRUCT(small VARCHAR, normal VARCHAR, art_crop VARCHAR)',
        prices: 'STRUCT(eur VARCHAR, usd VARCHAR)',
        card_faces: 'STRUCT(artist VARCHAR, flavor_text VARCHAR, illustration_id VARCHAR, image_uris STRUCT(small VARCHAR, normal VARCHAR, art_crop VARCHAR))[]'
    })
    WHERE lang = 'en'
)
SELECT
    id AS print_id,
    oracle_id,
    set AS set_code,
    set_type,
    collector_number,
    coalesce(digital, false) AS digital,
    rarity,
    released_at,
    border_color,
    coalesce(promo, false) AS promo,
    coalesce(full_art, false) AS full_art,
    coalesce(frame_effects, []) AS frame_effects,
    coalesce(artist, card_faces[1].artist) AS artist,
    coalesce(flavor_text, nullif(array_to_string(list_filter(list_transform(card_faces, lambda f: f.flavor_text), lambda t: t IS NOT NULL), chr(10)), '')) AS flavor_text,
    list_distinct(list_filter(list_prepend(illustration_id, list_transform(card_faces, lambda f: f.illustration_id)), lambda i: i IS NOT NULL)) AS illustration_ids,
    coalesce(image_uris.small, card_faces[1].image_uris.small) AS image_small,
    coalesce(image_uris.normal, card_faces[1].image_uris.normal) AS image_normal,
    coalesce(image_uris.art_crop, card_faces[1].image_uris.art_crop) AS image_art_crop,
    TRY_CAST(prices.eur AS DECIMAL(10, 2)) AS price_eur,
    TRY_CAST(prices.usd AS DECIMAL(10, 2)) AS price_usd
FROM src
WHERE oracle_id IN (SELECT oracle_id FROM cards);

CREATE TABLE print_illustrations AS
SELECT DISTINCT print_id, unnest(illustration_ids) AS illustration_id
FROM prints;

ALTER TABLE cards ADD COLUMN default_print_id VARCHAR;

UPDATE cards SET default_print_id = ranked.print_id
FROM (
    SELECT
        oracle_id,
        print_id,
        row_number() OVER (
            PARTITION BY oracle_id
            ORDER BY
                NOT digital
                    AND NOT promo
                    AND NOT full_art
                    AND border_color = 'black'
                    AND NOT list_has_any(frame_effects, ['showcase', 'extendedart', 'etched', 'inverted'])
                    AND set_type IN ('core', 'expansion', 'commander', 'masters', 'draft_innovation')
                    AND TRY_CAST(collector_number AS INTEGER) IS NOT NULL DESC,
                NOT promo AND NOT digital DESC,
                released_at DESC,
                TRY_CAST(collector_number AS INTEGER) NULLS LAST,
                print_id
        ) AS rank
    FROM prints
) AS ranked
WHERE ranked.oracle_id = cards.oracle_id AND ranked.rank = 1;
