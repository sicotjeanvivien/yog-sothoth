"use client";

/**
 * A fetch polled every `POLL_INTERVAL_MS`; a tick is skipped while the last is in flight. A
 * failure reads `error`, never a silent stale value.
 */

import { useEffect, useRef, useState } from "react";

import { ApiClientError } from "@/lib/api/errors";

const POLL_INTERVAL_MS = 10_000;

export type Half<T> =
  | { phase: "loading" }
  | { phase: "ready"; data: T }
  | { phase: "error" };

const LOADING = { phase: "loading" } as const;

export function usePolled<T>(fetcher: () => Promise<T>): Half<T> {
  const [state, setState] = useState<Half<T>>(LOADING);
  const inFlight = useRef(false);

  useEffect(() => {
    const load = async () => {
      if (inFlight.current) return;
      inFlight.current = true;
      try {
        setState({ phase: "ready", data: await fetcher() });
      } catch (err) {
        if (!(err instanceof ApiClientError)) {
          console.error("usePolled: unexpected error", err);
        }
        setState({ phase: "error" });
      } finally {
        inFlight.current = false;
      }
    };
    const timer = setInterval(() => void load(), POLL_INTERVAL_MS);
    setTimeout(() => void load(), 0);
    return () => clearInterval(timer);
  }, [fetcher]);

  return state;
}
