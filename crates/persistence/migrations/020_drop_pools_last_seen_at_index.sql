-- 020_drop_pools_last_seen_at_index.sql — pools without its last_seen_at index.
-- Every event rewrites last_seen_at, and an update of an indexed column cannot
-- be HOT: it writes an entry in every index of the table.
-- No query filters or sorts on the column; reads by address use pools_pkey.

DROP INDEX idx_pools_last_seen_at;
