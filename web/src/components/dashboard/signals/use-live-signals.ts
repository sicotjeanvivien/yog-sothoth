"use client";

/**
 * The signals that arrive on the `signals` topic of the page's connection (`LiveProvider`), from
 * an empty list. ⚠️ The stream carries no history: a reopening adds a gap row where events may
 * have been missed.
 */

import { useEffect, useState } from "react";

import { useLive } from "@/components/dashboard/live/live-provider";
import { zSignalResponse } from "@/lib/api/schema/zod.gen";
import { markSignalGap, mergeSignals, type SignalFeedRow } from "@/lib/signals/merge-signals";

export function useLiveSignals() {
  const { status, subscribe, onReopen } = useLive();
  const [rows, setRows] = useState<readonly SignalFeedRow[]>([]);

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
