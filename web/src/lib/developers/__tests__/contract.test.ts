import { describe, expect, it } from "vitest";

import { RELEASES } from "@/components/marketing/changelog/releases";

import spec from "../../../../public/openapi.json";

import { FAMILIES, endpointsOf, familyOf, type Endpoint, type OpenApi } from "../contract";

const contract = spec as unknown as OpenApi;
const endpoints = endpointsOf(contract);

function endpoint(id: string): Endpoint {
  const found = endpoints.find((e) => e.id === id);
  if (!found) throw new Error(`no ${id}`);
  return found;
}

const operations = Object.values(contract.paths).flatMap((item) => Object.keys(item));

describe("endpointsOf", () => {
  // Mutation: a route filtered out — the page would hide it.
  it("shows every operation of the contract, once", () => {
    expect(endpoints).toHaveLength(operations.length);
    expect(new Set(endpoints.map((e) => e.id)).size).toBe(operations.length);
  });

  // Mutation: the contract's order kept — the page would open on a route mid-list.
  it("lists the routes family by family, the service's first", () => {
    const order = endpoints.map((e) => FAMILIES.indexOf(e.family));
    expect(order).toEqual([...order].sort((a, b) => a - b));
    expect(endpoints[0].path).toBe("/api/ingestion");
  });

  // Mutation: a segment taken out of the family table — the build fails rather than misfiling.
  it("files every route in one of the families", () => {
    for (const e of endpoints) expect(FAMILIES).toContain(e.family);
  });

  // Mutations: the nullability dropped; the array's items not followed.
  it("flattens a page's fields, items and nullability included", () => {
    const fields = new Map(endpoint("list_pool_swaps").fields.map((f) => [f.path, f]));
    expect(fields.get("items[].amountA")).toMatchObject({ type: "U128String", required: true });
    expect(fields.get("items[].at")?.type).toBe("date-time");
    expect(fields.get("items[].direction")?.type).toBe("enum");
    expect(fields.get("nextCursor")).toMatchObject({ type: "string | null", required: true });
    expect(fields.get("items[].meteoraDammV2")).toMatchObject({ type: "object", required: false });
  });

  // Mutation: a `oneOf [$ref, null]` read as the bare reference.
  it("reads a nullable reference as nullable", () => {
    const lock = endpoint("list_protocol_liquidity_moves").fields.find(
      (f) => f.path === "items[].lock",
    );
    expect(lock?.type).toBe("enum | null");
  });

  // Mutation: a union of objects left unexpanded — a change's protocol fields would not show.
  it("lists a union's fields, required only where every variant requires them", () => {
    const fields = new Map(endpoint("list_position_changes").fields.map((f) => [f.path, f]));
    expect(fields.get("items[].meteoraDammV2")?.type).toBe("one of");
    expect(fields.get("items[].meteoraDammV2.kind")?.required).toBe(true);
    expect(fields.get("items[].meteoraDammV2.vesting")?.required).toBe(false);
  });

  // Mutations: a path parameter listed after the query; the bounds of `limit` lost.
  it("reads the parameters, path first", () => {
    const params = endpoint("list_pool_swaps").params;
    expect(params[0]).toMatchObject({ name: "pool", in: "path", required: true });
    expect(params.find((p) => p.name === "window")?.enum).toEqual(["24h", "7d", "30d"]);
    expect(params.find((p) => p.name === "limit")).toMatchObject({
      type: "integer",
      min: 1,
      max: 200,
    });
  });

  it("marks the signal stream as server-sent events", () => {
    expect(endpoints.filter((e) => e.sse).map((e) => e.id)).toEqual(["stream_signals"]);
  });
});

describe("familyOf", () => {
  it("files a scoped route under its scope", () => {
    expect(familyOf("/api/pools/{pool}/swaps")).toBe("byPool");
    expect(familyOf("/api/swaps")).toBe("swaps");
  });

  // Mutation: a fallback family — an unknown route would land somewhere unnamed.
  it("refuses a route of no family", () => {
    expect(() => familyOf("/api/unknown")).toThrow("no family for /api/unknown");
  });
});

describe("the contract's version", () => {
  // Mutation: a release in the changelog without the API crate's version raised to it.
  it("is the latest release's", () => {
    expect(`v${contract.info.version}`).toBe(RELEASES[0]?.version);
  });
});
