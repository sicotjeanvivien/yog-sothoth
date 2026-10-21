-- 033_swap_events_5min_policies.sql — the policies of the 5-minute aggregate recreated by 031,
-- once 032 has materialised it: the same as 021's.

-- ⚠️ `end_offset` is the freeze margin: a row landing in a materialised tranche
-- stays unseen until the next refresh. `start_offset` stays under the raw 7 days (008).
SELECT add_continuous_aggregate_policy('meteora_damm_v2_swap_events_5min',
    start_offset         => INTERVAL '6 days',
    end_offset           => INTERVAL '10 minutes',
    schedule_interval    => INTERVAL '5 minutes',
    -- ⚠️ Newest first, the watermark leads while older tranches are still empty.
    refresh_newest_first => false);

-- ⚠️ Past the raw 7 days: an hour caught up after a week-long outage reads its tranches.
SELECT add_retention_policy('meteora_damm_v2_swap_events_5min', INTERVAL '30 days');
