import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { send } from "../endpoint-detail";

describe("send", () => {
  beforeEach(() => {
    vi.stubEnv("NEXT_PUBLIC_YOG_API_URL", "https://api.example");
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  // Mutation: the method left out of `fetch`, which then sends a `GET` the route refuses.
  it("sends the method the route declares", async () => {
    const fetched = vi.fn(async () => new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetched);

    await send("https://api.example/api/auth/tickets", "POST");

    expect(fetched).toHaveBeenCalledWith(
      "https://api.example/api/auth/tickets",
      expect.objectContaining({ method: "POST" }),
    );
  });

  // Mutation: the key left out — every route answers 401 once the API is closed.
  it("sends the visitor's key when there is one", async () => {
    const fetched = vi.fn(async () => new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetched);

    await send("https://api.example/api/signals", "GET", "yog_visitor");

    expect(fetched).toHaveBeenCalledWith(
      "https://api.example/api/signals",
      expect.objectContaining({
        headers: { Accept: "application/json", Authorization: "Bearer yog_visitor" },
      }),
    );
  });

  // Mutation: an empty `Bearer ` sent — the API reads a malformed key, not a missing one.
  it("sends no Authorization header without a key", async () => {
    const fetched = vi.fn(async () => new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetched);

    await send("https://api.example/api/signals", "GET");

    expect(fetched).toHaveBeenCalledWith(
      "https://api.example/api/signals",
      expect.objectContaining({ headers: { Accept: "application/json" } }),
    );
  });
});
