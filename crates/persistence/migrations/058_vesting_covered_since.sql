-- 058_vesting_covered_since.sql — the first vesting lock observed, written with each locked reading:
-- the vestings locked before it are out of reach. NULL on the readings written before.

ALTER TABLE analytic_protocol_tvl ADD COLUMN vesting_covered_since TIMESTAMPTZ;
