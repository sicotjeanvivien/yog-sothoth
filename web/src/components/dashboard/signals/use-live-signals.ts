"use client";

/**
 * The live tail of the signals, on the `signals` topic of the page's connection (`LiveProvider`),
 * over a list seeded by the server-rendered page. ⚠️ The stream carries no history: a reopening
 * adds a gap row where events may have been missed.
 */

import { useEffect, useState } from "react";

import { useLive } from "@/components/dashboard/live/live-provider";
import { zSignalResponse, type SignalResponse } from "@/lib/api/schema/zod.gen";
import { markSignalGap, mergeSignals, type SignalFeedRow } from "@/lib/signals/merge-signals";

export function useLiveSignals(initial: readonly SignalResponse[]) {
  const { status, subscribe, onReopen } = useLive();
  const [rows, setRows] = useState<readonly SignalFeedRow[]>(() => mergeSignals([], initial));

  useEffect(
    () =>
      subscribe("signals", (raw) => {
        const parsed = zSignalResponse.safeParse(raw);
        if (!parsed.success) {
          console.warn("signal stream: invalid event skipped", parsed.error);
          return;
        }
        setRows((current) => mergeSignals(current, [parsed.data]));
      }),
    [subscribe],
  );

  useEffect(
    () => onReopen(() => setRows((current) => markSignalGap(current, new Date().toISOString()))),
    [onReopen],
  );

  return { rows, status };
}
