"use client";

/**
 * The live tail of the signals, on the `signals` topic of the page's connection (`LiveProvider`),
 * over a list seeded by the server-rendered page. ⚠️ The stream carries no history, and a gap
 * left by a reopening stays: the signals of the past are another page's.
 */

import { useEffect, useState } from "react";

import { useLive } from "@/components/dashboard/live/live-provider";
import { zSignalResponse, type SignalResponse } from "@/lib/api/schema/zod.gen";
import { mergeSignals } from "@/lib/signals/merge-signals";

export function useLiveSignals(initial: readonly SignalResponse[]) {
  const { status, subscribe } = useLive();
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

  return { signals, status };
}
