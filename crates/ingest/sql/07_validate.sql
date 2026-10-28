SELECT error('cards: table is empty') WHERE (SELECT count(*) FROM cards) = 0;
SELECT error('cards: oracle_id or name missing, the bulk format may have changed')
WHERE EXISTS (SELECT 1 FROM cards WHERE oracle_id IS NULL OR name IS NULL OR legalities IS NULL);
SELECT error('cards: duplicate oracle_id')
WHERE (SELECT count(*) FROM cards) <> (SELECT count(DISTINCT oracle_id) FROM cards);
SELECT error('prints: table is empty') WHERE (SELECT count(*) FROM prints) = 0;
SELECT error('tags: tag without slug or kind, the tag format may have changed')
WHERE EXISTS (SELECT 1 FROM tags WHERE slug IS NULL OR kind IS NULL);
