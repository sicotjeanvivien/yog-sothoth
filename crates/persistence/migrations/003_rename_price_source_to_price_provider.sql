-- Rename `token_prices.price_source` → `token_prices.price_provider`
-- to match the domain rename of `TokenPrice.price_source` →
-- `TokenPrice.price_provider`.
--
-- The values stored in the column ('jupiter' | 'helius' | 'fallback')
-- are not affected — only the column name changes.
--
-- TimescaleDB propagates the RENAME to every existing chunk in the
-- hypertable. No data is rewritten.

ALTER TABLE token_prices RENAME COLUMN price_source TO price_provider;