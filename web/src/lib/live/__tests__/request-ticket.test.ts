import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { __resetServerEnv } from "@/lib/config/server-env.schema";

import { requestTicket } from "../request-ticket";

const KEY = `yog_0badc0de_${"a".repeat(64)}`;

beforeEach(() => {
  __resetServerEnv();
  vi.stubEnv("YOG_API_URL", "http://api.test");
  vi.stubEnv("YOG_API_KEY", KEY);
});

afterEach(() => {
  __resetServerEnv();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

// Mutations: the key left out, another route, a GET — the API answers 401, 404 or 405.
it("trades the dashboard's key for a ticket at POST /api/auth/tickets", async () => {
  const fetched = vi.fn(async () =>
    Response.json({ ticket: "yogt_abc", expiresAt: "2026-11-09T12:00:30Z" }),
  );
  vi.stubGlobal("fetch", fetched);

  expect(await requestTicket()).toBe("yogt_abc");
  expect(fetched).toHaveBeenCalledWith(
    "http://api.test/api/auth/tickets",
    expect.objectContaining({
      method: "POST",
      headers: expect.objectContaining({ Authorization: `Bearer ${KEY}` }),
    }),
  );
});

// Mutation: a refusal swallowed into an empty ticket — the live connection opens with nothing.
it("throws when the API refuses the key", async () => {
  vi.stubGlobal("fetch", vi.fn(async () => Response.json({ status: 401 }, { status: 401 })));
  await expect(requestTicket()).rejects.toThrow(/HTTP 401/);
});
