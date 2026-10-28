-- 041_token_prices_sized_to_its_readers.sql — the price series in 1-day chunks, each compressed a
-- day after it closes: every reader asks for a recent price, or already reads a compressed chunk.
-- ⚠️ The chunk open when this runs keeps its 7 days; `001` and `011` still say 7, this file holds.
-- Policies are replaced through remove/add: `setup_roles.sql` revokes the job API, `alter_job` too.

SELECT set_chunk_time_interval('token_prices', INTERVAL '1 day');

SELECT remove_compression_policy('token_prices', if_exists => true);
SELECT add_compression_policy   ('token_prices', INTERVAL '1 day');
