"use client";

/**
 * The live tail of the signals, every protocol's or one protocol's: one `EventSource` on the
 * `signals` topic of `/api/live`, over a list seeded by the server-rendered page. ⚠️ The stream
 * carries no history: after a drop, the gap is refilled from the collection and merged by `id`.
 */

import { useEffect, useRef, useState } from "react";

import { fetchSignalsBrowser } from "@/lib/api/browser/signals";
import { loadClientEnv } from "@/lib/config/client-env.schema";
import {
  zSignalResponse,
  type ProtocolResponse,
  type SignalResponse,
} from "@/lib/api/schema/zod.gen";
import { mergeSignals } from "@/lib/signals/merge-signals";
import { liveSignalsPath } from "@/lib/signals/signal-paths";

export type StreamStatus = "connecting" | "live" | "reconnecting";

export function useLiveSignals(initial: readonly SignalResponse[], protocol?: ProtocolResponse) {
  const [signals, setSignals] = useState<readonly SignalResponse[]>(initial);
  const [status, setStatus] = useState<StreamStatus>("connecting");

  // Tells a reopen, with a gap to refill, from the first open, whose SSR page is fresh.
  const droppedRef = useRef(false);

  useEffect(() => {
    const { NEXT_PUBLIC_YOG_API_URL } = loadClientEnv();
    const source = new EventSource(`${NEXT_PUBLIC_YOG_API_URL}${liveSignalsPath(protocol)}`);

    source.onopen = () => {
      setStatus("live");
      if (droppedRef.current) {
        droppedRef.current = false;
        fetchSignalsBrowser(protocol)
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

    // ⚠️ A named event: `onmessage` only receives the unnamed ones.
    source.addEventListener("signals", (event) => {
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
    });

    return () => source.close();
  }, [protocol]);

  return { signals, status };
}
