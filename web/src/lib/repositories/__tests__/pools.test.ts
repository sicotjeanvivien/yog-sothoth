import { describe, expect, it } from "vitest";
import type { Sql } from "postgres";
import { listPools } from "../pools";
import { DatabaseError, isDatabaseError } from "@/lib/db/errors";

// Build a minimal `Sql` stand-in that satisfies the tagged-template
// call shape used by the repository. The real postgres.js client
// is a callable object with many extras, but `listPools` only uses
// it as a tag function — that is the surface we mock.
function makeSqlMock(
  resolve: () => Promise<unknown[]> | unknown[],
): Sql {
  const fn = (() => {
    const result = resolve();
    return Promise.resolve(result);
  }) as unknown as Sql;
  return fn;
}

function makeFailingSqlMock(error: unknown): Sql {
  const fn = (() => Promise.reject(error)) as unknown as Sql;
  return fn;
}

describe("listPools", () => {
  it("returns the camelCase mapping of every row", async () => {
    const sql = makeSqlMock(() => [
      {
        pool_address: "addr1",
        protocol: "damm_v2",
        token_a_mint: "mintA1",
        token_b_mint: "mintB1",
        first_seen_at: new Date("2026-04-01T00:00:00Z"),
        last_seen_at: new Date("2026-04-30T00:00:00Z"),
      },
      {
        pool_address: "addr2",
        protocol: "damm_v2",
        token_a_mint: "mintA2",
        token_b_mint: "mintB2",
        first_seen_at: new Date("2026-04-02T00:00:00Z"),
        last_seen_at: new Date("2026-04-29T00:00:00Z"),
      },
    ]);

    const pools = await listPools(sql);

    expect(pools).toHaveLength(2);
    expect(pools[0]?.poolAddress).toBe("addr1");
    expect(pools[1]?.poolAddress).toBe("addr2");
    expect(pools[0]?.firstSeenAt).toBe("2026-04-01T00:00:00.000Z");
  });

  it("returns an empty array when the database has no pools yet", async () => {
    const sql = makeSqlMock(() => []);
    const pools = await listPools(sql);
    expect(pools).toEqual([]);
  });

  it("wraps connection errors as DatabaseError(connection)", async () => {
    const econnrefused = Object.assign(new Error("connect ECONNREFUSED"), {
      code: "ECONNREFUSED",
    });
    const sql = makeFailingSqlMock(econnrefused);

    await expect(listPools(sql)).rejects.toSatisfy((error: unknown) => {
      return isDatabaseError(error) && error.kind === "connection";
    });
  });

  it("wraps authentication errors as DatabaseError(connection)", async () => {
    // SQLSTATE 28P01 = invalid_password
    const authErr = Object.assign(new Error("password authentication failed"), {
      code: "28P01",
    });
    const sql = makeFailingSqlMock(authErr);

    await expect(listPools(sql)).rejects.toSatisfy((error: unknown) => {
      return isDatabaseError(error) && error.kind === "connection";
    });
  });

  it("wraps generic SQL errors as DatabaseError(query)", async () => {
    // SQLSTATE 42501 = insufficient_privilege (e.g. SELECT denied)
    const permDenied = Object.assign(new Error("permission denied"), {
      code: "42501",
    });
    const sql = makeFailingSqlMock(permDenied);

    await expect(listPools(sql)).rejects.toSatisfy((error: unknown) => {
      return isDatabaseError(error) && error.kind === "query";
    });
  });

  it("wraps schema-mismatch rows as DatabaseError(validation)", async () => {
    // Missing required `protocol` column on this row.
    const sql = makeSqlMock(() => [
      {
        pool_address: "addr1",
        token_a_mint: "mintA1",
        token_b_mint: "mintB1",
        first_seen_at: new Date("2026-04-01T00:00:00Z"),
        last_seen_at: new Date("2026-04-30T00:00:00Z"),
      },
    ]);

    await expect(listPools(sql)).rejects.toSatisfy((error: unknown) => {
      return isDatabaseError(error) && error.kind === "validation";
    });
  });

  it("preserves an upstream DatabaseError instead of re-wrapping it", async () => {
    const original = new DatabaseError("connection", "boom");
    const sql = makeFailingSqlMock(original);

    try {
      await listPools(sql);
      throw new Error("listPools should have thrown");
    } catch (caught) {
      expect(caught).toBe(original);
    }
  });
});