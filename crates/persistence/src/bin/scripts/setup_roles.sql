-- Postgres roles and structural privileges, run as admin before any migration
-- (`yog-migrate setup-roles`). Roles are cluster-wide, the rest per database; idempotent.
-- ⚠️ A rerun never resets an existing role's password: replace the `CHANGE_ME_…` placeholders
-- before the first run, and change a password with `ALTER ROLE`. Every runtime role reads
-- `public`; what each writes is granted per table in the migrations.


-- Roles — cluster scope: created once per cluster, not once per database.
DO $$
DECLARE
    role_name TEXT;
    -- Password only ever applies to a role this block actually creates.
    passwords CONSTANT JSONB := jsonb_build_object(
        'yog_migrate', 'CHANGE_ME_migrate_password',
        'yog_indexer', 'CHANGE_ME_indexer_password',
        'yog_api',     'CHANGE_ME_api_password',
        'yog_context', 'CHANGE_ME_context_password',
        'yog_signals', 'CHANGE_ME_signals_password',
        'yog_archive', 'CHANGE_ME_archive_password',
        'yog_analytic', 'CHANGE_ME_analytic_password'
    );
BEGIN
    FOREACH role_name IN ARRAY ARRAY[
        'yog_migrate', 'yog_indexer', 'yog_api', 'yog_context', 'yog_signals',
        'yog_archive', 'yog_analytic'
    ] LOOP
        IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = role_name) THEN
            RAISE NOTICE 'role % already exists — left untouched (password included)', role_name;
        ELSE
            EXECUTE format(
                'CREATE ROLE %I LOGIN PASSWORD %L',
                role_name, passwords ->> role_name
            );
            RAISE NOTICE 'role % created', role_name;
        END IF;
    END LOOP;
END $$;



-- Schema access — database scope: re-run for every database of the cluster.
GRANT USAGE ON SCHEMA public TO yog_indexer, yog_api, yog_context, yog_signals, yog_analytic;

-- Owning the schema gives yog_migrate GRANT authority over the tables it creates.
ALTER SCHEMA public OWNER TO yog_migrate;
GRANT USAGE, CREATE ON SCHEMA public TO yog_migrate;


-- Default privileges, SELECT only — database scope. ⚠️ FOR ROLE yog_migrate: they apply only
-- to what it creates. They also hide a lost explicit grant: `tests/privileges.rs` guards it.
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_indexer;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_api;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_context;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_signals;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_analytic;

-- `public` is enough for yog_archive too: the internal tables of a hypertable
-- or a continuous aggregate inherit its grants.
ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON TABLES TO yog_archive;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT SELECT ON SEQUENCES TO yog_archive;

ALTER DEFAULT PRIVILEGES FOR ROLE yog_migrate IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO yog_indexer;


-- What PUBLIC holds by default, taken back — database scope: TimescaleDB's job API and TEMP.
-- ⚠️ An `ALTER EXTENSION timescaledb UPDATE` can restore `alter_job`: re-run this file after it.
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- ⚠️ yog_migrate loses them too, and CI migrates as superuser: a migration calling `alter_job`
-- needs its own GRANT. `ROUTINE`: `run_job` is a procedure.
REVOKE EXECUTE ON ROUTINE add_job, alter_job, delete_job, run_job FROM PUBLIC;

DO $$
BEGIN
    EXECUTE format('REVOKE TEMPORARY ON DATABASE %I FROM PUBLIC', current_database());
END $$;


-- yog_archive — what `pg_dump` reads and already exists — database scope. Granted only where
-- missing, `public` first so chunks inherit. ⚠️ An object yog_migrate did not create fails the
-- next dump until this file runs again.
DO $$
DECLARE
    membership RECORD;
BEGIN
    FOR membership IN
        SELECT m.member::REGROLE AS member, m.grantor::REGROLE AS grantor
          FROM pg_auth_members m
         WHERE m.roleid = 'pg_read_all_data'::REGROLE
           AND m.member::REGROLE::TEXT LIKE 'yog\_%'
    LOOP
        EXECUTE format('REVOKE pg_read_all_data FROM %s GRANTED BY %s',
                       membership.member, membership.grantor);
    END LOOP;
    IF EXISTS (SELECT 1 FROM pg_roles
                WHERE rolname LIKE 'yog\_%'
                  AND pg_has_role(oid, 'pg_read_all_data', 'MEMBER')) THEN
        RAISE EXCEPTION 'a yog_* role still reads pg_authid through pg_read_all_data';
    END IF;
END $$;

DO $$
DECLARE
    schema_name TEXT;
    relation    RECORD;
BEGIN
    FOR schema_name IN
        SELECT nspname FROM pg_namespace
         WHERE nspname NOT LIKE 'pg\_%' AND nspname <> 'information_schema'
           AND NOT has_schema_privilege('yog_archive', oid, 'USAGE')
    LOOP
        EXECUTE format('GRANT USAGE ON SCHEMA %I TO yog_archive', schema_name);
    END LOOP;

    FOR relation IN
        SELECT c.oid, c.relkind
          FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname NOT LIKE 'pg\_%' AND n.nspname <> 'information_schema'
           AND c.relkind IN ('r', 'p', 'v', 'm', 'f', 'S')
         ORDER BY n.nspname <> 'public'
    LOOP
        IF NOT has_table_privilege('yog_archive', relation.oid, 'SELECT') THEN
            EXECUTE format('GRANT SELECT ON %s %s TO yog_archive',
                           CASE relation.relkind WHEN 'S' THEN 'SEQUENCE' ELSE 'TABLE' END,
                           relation.oid::REGCLASS);
        END IF;
    END LOOP;
END $$;
