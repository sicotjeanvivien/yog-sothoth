SELECT all_mints.mint AS "mint!: String" FROM (
    SELECT token_a_mint AS mint FROM pools
    UNION
    SELECT token_b_mint AS mint FROM pools
) AS all_mints
-- ⚠️ `IS NOT NULL` is load-bearing: `NOT EXISTS` keeps a NULL mint, which fails the `!`.
WHERE all_mints.mint IS NOT NULL
  -- ⚠️ Never `NOT IN`: unhashed past `work_mem`, it rereads `token_metadata` for each mint.
  AND NOT EXISTS (SELECT 1 FROM token_metadata tm WHERE tm.mint = all_mints.mint)
