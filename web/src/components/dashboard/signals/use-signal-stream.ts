"use client";

/**
 * Live tail of the signal feed over SSE.
 *
 * Owns one `EventSource` on `GET /api/signals/stream` (direct to the
 * public gateway — no proxy) for the lifetime of the component, and a
 * signal list seeded by the server-rendered first page.
 *
 * # Contract with the stream
 *
 * The stream only carries signals born after it (re)opened — never
 * history. So on every reopen *after a drop*, the hook refills the gap
 * from `GET /api/signals` and reconciles by `id` (`mergeSignals`); the
 * browser's `EventSource` handles the reconnection itself.
 *
 * A malformed event is logged and skipped — a drifting wire contract
 * must not crash the page (the zod issue pinpoints the drift).
 */

import { useEffect, useRef, useState } from "react";

import { fetchSignalsBrowser } from "@/lib/api/browser/signals";
import { loadClientEnv } from "@/lib/config/client-env.schema";
import { SignalSchema, type SignalResponse } from "@/lib/api/schema/signal";
import { mergeSignals } from "@/lib/signals/merge-signals";

/** Connection state surfaced to the UI. */
export type StreamStatus = "connecting" | "live" | "reconnecting";

export function useSignalStream(initial: readonly SignalResponse[]) {
  const [signals, setSignals] = useState<readonly SignalResponse[]>(initial);
  const [status, setStatus] = useState<StreamStatus>("connecting");

  // Set on error, consumed on the next open: distinguishes a reopen
  // (gap to refill) from the very first open (the SSR page is fresh).
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
        // Refill whatever was born during the outage; ids dedup the
        // overlap with what the stream already delivered.
        fetchSignalsBrowser()
          .then((page) => {
            setSignals((current) => mergeSignals(current, page.items));
          })
          .catch((err) => {
            // The stream is live again — the gap stays until the next
            // reconnect, which is better than tearing the page down.
            console.warn("signal feed refill failed", err);
          });
      }
    };

    source.onerror = () => {
      // EventSource reconnects on its own; just reflect the state.
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
      const parsed = SignalSchema.safeParse(raw);
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
