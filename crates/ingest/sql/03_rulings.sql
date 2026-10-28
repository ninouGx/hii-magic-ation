CREATE TABLE rulings AS
SELECT oracle_id, source, published_at, comment
FROM read_json($rulings, format = 'newline_delimited', columns = {
    oracle_id: 'VARCHAR',
    source: 'VARCHAR',
    published_at: 'DATE',
    comment: 'VARCHAR'
})
WHERE oracle_id IN (SELECT oracle_id FROM cards);
