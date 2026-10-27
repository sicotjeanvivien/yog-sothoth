-- ⚠️ `NOT EXISTS`, never `NOT IN`: unhashed once `token_metadata` outgrows `work_mem`, `NOT IN`
-- rereads the whole table for each mint of the pools.
SELECT all_mints.mint AS "mint!: String" FROM (
    SELECT token_a_mint AS mint FROM pools
    UNION
    SELECT token_b_mint AS mint FROM pools
) AS all_mints
-- ⚠️ `IS NOT NULL` is load-bearing: `NOT EXISTS` keeps a NULL mint, which fails the `!`.
WHERE all_mints.mint IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM token_metadata tm WHERE tm.mint = all_mints.mint)
