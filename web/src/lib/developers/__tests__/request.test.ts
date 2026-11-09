import { describe, expect, it } from "vitest";

import type { Endpoint } from "../contract";
import { byteSize, curlOf, missingOf, pageValues, urlOf } from "../request";

const endpoint = {
  path: "/api/tokens/{mint}/prices",
  params: [
    { name: "mint", in: "path", required: true },
    { name: "cursor", in: "query", required: false },
    { name: "limit", in: "query", required: false },
  ],
} as unknown as Endpoint;

describe("urlOf", () => {
  // Mutations: an empty query parameter kept; the path value not encoded.
  it("fills the path encoded and leaves empty parameters out", () => {
    expect(urlOf("https://api.example", endpoint, { mint: "a/b", cursor: "", limit: "2" })).toBe(
      "https://api.example/api/tokens/a%2Fb/prices?limit=2",
    );
  });

  it("keeps an empty path parameter named", () => {
    expect(urlOf("", endpoint, {})).toBe("/api/tokens/{mint}/prices");
  });
});

describe("curlOf", () => {
  // Mutations: the stream's curl without `-N`, which buffers; double quotes around the URL, where `!` expands.
  it("streams the events unbuffered, with the ticket, the URL single-quoted", () => {
    expect(curlOf("https://x/api/live?topics=signals", true, "GET")).toBe(
      `curl -N -H 'Accept: text/event-stream' -H "Authorization: Bearer $TICKET" 'https://x/api/live?topics=signals'`,
    );
  });

  // Mutation: the header single-quoted, so the copied command sends `$YOG_API_KEY` itself.
  it("sends the key from the shell's variable, double-quoted", () => {
    expect(curlOf("https://x/api/swaps", false, "GET")).toBe(
      `curl -s -H "Authorization: Bearer $YOG_API_KEY" 'https://x/api/swaps'`,
    );
  });

  // Mutation: the method left out, so the copied command sends a `GET` the route refuses.
  it("names a method other than GET", () => {
    expect(curlOf("https://x/api/auth/tickets", false, "POST")).toBe(
      `curl -s -X POST -H "Authorization: Bearer $YOG_API_KEY" 'https://x/api/auth/tickets'`,
    );
  });
});

describe("missingOf", () => {
  it("names the required parameters left empty", () => {
    expect(missingOf(endpoint, { limit: "2" })).toEqual(["mint"]);
    expect(missingOf(endpoint, { mint: "x" })).toEqual([]);
  });
});

describe("byteSize", () => {
  // Mutation: `text.length` — a non-ASCII body reads smaller than the wire carried.
  it("counts bytes, not characters", () => {
    expect(byteSize("é—")).toBe(5);
  });
});

describe("pageValues", () => {
  const signals = {
    path: "/api/signals",
    params: [
      { name: "dir", in: "query", required: false },
      { name: "position", in: "query", required: false },
      { name: "cursor", in: "query", required: false },
    ],
  } as unknown as Endpoint;

  // Mutations: `dir` left as it was — Previous fetches the page after; `position` kept — a 400.
  it("pages the way asked, without the position", () => {
    expect(pageValues(signals, { position: "last", limit: "2" }, "c", "prev")).toEqual({
      position: "",
      limit: "2",
      cursor: "c",
      dir: "prev",
    });
  });

  it("adds no parameter the route does not take", () => {
    expect(pageValues(endpoint, { mint: "m" }, "c", "next")).toEqual({ mint: "m", cursor: "c" });
  });
});
