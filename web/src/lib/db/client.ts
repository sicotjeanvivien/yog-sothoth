import postgres, { type Sql } from "postgres";
import { DatabaseError } from "./errors";

// Singleton postgres.js client.
//
// A single connection pool is shared across the entire Node.js
// process. The `globalThis` trick prevents `next dev` hot reloads
// from leaking pools across module re-evaluations: in production
// the singleton lives in the module cache, in development it lives
// on `globalThis` so every reload picks up the same instance.

type GlobalSqlHolder = {
  __yogSothothSql?: Sql;
};

const globalForSql = globalThis as unknown as GlobalSqlHolder;

/**
 * Build a new postgres.js client.
 *
 * Throws `DatabaseError(kind="connection")` synchronously if the
 * `DATABASE_URL` env var is absent. Network-level failures surface
 * later, on the first query, and are wrapped by callers.
 */
function createClient(): Sql {
  const url = process.env.DATABASE_URL;
  if (url === undefined || url === "") {
    throw new DatabaseError(
      "connection",
      "DATABASE_URL is not set; the web app cannot reach the database.",
    );
  }

  return postgres(url, {
    // Conservative pool settings for v0.1. Web traffic is a single
    // dashboard user; a tiny pool avoids saturating the read-only
    // role that the migration provisioned.
    max: 5,
    idle_timeout: 20,
    connect_timeout: 10,
    // Disable prepared statements: the benefit is marginal at our
    // scale and they complicate the mock-based unit tests.
    prepare: false,
    // Keep field names exactly as PostgreSQL returns them. The
    // repository layer is responsible for snake_case → camelCase.
    transform: { undefined: null },
  });
}

/**
 * Return the process-wide postgres.js client, creating it on first
 * access. Safe to import from any server-side module.
 */
export function getSql(): Sql {
  // Reuse the existing instance whenever possible.
  if (globalForSql.__yogSothothSql !== undefined) {
    return globalForSql.__yogSothothSql;
  }

  const sql = createClient();

  // Persist on globalThis only outside production. In production
  // the regular module cache already keeps the singleton alive and
  // adding it to globalThis would only obscure the lifecycle.
  if (process.env.NODE_ENV !== "production") {
    globalForSql.__yogSothothSql = sql;
  }

  return sql;
}

/**
 * Tear down the shared pool. Intended for graceful shutdown hooks
 * and for tests that want a fresh client between runs.
 */
export async function closeSql(): Promise<void> {
  const sql = globalForSql.__yogSothothSql;
  if (sql !== undefined) {
    await sql.end({ timeout: 5 });
    delete globalForSql.__yogSothothSql;
  }
}