import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const KEY = `yog_01020304_${"ab".repeat(32)}`;
const TOKEN = `ylt_${"cd".repeat(32)}`;

type Seen = { url: string; authorization: string | null; body: unknown };
let seen: Seen[] = [];

beforeEach(() => {
  seen = [];
  vi.stubEnv("YOG_API_KEY", KEY);
  vi.stubEnv("NEXT_PUBLIC_YOG_API_URL", "http://api.test");
  vi.stubGlobal("fetch", async (url: string, init: RequestInit) => {
    seen.push({
      url,
      authorization: new Headers(init.headers).get("authorization"),
      body: JSON.parse(String(init.body)),
    });
    return Response.json({ accessToken: TOKEN, tokenType: "Bearer", expiresIn: 30 });
  });
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.resetModules();
});

async function post(body: unknown): Promise<Response> {
  const { POST } = await import("../api/live-token/route");
  return POST(new Request("http://web.test/api/live-token", { method: "POST", body: JSON.stringify(body) }));
}

describe("POST /api/live-token", () => {
  // Mutation: the answer passed on whole, or the key put in it — the browser would read it.
  it("asks the API with the dashboard's key and hands the page the token alone", async () => {
    const response = await post({ path: "/api/live", topics: ["signals"] });

    expect(seen).toEqual([
      {
        url: "http://api.test/api/live-tokens",
        authorization: `Bearer ${KEY}`,
        body: { path: "/api/live", topics: ["signals"] },
      },
    ]);
    const text = await response.text();
    expect(JSON.parse(text)).toEqual({ accessToken: TOKEN });
    expect(text).not.toContain(KEY);
    expect(response.headers.get("cache-control")).toBe("no-store");
  });

  // Mutation: any body forwarded — the key would sign for whatever a caller sends.
  it("refuses a body that is not { path, topics }, without asking the API", async () => {
    for (const body of [null, { path: "/api/live" }, { path: "/api/live", topics: ["signals"], ttl: 600 }]) {
      expect((await post(body)).status, JSON.stringify(body)).toBe(400);
    }
    expect(seen).toEqual([]);
  });
});
