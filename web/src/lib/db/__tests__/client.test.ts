import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Holder shape mirroring the one declared in `../client.ts`. We
// don't import it from there to keep this test file independent
// of internal module shape changes.
type GlobalSqlHolder = {
  __yogSothothSql?: unknown;
};

// Property-based shape of the error we expect to catch. We do NOT
// import `DatabaseError` from `../errors` and use `instanceof`
// because `vi.resetModules()` re-evaluates the whole import graph
// of the module under test on every iteration. That re-evaluation
// builds a *new* `DatabaseError` class for each `await import(...)`,
// so any error caught from the freshly imported module is an
// instance of a class distinct from the one statically imported at
// the top of this file. `instanceof` then returns false even though
// the value is, semantically, the right error.
//
// Asserting on observable properties (`name`, `kind`) sidesteps the
// class-identity trap entirely and stays robust whatever the test
// runner does with module caches.
type DatabaseErrorLike = Error & { kind: string };

function isDatabaseErrorLike(value: unknown): value is DatabaseErrorLike {
  return (
    value instanceof Error &&
    value.name === "DatabaseError" &&
    typeof (value as { kind?: unknown }).kind === "string"
  );
}

describe("getSql (config-time errors)", () => {
  // The client module evaluates `process.env.DATABASE_URL` lazily,
  // inside `getSql`. We can therefore mutate the env between
  // `vi.resetModules()` calls and re-import to exercise different
  // paths without spawning real connections.
  //
  // The singleton is also stored on `globalThis` outside production,
  // and `vi.resetModules()` does NOT clear `globalThis`. We have to
  // delete the holder by hand before each test so that the second
  // `getSql()` call evaluates `DATABASE_URL` instead of returning a
  // cached client created during a prior test.
  const originalEnv = { ...process.env };

  beforeEach(() => {
    vi.resetModules();
    delete (globalThis as GlobalSqlHolder).__yogSothothSql;
  });

  afterEach(() => {
    process.env = { ...originalEnv };
    delete (globalThis as GlobalSqlHolder).__yogSothothSql;
  });

  it("throws DatabaseError(connection) when DATABASE_URL is missing", async () => {
    delete process.env.DATABASE_URL;

    const mod = await import("../client");

    let caught: unknown;
    try {
      mod.getSql();
    } catch (err) {
      caught = err;
    }

    expect(isDatabaseErrorLike(caught)).toBe(true);
    if (isDatabaseErrorLike(caught)) {
      expect(caught.kind).toBe("connection");
    }
  });

  it("throws DatabaseError(connection) when DATABASE_URL is empty", async () => {
    process.env.DATABASE_URL = "";

    const mod = await import("../client");

    let caught: unknown;
    try {
      mod.getSql();
    } catch (err) {
      caught = err;
    }

    expect(isDatabaseErrorLike(caught)).toBe(true);
    if (isDatabaseErrorLike(caught)) {
      expect(caught.kind).toBe("connection");
    }
  });
});