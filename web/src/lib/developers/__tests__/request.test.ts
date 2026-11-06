import { describe, expect, it } from "vitest";

import type { Endpoint } from "../contract";
import { byteSize, curlOf, missingOf, pageValues, topicsOf, urlOf } from "../request";

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

describe("topicsOf", () => {
  // Mutation: no topic read — the live panel listens to nothing, its events all named.
  it("reads the topics a live URL asks for, encoded or not", () => {
    expect(topicsOf("https://x/api/live?topics=signals%2Cliquidity-changes")).toEqual([
      "signals",
      "liquidity-changes",
    ]);
    expect(topicsOf("https://x/api/live/protocols/meteora_damm_v2?topics=signals")).toEqual([
      "signals",
    ]);
    expect(topicsOf("https://x/api/live")).toEqual([]);
  });
});

describe("curlOf", () => {
  const rest = { sse: false, body: null, keyed: false };

  // Mutations: the stream's curl without `-N`, which buffers, or without its token.
  it("streams the events unbuffered, with a live token, the URL single-quoted", () => {
    expect(curlOf("https://x/api/live?topics=signals", { ...rest, sse: true })).toBe(
      `curl -N -H 'Accept: text/event-stream' -H "Authorization: Bearer $YOG_LIVE_TOKEN" 'https://x/api/live?topics=signals'`,
    );
  });

  // Mutation: the key written into the command — copied, it would land in a shell history or a bug report.
  it("names the key by a variable, and only when one was given", () => {
    expect(curlOf("https://x/api/swaps", rest)).toBe("curl -s 'https://x/api/swaps'");
    expect(curlOf("https://x/api/swaps", { ...rest, keyed: true })).toBe(
      `curl -s -H "Authorization: Bearer $YOG_API_KEY" 'https://x/api/swaps'`,
    );
  });

  // Mutation: the body left out, or quoted so that the shell reads into it.
  it("POSTs the body, compacted and single-quoted, with the key it needs", () => {
    expect(curlOf("https://x/api/live-tokens", { ...rest, body: '{\n  "path": "/api/live"\n}' })).toBe(
      `curl -s -X POST -H "Authorization: Bearer $YOG_API_KEY" -H 'Content-Type: application/json' -d '{"path":"/api/live"}' 'https://x/api/live-tokens'`,
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
