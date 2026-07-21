/**
 * Unit tests for the LocalStorage watchlist store.
 *
 * The vitest env is plain Node (no `window`), so we install a minimal
 * `window.localStorage` stub and re-import the module fresh per test
 * (`vi.resetModules`) to reset its module-level snapshot cache.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const KEY = "yog_watchlist";

function installWindow(): Map<string, string> {
  const store = new Map<string, string>();
  (globalThis as unknown as { window: unknown }).window = {
    localStorage: {
      getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
      setItem: (k: string, v: string) => void store.set(k, v),
      removeItem: (k: string) => void store.delete(k),
    },
    addEventListener: () => {},
    removeEventListener: () => {},
  };
  return store;
}

let store: Map<string, string>;

beforeEach(() => {
  vi.resetModules();
  store = installWindow();
});

afterEach(() => {
  delete (globalThis as unknown as { window?: unknown }).window;
});

describe("watchlist-storage", () => {
  it("returns an empty list when nothing is stored", async () => {
    const m = await import("../watchlist-storage");
    expect(m.getSnapshot()).toEqual([]);
  });

  it("persists a write and reads it back", async () => {
    const m = await import("../watchlist-storage");
    m.setWatchlist(["a", "b"]);
    expect(m.getSnapshot()).toEqual(["a", "b"]);
    expect(JSON.parse(store.get(KEY)!)).toEqual(["a", "b"]);
  });

  it("returns a stable reference between unchanged reads", async () => {
    const m = await import("../watchlist-storage");
    expect(m.getSnapshot()).toBe(m.getSnapshot());
  });

  it("treats malformed JSON as empty", async () => {
    store.set(KEY, "{ not json");
    const m = await import("../watchlist-storage");
    expect(m.getSnapshot()).toEqual([]);
  });

  it("drops non-string entries", async () => {
    store.set(KEY, JSON.stringify(["a", 3, null, "b"]));
    const m = await import("../watchlist-storage");
    expect(m.getSnapshot()).toEqual(["a", "b"]);
  });

  it("notifies same-tab subscribers on write", async () => {
    const m = await import("../watchlist-storage");
    const listener = vi.fn();
    m.subscribe(listener);
    m.setWatchlist(["x"]);
    expect(listener).toHaveBeenCalledOnce();
  });

  it("getServerSnapshot is always the empty list", async () => {
    const m = await import("../watchlist-storage");
    m.setWatchlist(["a"]);
    expect(m.getServerSnapshot()).toEqual([]);
  });
});
