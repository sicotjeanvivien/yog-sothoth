-- 051_ingest_coverage.sql — the slots each ingestion session observed, per protocol watched whole,
-- with the chain's time at each end. A gap is never written: `ingest_coverage_gaps` is the
-- complement of the ranges, so a killed process leaves one without anyone deciding it.

-- ⚠️ Keyed on the session, not on `observed_from`: a replay delivers its oldest slots last, so the
-- low end of a live session still moves.
CREATE TABLE ingest_coverage (
    protocol           TEXT        NOT NULL,
    session_started_at TIMESTAMPTZ NOT NULL,
    observed_from      BIGINT      NOT NULL,
    observed_to        BIGINT      NOT NULL,
    observed_from_at   TIMESTAMPTZ NOT NULL,
    observed_to_at     TIMESTAMPTZ NOT NULL,
    updated_at         TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (protocol, session_started_at),
    CHECK (observed_from <= observed_to)
);

-- ⚠️ Against the highest end of every range before, not the previous one only: a range inside
-- another would otherwise open a gap that was observed.
CREATE VIEW ingest_coverage_gaps AS
WITH ranked AS (
    SELECT protocol, observed_from, observed_from_at,
           max(observed_to)    OVER before AS covered_to,
           max(observed_to_at) OVER before AS covered_to_at
      FROM ingest_coverage
    WINDOW before AS (PARTITION BY protocol ORDER BY observed_from, session_started_at
                      ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING)
)
SELECT protocol,
       covered_to + 1     AS missing_from,
       observed_from - 1  AS missing_to,
       covered_to_at      AS last_seen_at,
       observed_from_at   AS seen_again_at
  FROM ranked
 WHERE observed_from > covered_to + 1;

GRANT SELECT, INSERT, UPDATE ON ingest_coverage TO yog_indexer;
GRANT SELECT ON ingest_coverage, ingest_coverage_gaps TO yog_api;
