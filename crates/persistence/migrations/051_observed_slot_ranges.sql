-- 051_observed_slot_ranges.sql — the slots each ingestion session watched, per protocol watched
-- whole, with the chain's time at each end. A range nobody watched is never written:
-- `unobserved_slot_ranges` is the complement, so a killed process leaves one without anyone deciding it.

-- ⚠️ Keyed on the session, not on `first_slot`: a replay delivers its oldest slots last, so the
-- first slot of a live session still moves.
CREATE TABLE observed_slot_ranges (
    protocol           TEXT        NOT NULL,
    session_started_at TIMESTAMPTZ NOT NULL,
    first_slot         BIGINT      NOT NULL,
    last_slot          BIGINT      NOT NULL,
    first_slot_at      TIMESTAMPTZ NOT NULL,
    last_slot_at       TIMESTAMPTZ NOT NULL,
    updated_at         TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (protocol, session_started_at),
    CHECK (first_slot <= last_slot)
);

-- ⚠️ Against the highest last slot of every range before, not the previous one only: a range
-- inside another would otherwise read as unwatched.
CREATE VIEW unobserved_slot_ranges AS
WITH ranked AS (
    SELECT protocol, first_slot, first_slot_at,
           max(last_slot)    OVER before AS watched_until,
           max(last_slot_at) OVER before AS watched_until_at
      FROM observed_slot_ranges
    WINDOW before AS (PARTITION BY protocol ORDER BY first_slot, session_started_at
                      ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING)
)
SELECT protocol,
       watched_until + 1  AS first_slot,
       first_slot - 1     AS last_slot,
       watched_until_at   AS last_seen_at,
       first_slot_at      AS seen_again_at
  FROM ranked
 WHERE first_slot > watched_until + 1;

GRANT SELECT, INSERT, UPDATE ON observed_slot_ranges TO yog_indexer;
GRANT SELECT ON observed_slot_ranges, unobserved_slot_ranges TO yog_api;
