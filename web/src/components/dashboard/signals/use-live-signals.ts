"use client";

/**
 * The live tail of the signals, on the `signals` topic of the page's connection (`LiveProvider`),
 * over a list seeded by the server-rendered page. ⚠️ The stream carries no history: a reopening
 * leaves a gap, kept as its instant so that the feed says where it resumed (`withGaps`).
 */

import { useEffect, useState } from "react";

import { useLive } from "@/components/dashboard/live/live-provider";
import { zSignalResponse, type SignalResponse } from "@/lib/api/schema/zod.gen";
import { mergeSignals } from "@/lib/signals/merge-signals";

/** The reopenings remembered: older ones fall below the signals a feed shows. */
const MAX_GAPS = 20;

export function useLiveSignals(initial: readonly SignalResponse[]) {
  const { status, subscribe, onReopen } = useLive();
  const [signals, setSignals] = useState<readonly SignalResponse[]>(initial);
  const [gaps, setGaps] = useState<readonly string[]>([]);

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
    () => onReopen(() => setGaps((current) => [new Date().toISOString(), ...current].slice(0, MAX_GAPS))),
    [onReopen],
  );

  return { signals, gaps, status };
}
