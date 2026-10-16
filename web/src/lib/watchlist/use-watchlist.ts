/**
 * The client hook over the LocalStorage watchlist, through `useSyncExternalStore`: empty on
 * the server and the first paint, then in sync across tabs and components.
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
  has: (address: string) => boolean;
  toggle: (address: string) => void;
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

  // The live snapshot, not `addresses`: stable callbacks, no stale closure.
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
