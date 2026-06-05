-- Rename `token_metadata.metadata_source` → `token_metadata.metadata_provider`
-- to match the domain rename of `TokenMetadata.metadata_source` →
-- `TokenMetadata.metadata_provider`, and align the column name with
-- the `token_prices.price_provider` convention.
--
-- The DEFAULT ('helius_das') is preserved by Postgres across a
-- RENAME COLUMN — no need to re-declare it. Existing values are
-- unchanged.

ALTER TABLE token_metadata RENAME COLUMN metadata_source TO metadata_provider;