SELECT error('cards: table is empty') WHERE (SELECT count(*) FROM cards) = 0;
SELECT error('cards: oracle_id or name missing, the bulk format may have changed')
WHERE EXISTS (SELECT 1 FROM cards WHERE oracle_id IS NULL OR name IS NULL OR legalities IS NULL);
SELECT error('cards: type_line or cmc missing on more than 1 % of cards, the bulk format may have changed')
WHERE (SELECT count(*) FILTER (WHERE type_line IS NULL OR cmc IS NULL) * 100 > count(*) FROM cards);
SELECT error('cards: duplicate oracle_id')
WHERE (SELECT count(*) FROM cards) <> (SELECT count(DISTINCT oracle_id) FROM cards);
SELECT error('prints: table is empty') WHERE (SELECT count(*) FROM prints) = 0;
SELECT error('prints: image_small missing on more than 5 % of prints, the bulk format may have changed')
WHERE (SELECT count(*) FILTER (WHERE image_small IS NULL) * 20 > count(*) FROM prints);
SELECT error('tags: tag without slug or kind, the tag format may have changed')
WHERE EXISTS (SELECT 1 FROM tags WHERE slug IS NULL OR kind IS NULL);
