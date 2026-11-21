-- Similar titles come from Similar sources now (ADR 0084): a row is one source's answer about a
-- title, and each title in it has a weight, such as '[{"id":"tmdb-679","weight":1.0}]'. The rows
-- so far are the TMDB Plugin's, which is a Similar source too, so they stay, each title weighed by
-- its rank as that source weighs it: 1.0 for the first, less for each after.
UPDATE similar_titles SET similar = (
    SELECT json_group_array(json_object(
        'id', ranked.value,
        'weight', 1.0 - ranked.key * 1.0 / json_array_length(similar_titles.similar)))
    FROM (SELECT key, value FROM json_each(similar_titles.similar) ORDER BY key) AS ranked
);
