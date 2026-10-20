-- 030_tvl_floor.sql — the protocol TVL of the hours before the first complete
-- one is false for good: empty, or a third of the pools. Deleted, so the first
-- row is the floor of a TVL variation, read like the first hour of the facts.

DELETE FROM analytic_protocol_tvl WHERE bucket < '2026-10-18 09:00:00+00';
