"use client";

/**
 * The live tail of the signal feed: one `EventSource` on `/api/signals/stream`, over a list
 * seeded by the server-rendered page. ⚠️ The stream carries no history: after a drop, the gap
 * is refilled from `GET /api/signals` and merged by `id`. A malformed event is logged, skipped.
 */

import { useEffect, useRef, useState } from "react";

import { fetchSignalsBrowser } from "@/lib/api/browser/signals";
import { loadClientEnv } from "@/lib/config/client-env.schema";
import { zSignalResponse, type SignalResponse } from "@/lib/api/schema/zod.gen";
import { mergeSignals } from "@/lib/signals/merge-signals";

export type StreamStatus = "connecting" | "live" | "reconnecting";

export function useSignalStream(initial: readonly SignalResponse[]) {
  const [signals, setSignals] = useState<readonly SignalResponse[]>(initial);
  const [status, setStatus] = useState<StreamStatus>("connecting");

  // Tells a reopen, with a gap to refill, from the first open, whose SSR page is fresh.
  const droppedRef = useRef(false);

  useEffect(() => {
    const { NEXT_PUBLIC_YOG_API_URL } = loadClientEnv();
    const source = new EventSource(
      `${NEXT_PUBLIC_YOG_API_URL}/api/signals/stream`,
    );

    source.onopen = () => {
      setStatus("live");
      if (droppedRef.current) {
        droppedRef.current = false;
        fetchSignalsBrowser()
          .then((page) => {
            setSignals((current) => mergeSignals(current, page.items));
          })
          .catch((err) => {
            // The gap stays until the next reconnect, rather than tearing the page down.
            console.warn("signal feed refill failed", err);
          });
      }
    };

    source.onerror = () => {
      // EventSource reconnects on its own.
      setStatus("reconnecting");
      droppedRef.current = true;
    };

    source.onmessage = (event) => {
      let raw: unknown;
      try {
        raw = JSON.parse(event.data as string);
      } catch {
        console.warn("signal stream: unparseable event skipped", event.data);
        return;
      }
      const parsed = zSignalResponse.safeParse(raw);
      if (!parsed.success) {
        console.warn("signal stream: invalid event skipped", parsed.error);
        return;
      }
      setSignals((current) => mergeSignals(current, [parsed.data]));
    };

    return () => source.close();
  }, []);

  return { signals, status };
}
