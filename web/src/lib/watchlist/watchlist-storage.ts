/**
 * The watchlist in LocalStorage, as a `useSyncExternalStore` source. ⚠️ `getServerSnapshot`
 * is empty and the cached snapshot keeps a stable reference: a new array per read loops React.
 */

export const WATCHLIST_STORAGE_KEY = "yog_watchlist";

const EMPTY: readonly string[] = Object.freeze([]);

/** `null` until read; its reference changes only with the list. */
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
    // Malformed JSON or unavailable storage: empty, not a throw into a render.
    return EMPTY;
  }
}

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

function onStorageEvent(event: StorageEvent): void {
  // `key === null` is a full clear.
  if (event.key !== null && event.key !== WATCHLIST_STORAGE_KEY) return;
  cache = readFromStorage();
  emit();
}

/** One shared cross-tab `storage` listener, installed lazily, removed with the last subscriber. */
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

export function getSnapshot(): readonly string[] {
  if (cache === null) {
    cache = readFromStorage();
  }
  return cache;
}

export function getServerSnapshot(): readonly string[] {
  return EMPTY;
}

/** ⚠️ Notifies this tab itself: the `storage` event reaches only the others. */
export function setWatchlist(next: readonly string[]): void {
  cache = next.length > 0 ? Object.freeze([...next]) : EMPTY;
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(WATCHLIST_STORAGE_KEY, JSON.stringify(cache));
    } catch {
      // Storage full or unavailable: the in-memory cache keeps the session consistent.
    }
  }
  emit();
}
