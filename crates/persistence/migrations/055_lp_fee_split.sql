-- 055_lp_fee_split.sql — the LPs' fees split between those claimed and those compounded into the
-- pool, valued by yog-analytic from the raw swaps of each tranche. No backfill: measured from the
-- first tranche after this migration.

-- NULL on a tranche not valued, on one without its raw swaps, and on the rows written before.
ALTER TABLE analytic_pool_5min
    ADD COLUMN claiming_fees_usd    NUMERIC,
    ADD COLUMN compounding_fees_usd NUMERIC;

ALTER TABLE analytic_pool_hourly
    ADD COLUMN claiming_fees_usd    NUMERIC,
    ADD COLUMN compounding_fees_usd NUMERIC;

ALTER TABLE analytic_protocol_hourly
    ADD COLUMN claiming_fees_usd    NUMERIC,
    ADD COLUMN compounding_fees_usd NUMERIC;
