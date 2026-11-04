"use client";

/**
 * The live tail of the signals, on the `signals` topic of the page's connection (`LiveProvider`),
 * over a list seeded by the server-rendered page. ⚠️ The stream carries no history: after a
 * reopening, the gap is refilled from the collection of the same scope and merged by `id`.
 */

import { useEffect, useState } from "react";

import { useLive } from "@/components/dashboard/live/live-provider";
import { fetchSignalsBrowser } from "@/lib/api/browser/signals";
import { zSignalResponse, type SignalResponse } from "@/lib/api/schema/zod.gen";
import { mergeSignals } from "@/lib/signals/merge-signals";

export function useLiveSignals(initial: readonly SignalResponse[]) {
  const { protocol, status, subscribe, onReopen } = useLive();
  const [signals, setSignals] = useState<readonly SignalResponse[]>(initial);

  useEffect(
    () =>
      subscribe("signals", (raw) => {
        const parsed = zSignalResponse.safeParse(raw);
        if (!parsed.success) {
          console.warn("signal stream: invalid event skipped", parsed.error);
          return;
        }
        setSignals((current) => mergeSignals(current, [parsed.data]));
      }),
    [subscribe],
  );

  useEffect(
    () =>
      onReopen(() => {
        fetchSignalsBrowser(protocol)
          .then((page) => {
            setSignals((current) => mergeSignals(current, page.items));
          })
          .catch((err) => {
            // The gap stays until the next reopening, rather than tearing the page down.
            console.warn("signal feed refill failed", err);
          });
      }),
    [onReopen, protocol],
  );

  return { signals, status };
}
