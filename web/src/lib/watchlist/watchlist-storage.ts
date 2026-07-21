/**
 * Watchlist persistence — the LocalStorage vocabulary and external store.
 *
 * Plain module (no "use client", no React) so the hook, and any future
 * server-side reconciliation, share the same key and serialization. The
 * watchlist is a set of pool addresses the visitor chose to keep an eye on;
 * it lives entirely in the browser (no account, no backend) until the v0.3
 * server-side watchlist replaces this store behind the same hook.
 *
 * Shaped as a `useSyncExternalStore` source: a cached snapshot with a stable
 * reference (so React can bail out of renders), a subscription that also
 * listens for cross-tab `storage` events, and a `getServerSnapshot` that
 * returns a stable empty list (the server can't read LocalStorage, so the
 * first client render must match "empty" then hydrate).
 */

export const WATCHLIST_STORAGE_KEY = "yog_watchlist";

/** Stable empty reference — returned on the server and when storage is empty
 *  or unreadable, so `useSyncExternalStore` never sees a changing snapshot. */
const EMPTY: readonly string[] = Object.freeze([]);

/** Cached client snapshot. `null` means "not read yet"; otherwise it is the
 *  current value with a reference that only changes when the list changes. */
let cache: readonly string[] | null = null;

const listeners = new Set<() => void>();

function readFromStorage(): readonly string[] {
  if (typeof window === "undefined") return EMPTY;
  try {
    const raw = window.localStorage.getItem(WATCHLIST_STORAGE_KEY);
    if (!raw) return EMPTY;
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return EMPTY;
    const cleaned = parsed.filter((x): x is string => typeof x === "string");
    return cleaned.length > 0 ? Object.freeze(cleaned) : EMPTY;
  } catch {
    // Malformed JSON or a locked/again-unavailable storage — treat as empty
    // rather than throwing into a render.
    return EMPTY;
  }
}

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

function onStorageEvent(event: StorageEvent): void {
  // Only react to our key (and to a full clear, `key === null`).
  if (event.key !== null && event.key !== WATCHLIST_STORAGE_KEY) return;
  cache = readFromStorage();
  emit();
}

/** `useSyncExternalStore` subscribe: local listeners + one shared cross-tab
 *  `storage` listener, installed lazily and torn down when the last
 *  subscriber leaves. */
export function subscribe(listener: () => void): () => void {
  if (listeners.size === 0 && typeof window !== "undefined") {
    window.addEventListener("storage", onStorageEvent);
  }
  listeners.add(listener);

  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && typeof window !== "undefined") {
      window.removeEventListener("storage", onStorageEvent);
    }
  };
}

/** Current client snapshot (stable reference between unchanged reads). */
export function getSnapshot(): readonly string[] {
  if (cache === null) {
    cache = readFromStorage();
  }
  return cache;
}

/** Server/first-paint snapshot — always the stable empty list. */
export function getServerSnapshot(): readonly string[] {
  return EMPTY;
}

/** Replace the watchlist, persist it, and notify subscribers in this tab
 *  (the `storage` event only reaches *other* tabs). */
export function setWatchlist(next: readonly string[]): void {
  cache = next.length > 0 ? Object.freeze([...next]) : EMPTY;
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(WATCHLIST_STORAGE_KEY, JSON.stringify(cache));
    } catch {
      // Storage full or unavailable (private mode): keep the in-memory cache
      // so the UI stays consistent for this session, just not persisted.
    }
  }
  emit();
}
