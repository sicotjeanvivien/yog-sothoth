// Domain-level error type for any database failure surfaced by the
// repositories. Wrapping the raw postgres.js / Zod errors at the
// boundary keeps Route Handlers free of driver-specific details and
// gives every database failure a single shape to handle.

/**
 * Categorizes the high-level reason a repository call failed.
 *
 * - `connection`: could not reach the database (network, auth, timeout)
 * - `query`: the SQL ran but the database returned an error
 * - `validation`: rows came back but failed schema validation, meaning
 *   the database state does not match what the application expects
 * - `unknown`: catch-all for anything we have not classified yet
 */
export type DatabaseErrorKind =
  | "connection"
  | "query"
  | "validation"
  | "unknown";

export class DatabaseError extends Error {
  public readonly kind: DatabaseErrorKind;
  // Preserved for diagnostic logging, never leaked to API consumers.
  public override readonly cause: unknown;

  constructor(kind: DatabaseErrorKind, message: string, cause?: unknown) {
    super(message);
    this.name = "DatabaseError";
    this.kind = kind;
    this.cause = cause;
  }
}

/**
 * Type guard that narrows `unknown` errors to `DatabaseError`. Used
 * by Route Handlers to map repository failures onto HTTP responses
 * without resorting to `instanceof` checks scattered across files.
 */
export function isDatabaseError(error: unknown): error is DatabaseError {
  return error instanceof DatabaseError;
}