import { afterEach, describe, expect, it, vi } from "vitest";
import * as z from "zod";

import { ApiClientError } from "../../errors";
import { apiGetWithUrl, apiPostWithUrl } from "../client-core";

const schema = z.object({ ticket: z.string() });

afterEach(() => {
  vi.unstubAllGlobals();
});

function stubFetch(response: Response) {
  const fetched = vi.fn(async () => response);
  vi.stubGlobal("fetch", fetched);
  return fetched;
}

describe("apiPostWithUrl", () => {
  // Mutations: a GET sent, or the caller's headers dropped — the key never reaches the API.
  it("POSTs with the headers it is given", async () => {
    const fetched = stubFetch(Response.json({ ticket: "yogt_1" }));
    const answer = await apiPostWithUrl("http://api.test/api/auth/tickets", schema, 1_000, {
      Authorization: "Bearer yog_key",
    });

    expect(answer).toEqual({ ticket: "yogt_1" });
    expect(fetched).toHaveBeenCalledWith(
      "http://api.test/api/auth/tickets",
      expect.objectContaining({
        method: "POST",
        headers: { Accept: "application/json", Authorization: "Bearer yog_key" },
      }),
    );
  });

  // Mutation: a non-2xx parsed as a body — a refused key would read as a validation failure.
  it("throws the API's refusal as an http error with its detail", async () => {
    stubFetch(
      Response.json(
        { type: "about:blank", title: "Unauthorized", status: 401, detail: "a valid API key is required" },
        { status: 401 },
      ),
    );
    const call = apiPostWithUrl("http://api.test/api/auth/tickets", schema, 1_000);
    await expect(call).rejects.toBeInstanceOf(ApiClientError);
    await expect(call).rejects.toMatchObject({
      details: { kind: "http", status: 401, remoteMessage: "a valid API key is required" },
    });
  });
});

describe("apiGetWithUrl", () => {
  // Mutation: the caller's headers dropped on a GET — every server-side read goes anonymous.
  it("sends the headers it is given", async () => {
    const fetched = stubFetch(Response.json({ ticket: "x" }));
    await apiGetWithUrl("http://api.test/api/signals", schema, 1_000, { Authorization: "Bearer yog_key" });
    expect(fetched).toHaveBeenCalledWith(
      "http://api.test/api/signals",
      expect.objectContaining({
        method: "GET",
        headers: { Accept: "application/json", Authorization: "Bearer yog_key" },
      }),
    );
  });
});
