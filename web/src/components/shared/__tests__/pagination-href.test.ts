/**
 * `buildHref`, which carries the pagination's correctness: namespacing, preservation of
 * unrelated params, and `cursor`/`dir` exclusive of `position`.
 */

import { describe, expect, it } from "vitest";

import { buildHref } from "../pagination-href";

/** The pathname and the params of a href, order-free. */
function parse(href: string): {
  pathname: string;
  params: Record<string, string[]>;
} {
  const [pathname, query = ""] = href.split("?");
  const sp = new URLSearchParams(query);
  const params: Record<string, string[]> = {};
  for (const [k, v] of sp.entries()) {
    (params[k] ??= []).push(v);
  }
  return { pathname: pathname ?? "", params };
}

describe("buildHref — single pagination (no prefix)", () => {
  it("returns the base path when no target params are set", () => {
    const href = buildHref("/pools", {}, "", {});
    expect(href).toBe("/pools");
  });

  it("sets cursor and dir when navigating to next", () => {
    const href = buildHref("/pools", {}, "", {
      cursor: "abc",
      dir: "next",
    });
    const { pathname, params } = parse(href);
    expect(pathname).toBe("/pools");
    expect(params).toEqual({ cursor: ["abc"], dir: ["next"] });
  });

  it("sets cursor and dir when navigating to prev", () => {
    const href = buildHref("/pools", {}, "", {
      cursor: "xyz",
      dir: "prev",
    });
    expect(parse(href).params).toEqual({ cursor: ["xyz"], dir: ["prev"] });
  });

  it("sets only position when jumping to first", () => {
    const href = buildHref("/pools", {}, "", { position: "first" });
    expect(parse(href).params).toEqual({ position: ["first"] });
  });

  it("sets only position when jumping to last", () => {
    const href = buildHref("/pools", {}, "", { position: "last" });
    expect(parse(href).params).toEqual({ position: ["last"] });
  });

  it("clears existing cursor and dir when jumping to a position", () => {
    const href = buildHref(
      "/pools",
      { cursor: "stale-cursor", dir: "next" },
      "",
      { position: "last" },
    );
    const { params } = parse(href);
    expect(params['cursor']).toBeUndefined();
    expect(params['dir']).toBeUndefined();
    expect(params['position']).toEqual(["last"]);
  });

  it("clears existing position when navigating to next", () => {
    const href = buildHref(
      "/pools",
      { position: "last" },
      "",
      { cursor: "abc", dir: "next" },
    );
    const { params } = parse(href);
    expect(params['position']).toBeUndefined();
    expect(params['cursor']).toEqual(["abc"]);
    expect(params['dir']).toEqual(["next"]);
  });
});

describe("buildHref — namespaced pagination", () => {
  it("camelCases the prefix into the param keys", () => {
    const href = buildHref("/pools/X", {}, "swaps", {
      cursor: "abc",
      dir: "next",
    });
    const { params } = parse(href);
    expect(params).toEqual({
      swapsCursor: ["abc"],
      swapsDir: ["next"],
    });
  });

  it("uses the prefix verbatim when building the position key", () => {
    const href = buildHref("/pools/X", {}, "liq", {
      position: "last",
    });
    expect(parse(href).params).toEqual({ liqPosition: ["last"] });
  });

  it("clears its own namespace when jumping to a position", () => {
    const href = buildHref(
      "/pools/X",
      { swapsCursor: "stale", swapsDir: "next" },
      "swaps",
      { position: "first" },
    );
    const { params } = parse(href);
    expect(params['swapsCursor']).toBeUndefined();
    expect(params['swapsDir']).toBeUndefined();
    expect(params['swapsPosition']).toEqual(["first"]);
  });

  it("does not touch other namespaces", () => {
    const href = buildHref(
      "/pools/X",
      { liqCursor: "Y", liqDir: "next" },
      "swaps",
      { cursor: "abc", dir: "next" },
    );
    const { params } = parse(href);
    expect(params['liqCursor']).toEqual(["Y"]);
    expect(params['liqDir']).toEqual(["next"]);
    expect(params['swapsCursor']).toEqual(["abc"]);
    expect(params['swapsDir']).toEqual(["next"]);
  });

  it("does not clear the unprefixed `cursor` when prefixed", () => {
    const href = buildHref(
      "/pools/X",
      { cursor: "global-cursor" },
      "swaps",
      { cursor: "swaps-cursor", dir: "next" },
    );
    const { params } = parse(href);
    expect(params['cursor']).toEqual(["global-cursor"]);
    expect(params['swapsCursor']).toEqual(["swaps-cursor"]);
  });
});

describe("buildHref — preservation of unrelated params", () => {
  it("preserves arbitrary unrelated string params", () => {
    const href = buildHref(
      "/pools",
      { q: "BONK", sort: "tvl_desc" },
      "",
      { cursor: "abc", dir: "next" },
    );
    const { params } = parse(href);
    expect(params['q']).toEqual(["BONK"]);
    expect(params['sort']).toEqual(["tvl_desc"]);
    expect(params['cursor']).toEqual(["abc"]);
  });

  it("preserves all paginations not owned by the current prefix", () => {
    const before = {
      swapsCursor: "S",
      swapsDir: "next",
      liqCursor: "L",
      liqDir: "next",
    };
    const href = buildHref("/pools/X", before, "swaps", {
      cursor: "S2",
      dir: "next",
    });
    const { params } = parse(href);
    expect(params['swapsCursor']).toEqual(["S2"]);
    expect(params['swapsDir']).toEqual(["next"]);
    expect(params['liqCursor']).toEqual(["L"]);
    expect(params['liqDir']).toEqual(["next"]);
  });

  it("preserves arrays with multiple values", () => {
    const href = buildHref(
      "/pools",
      { tag: ["new", "watched"] },
      "",
      { cursor: "abc", dir: "next" },
    );
    const { params } = parse(href);
    expect(params['tag']).toEqual(["new", "watched"]);
  });
});

describe("buildHref — edge cases", () => {
  it("returns the base path when search params are all undefined", () => {
    const href = buildHref(
      "/pools",
      { cursor: undefined, dir: undefined },
      "",
      {},
    );
    expect(href).toBe("/pools");
  });

  it("skips undefined entries in incoming search params", () => {
    // Mutation: an undefined value written as "key=undefined".
    const href = buildHref(
      "/pools",
      { q: undefined, sort: "tvl_desc" },
      "",
      { cursor: "abc", dir: "next" },
    );
    const { params } = parse(href);
    expect(params['q']).toBeUndefined();
    expect(params['sort']).toEqual(["tvl_desc"]);
    expect(params['cursor']).toEqual(["abc"]);
  });

  it("preserves the base path verbatim including subpaths", () => {
    const href = buildHref(
      "/pools/8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie",
      {},
      "swaps",
      { cursor: "abc", dir: "next" },
    );
    expect(parse(href).pathname).toBe(
      "/pools/8Pm2kZpnxD3hoMmt4bjStX2Pw2Z9abpbHzZxMPqxPmie",
    );
  });

  it("does not include `?` when no params are set", () => {
    const href = buildHref("/pools", { q: undefined }, "", {});
    expect(href).toBe("/pools");
    expect(href.includes("?")).toBe(false);
  });
});