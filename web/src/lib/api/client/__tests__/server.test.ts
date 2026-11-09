import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as z from "zod";

import { __resetServerEnv } from "@/lib/config/server-env.schema";

import { apiGet, apiPost } from "../server";

const KEY = `yog_0badc0de_${"a".repeat(64)}`;
const schema = z.object({});

let fetched: ReturnType<typeof vi.fn>;

beforeEach(() => {
  __resetServerEnv();
  vi.stubEnv("YOG_API_URL", "http://api.test");
  vi.stubEnv("YOG_API_KEY", KEY);
  fetched = vi.fn(async () => Response.json({}));
  vi.stubGlobal("fetch", fetched);
});

afterEach(() => {
  __resetServerEnv();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

// Mutations: the key left out of either call — every server-side read, or the ticket, goes anonymous.
it.each([
  ["apiGet", () => apiGet("/api/signals", { limit: 5 }, schema), "http://api.test/api/signals?limit=5"],
  ["apiPost", () => apiPost("/api/auth/tickets", schema), "http://api.test/api/auth/tickets"],
])("%s calls YOG_API_URL with the dashboard's key", async (_, call, url) => {
  await call();
  expect(fetched).toHaveBeenCalledWith(
    url,
    expect.objectContaining({ headers: expect.objectContaining({ Authorization: `Bearer ${KEY}` }) }),
  );
});
