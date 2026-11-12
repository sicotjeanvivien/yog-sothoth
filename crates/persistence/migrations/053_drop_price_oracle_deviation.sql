-- 053_drop_price_oracle_deviation.sql — the price_oracle_deviation detector is gone, with its one
-- read, the pool_price_snapshot view, and every signal it stored: a spot-versus-oracle gap above a
-- threshold described a pool's state, not an event.

DROP VIEW pool_price_snapshot;

DELETE FROM signals WHERE detector = 'price_oracle_deviation';
