-- no-transaction
-- 032_swap_events_5min_materialised.sql — the 5-minute aggregate recreated by 031,
-- materialised over every raw swap at once: its policy refreshes 6 days only, and the
-- older tranches, under its watermark yet never materialised, would read empty.
-- ⚠️ Outside a transaction, which a refresh refuses: the deploy waits for it.

CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_5min', NULL, now() - INTERVAL '10 minutes');
