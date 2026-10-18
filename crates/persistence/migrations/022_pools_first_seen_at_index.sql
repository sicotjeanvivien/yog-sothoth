-- 022_pools_first_seen_at_index.sql — pools indexed on first_seen_at, for the
-- metadata worker's search over the pools discovered within the last hour.
-- ⚠️ Keeps updates of pools HOT (020) only while first_seen_at stays written at INSERT alone.

CREATE INDEX idx_pools_first_seen_at ON pools (first_seen_at);
