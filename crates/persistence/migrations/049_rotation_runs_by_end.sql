-- 049_rotation_runs_by_end.sql — the runs that reach into a window, found by their end: the last
-- hour's split reads them on every request, and the table keeps every run ever drawn.

CREATE INDEX analytic_rotation_runs_by_end ON analytic_rotation_runs (protocol, ended_at DESC);
