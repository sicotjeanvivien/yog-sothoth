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
});
