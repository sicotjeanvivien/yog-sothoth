/**
 * The page's one `EventSource` on `/api/live`, reopened after a refusal. ⚠️ Nothing is replayed:
 * every opening that follows a loss leaves a gap, and `onReopen` fires.
 */

import type { LiveTopic } from "./live-topics";

export type StreamStatus = "connecting" | "live" | "reconnecting";

export type LiveHandlers = {
  onStatus: (status: StreamStatus) => void;
  onEvent: (topic: LiveTopic, data: unknown) => void;
  onReopen: () => void;
};

const FIRST_BASE_MS = 10_000;
const MAX_DELAY_MS = 60_000;

/** Never before the API's `Retry-After`. */
export function reopenDelayMs(refusals: number, random: number): number {
  const base = Math.min(FIRST_BASE_MS * 2 ** refusals, MAX_DELAY_MS);
  return base / 2 + (random * base) / 2;
}

/** `afterLoss`: the page already missed events, so the first opening is a reopening. `close` tells the next. */
export function openLive(
  url: string,
  topics: readonly LiveTopic[],
  handlers: LiveHandlers,
  afterLoss: boolean,
): { close: () => boolean } {
  let missed = afterLoss;
  let opened = false;
  let refusals = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let source = connect();

  function connect(): EventSource {
    const next = new EventSource(url);

    next.onopen = () => {
      handlers.onStatus("live");
      if (missed) {
        handlers.onReopen();
      }
      missed = false;
      opened = true;
      refusals = 0;
    };

    next.onerror = () => {
      handlers.onStatus("reconnecting");
      missed = true;
      // ⚠️ CLOSED is final: after a non-200 or non-stream answer the browser never retries itself.
      if (next.readyState === EventSource.CLOSED) {
        timer = setTimeout(() => {
          source = connect();
        }, reopenDelayMs(refusals++, Math.random()));
      }
    };

    for (const topic of new Set(topics)) {
      // ⚠️ A named event: `onmessage` only receives the unnamed ones.
      next.addEventListener(topic, (event) => {
        let data: unknown;
        try {
          data = JSON.parse((event as MessageEvent<string>).data);
        } catch {
          console.warn(`live ${topic}: unparseable event skipped`);
          return;
        }
        handlers.onEvent(topic, data);
      });
    }

    return next;
  }

  return {
    close: () => {
      clearTimeout(timer);
      source.close();
      return missed || opened;
    },
  };
}
