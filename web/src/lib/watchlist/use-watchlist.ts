/**
 * `useWatchlist` — the client hook over the LocalStorage watchlist store.
 *
 * Backed by `useSyncExternalStore`, so it is SSR-safe (renders "empty" on the
 * server and the first client paint, then hydrates to the stored value without
 * a mismatch warning) and stays in sync across tabs and across every component
 * that uses it in the same tab.
 *
 * The v0.3 server-side watchlist can replace the store behind this hook
 * without touching the components that consume it.
 */

"use client";

import { useCallback, useSyncExternalStore } from "react";

import {
  getServerSnapshot,
  getSnapshot,
  setWatchlist,
  subscribe,
} from "./watchlist-storage";

export type UseWatchlist = {
  /** The watched pool addresses (order = insertion order kept by the array). */
  addresses: readonly string[];
  /** Whether a given pool address is on the watchlist. */
  has: (address: string) => boolean;
  /** Add the address if absent, remove it if present. */
  toggle: (address: string) => void;
  /** Remove an address (no-op if absent). */
  remove: (address: string) => void;
};

export function useWatchlist(): UseWatchlist {
  const addresses = useSyncExternalStore(
    subscribe,
    getSnapshot,
    getServerSnapshot,
  );

  const has = useCallback(
    (address: string) => addresses.includes(address),
    [addresses],
  );

  // Read the live snapshot inside the callbacks (not `addresses`) so they stay
  // referentially stable and never operate on a stale closure.
  const toggle = useCallback((address: string) => {
    const current = getSnapshot();
    setWatchlist(
      current.includes(address)
        ? current.filter((a) => a !== address)
        : [...current, address],
    );
  }, []);

  const remove = useCallback((address: string) => {
    const current = getSnapshot();
    if (current.includes(address)) {
      setWatchlist(current.filter((a) => a !== address));
    }
  }, []);

  return { addresses, has, toggle, remove };
}
