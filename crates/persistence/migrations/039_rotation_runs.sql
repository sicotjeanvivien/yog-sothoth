-- 039_rotation_runs.sql — the bursts of 036 renamed rotation runs, the name of their route:
-- the two tables, their indexes, and every constraint Postgres named after them. Rows, keys
-- and grants follow the tables; nothing else changes.

ALTER TABLE analytic_bursts RENAME TO analytic_rotation_runs;
ALTER TABLE analytic_burst_hours RENAME TO analytic_rotation_run_hours;

ALTER INDEX analytic_bursts_pkey RENAME TO analytic_rotation_runs_pkey;
ALTER INDEX analytic_bursts_by_protocol RENAME TO analytic_rotation_runs_by_protocol;
ALTER INDEX analytic_bursts_unsettled RENAME TO analytic_rotation_runs_unsettled;
ALTER INDEX analytic_burst_hours_pkey RENAME TO analytic_rotation_run_hours_pkey;
ALTER INDEX analytic_burst_hours_by_protocol RENAME TO analytic_rotation_run_hours_by_protocol;

-- ⚠️ Their names depend on the Postgres version (18 names each NOT NULL): renamed by prefix,
-- never listed by hand.
DO $$
DECLARE
    c RECORD;
BEGIN
    FOR c IN
        SELECT conrelid::regclass AS tbl, conname
          FROM pg_constraint
         WHERE conrelid IN ('analytic_rotation_runs'::regclass,
                            'analytic_rotation_run_hours'::regclass)
           AND conname LIKE 'analytic\_burst%'
    LOOP
        EXECUTE format(
            'ALTER TABLE %s RENAME CONSTRAINT %I TO %I',
            c.tbl,
            c.conname,
            replace(replace(c.conname, 'analytic_burst_hours', 'analytic_rotation_run_hours'),
                    'analytic_bursts', 'analytic_rotation_runs')
        );
    END LOOP;
END
$$;
