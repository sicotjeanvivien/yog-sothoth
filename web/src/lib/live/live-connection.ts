/**
 * The page's one stream on `/api/live`, each opening with a new ticket. ⚠️ Nothing is replayed:
 * every opening that follows a loss leaves a gap, and `onReopen` fires.
 */

import type { LiveTopic } from "./live-topics";
import { openSseStream } from "./sse-stream";

export type StreamStatus = "connecting" | "live" | "reconnecting";

export type LiveHandlers = {
  onStatus: (status: StreamStatus) => void;
  onEvent: (topic: LiveTopic, data: unknown) => void;
  onReopen: () => void;
};

const FIRST_BASE_MS = 10_000;
const MAX_DELAY_MS = 60_000;

/** After a refusal, growing with each one. Never before the API's `Retry-After`. */
export function reopenDelayMs(refusals: number, random: number): number {
  const base = Math.min(FIRST_BASE_MS * 2 ** refusals, MAX_DELAY_MS);
  return base / 2 + (random * base) / 2;
}

/** After an open stream drops: 1 to 3 s, so the visitors an API restart cuts come back spread. */
export function dropDelayMs(random: number): number {
  return 1_000 + random * 2_000;
}

/** `afterLoss`: the page already missed events, so the first opening is a reopening. `close` tells the next. */
export function openLive(
  url: string,
  topics: readonly LiveTopic[],
  handlers: LiveHandlers,
  afterLoss: boolean,
  requestTicket: () => Promise<string>,
): { close: () => boolean } {
  const wanted = new Set<string>(topics);
  const controller = new AbortController();
  let missed = afterLoss;
  let opened = false;
  let refusals = 0;
  let closed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function connect(): Promise<void> {
    let ticket: string;
    try {
      ticket = await requestTicket();
    } catch {
      return lost(refusalDelay(null));
    }
    if (closed) return;

    const end = await openSseStream(
      url,
      ticket,
      {
        onOpen: () => {
          handlers.onStatus("live");
          if (missed) {
            handlers.onReopen();
          }
          missed = false;
          opened = true;
          refusals = 0;
        },
        onEvent: ({ event, data }) => {
          if (!wanted.has(event)) return;
          let parsed: unknown;
          try {
            parsed = JSON.parse(data);
          } catch {
            console.warn(`live ${event}: unparseable event skipped`);
            return;
          }
          handlers.onEvent(event as LiveTopic, parsed);
        },
      },
      controller.signal,
    );
    if (end.kind === "closed") {
      lost(dropDelayMs(Math.random()));
    } else {
      lost(refusalDelay(end.kind === "refused" ? end.retryAfterMs : null));
    }
  }

  function refusalDelay(retryAfterMs: number | null): number {
    return Math.max(reopenDelayMs(refusals++, Math.random()), retryAfterMs ?? 0);
  }

  function lost(delayMs: number): void {
    if (closed) return;
    handlers.onStatus("reconnecting");
    missed = true;
    timer = setTimeout(() => void connect(), delayMs);
  }

  void connect();

  return {
    close: () => {
      closed = true;
      clearTimeout(timer);
      controller.abort();
      return missed || opened;
    },
  };
}
