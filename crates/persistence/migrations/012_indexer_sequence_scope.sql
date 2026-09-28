-- ============================================================================
-- 012 — yog_indexer draws only from the sequences of the tables it writes
-- ============================================================================
-- The baseline granted `USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public` to
-- yog_indexer (§12), and `setup_roles.sql` does the same for every future
-- sequence by default. "All" took in two tables the indexer never writes:
-- `signals` (yog_signals appends to it) and `announcements` (written by hand by
-- the operator). Measured on 25 Sept 2026: `nextval('signals_id_seq')` succeeds
-- under yog_indexer, which cannot insert a row there (`42501`). The only effect
-- is burning identifiers, but it is a write the role matrix says it cannot make.
--
-- USAGE only: it is what `nextval` needs. SELECT stays, like every read.
--
-- ⚠️ The default privilege on sequences in `setup_roles.sql` is left as is: most
-- new tables are event tables the indexer writes. A future BIGSERIAL table it
-- does not write gets the same leak unless its migration revokes it too.
-- ============================================================================

REVOKE USAGE ON SEQUENCE signals_id_seq, announcements_id_seq FROM yog_indexer;
