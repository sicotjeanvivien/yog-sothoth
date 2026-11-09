import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { dropDelayMs, openLive, reopenDelayMs, type LiveHandlers } from "../live-connection";

/** One `fetch` of the stream, answered by the test. */
class FakeStream {
  static all: FakeStream[] = [];
  private answer!: (response: Response) => void;
  private refuse!: (error: unknown) => void;
  private body: ReadableStreamDefaultController<Uint8Array> | null = null;
  readonly answered: Promise<Response>;
  aborted = false;

  constructor(
    readonly url: string,
    readonly init: RequestInit,
  ) {
    this.answered = new Promise((resolve, reject) => {
      this.answer = resolve;
      this.refuse = reject;
    });
    init.signal?.addEventListener("abort", () => {
      this.aborted = true;
      const error = new DOMException("aborted", "AbortError");
      this.refuse(error);
      this.body?.error(error);
    });
    FakeStream.all.push(this);
  }

  get ticket(): string {
    return (this.init.headers as Record<string, string>)["Authorization"] ?? "";
  }

  open(): void {
    const body = new ReadableStream<Uint8Array>({ start: (c) => void (this.body = c) });
    this.answer(new Response(body, { status: 200, headers: { "Content-Type": "text/event-stream" } }));
  }

  refused(status: number, headers: Record<string, string> = {}): void {
    this.answer(new Response('{"status":503}', { status, headers }));
  }

  emit(event: string, data: string): void {
    this.body?.enqueue(new TextEncoder().encode(`event: ${event}\ndata: ${data}\n\n`));
  }

  end(): void {
    this.body?.close();
  }
}

const LIVE_URL = "http://api.test/api/live?topics=signals";
const streams = () => FakeStream.all;
const last = () => streams()[streams().length - 1]!;
const flush = () => vi.advanceTimersByTimeAsync(0);

function handlers(): LiveHandlers & { reopens: number } {
  const h = {
    reopens: 0,
    onStatus: () => {},
    onEvent: () => {},
    onReopen: () => {
      h.reopens += 1;
    },
  };
  return h;
}

let issued = 0;
const requestTicket = vi.fn(async () => `yogt_${++issued}`);

beforeEach(() => {
  FakeStream.all = [];
  issued = 0;
  requestTicket.mockClear();
  vi.stubGlobal("fetch", (url: string, init: RequestInit) => new FakeStream(url, init).answered);
  vi.useFakeTimers();
  vi.spyOn(Math, "random").mockReturnValue(1);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

async function opened(h: LiveHandlers = handlers(), afterLoss = false) {
  const connection = openLive(LIVE_URL, ["signals"], h, afterLoss, requestTicket);
  await flush();
  return connection;
}

// Mutation: the ticket asked once and reused — the API refuses it at the second opening.
it("asks a new ticket for every opening, and sends it in the header", async () => {
  await opened();
  last().open();
  await flush();
  last().end();
  await vi.advanceTimersByTimeAsync(3_000);

  expect(requestTicket).toHaveBeenCalledTimes(2);
  expect(streams().map((s) => s.ticket)).toEqual(["Bearer yogt_1", "Bearer yogt_2"]);
  expect(streams().map((s) => s.url)).toEqual([LIVE_URL, LIVE_URL]);
});

describe("an open stream that drops", () => {
  // Mutation: the refusals' delay used for a drop — every cut leaves a 5 s hole at least.
  it("reopens within 1 to 3 s, and that opening marks the gap", async () => {
    const h = handlers();
    await opened(h);
    last().open();
    await flush();
    last().end();

    await vi.advanceTimersByTimeAsync(2_999);
    expect(streams()).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(streams()).toHaveLength(2);

    last().open();
    await flush();
    expect(h.reopens).toBe(1);
  });

  // Mutation: no random part — every visitor an API restart cuts comes back at the same instant.
  it("spreads the retry between 1 and 3 s", () => {
    expect(dropDelayMs(0)).toBe(1_000);
    expect(dropDelayMs(0.5)).toBe(2_000);
    expect(dropDelayMs(1)).toBe(3_000);
  });
});

describe("a refused opening", () => {
  // Mutation: a refusal retried on the drop's rhythm — a full API is hammered every 3 s.
  it("waits the refusals' delay", async () => {
    await opened();
    last().refused(503);
    await flush();

    await vi.advanceTimersByTimeAsync(9_999);
    expect(streams()).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(streams()).toHaveLength(2);
  });

  // Mutation: `Retry-After` ignored — the page comes back before the API said it could.
  it("never comes back before the API's Retry-After", async () => {
    await opened();
    last().refused(503, { "Retry-After": "30" });
    await flush();

    await vi.advanceTimersByTimeAsync(29_999);
    expect(streams()).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(streams()).toHaveLength(2);
  });

  // Mutation: a failed ticket request never retried — the page stays without its stream.
  it("counts a failed ticket request as a refusal", async () => {
    requestTicket.mockRejectedValueOnce(new Error("401")).mockRejectedValueOnce(new Error("401"));
    await opened();
    expect(streams()).toHaveLength(0);

    await vi.advanceTimersByTimeAsync(10_000);
    expect(requestTicket).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(19_999);
    expect(requestTicket).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(streams()).toHaveLength(1);
  });
});

describe("the refusals' delay", () => {
  // Mutation: the base never doubles.
  it("doubles per consecutive refusal", () => {
    expect([0, 1, 2].map((n) => reopenDelayMs(n, 1))).toEqual([10_000, 20_000, 40_000]);
  });

  // Mutation: no cap — the fourth refusal would wait 80 s, the tenth over an hour.
  it("is capped at 60 s", () => {
    expect(reopenDelayMs(3, 1)).toBe(60_000);
    expect(reopenDelayMs(30, 1)).toBe(60_000);
  });

  // Mutation: no random part — every tab refused together comes back at the same second.
  it("keeps a random half", () => {
    expect(reopenDelayMs(0, 0)).toBe(5_000);
    expect(reopenDelayMs(30, 0)).toBe(30_000);
  });

  // Mutation: an opening does not reset the count — a later refusal waits as if the first ones had just happened.
  it("starts over after an opening", async () => {
    await opened();
    last().refused(503);
    await vi.advanceTimersByTimeAsync(10_000);
    last().refused(503);
    await vi.advanceTimersByTimeAsync(20_000);
    expect(streams()).toHaveLength(3);

    last().open();
    await flush();
    last().end();
    await vi.advanceTimersByTimeAsync(3_000);
    last().refused(503);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(streams()).toHaveLength(5);
  });
});

// Mutation: `close` neither clears the timer nor stops the loop — an unmounted page reconnects behind its back.
it("opens nothing once closed during the wait", async () => {
  const connection = await opened();
  last().refused(503);
  await flush();
  connection.close();
  await vi.runAllTimersAsync();
  expect(streams()).toHaveLength(1);
});

// Mutation: `close` does not abort — the stream keeps a place on the API after the page left.
it("aborts the open stream on close", async () => {
  const connection = await opened();
  last().open();
  await flush();
  connection.close();
  await vi.runAllTimersAsync();
  expect(last().aborted).toBe(true);
  expect(streams()).toHaveLength(1);
});

// Mutation: a refusal before any opening counts as no loss — the feed would show no gap.
it("counts a refusal before the first opening as a gap", async () => {
  const h = handlers();
  const first = await opened(h);
  last().refused(503);
  await vi.advanceTimersByTimeAsync(10_000);
  last().open();
  await flush();
  expect(h.reopens).toBe(1);

  const second = await opened();
  last().refused(503);
  await flush();
  expect(second.close()).toBe(true);
  first.close();
});

// Mutation: the first opening marked as a reopening — every page load shows a gap.
it("marks no gap at the first opening", async () => {
  const h = handlers();
  await opened(h);
  last().open();
  await flush();
  expect(h.reopens).toBe(0);
});

// Mutation: `close` reports no gap after an opening — new topics would not mark what they missed.
it("hands the gap to the next connection", async () => {
  const first = await opened();
  last().open();
  await flush();
  expect(first.close()).toBe(true);

  const h = handlers();
  await opened(h, true);
  last().open();
  await flush();
  expect(h.reopens).toBe(1);
});

// Mutations: another topic handed on, a bad payload thrown, no status — the feed falls silent or stays "reconnecting".
it("hands each topic's parsed events on, and the status", async () => {
  const events: [string, unknown][] = [];
  const statuses: string[] = [];
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  openLive(
    LIVE_URL,
    ["signals", "pool-creations", "signals"],
    {
      onStatus: (status) => statuses.push(status),
      onEvent: (topic, data) => events.push([topic, data]),
      onReopen: () => {},
    },
    false,
    requestTicket,
  );
  await flush();
  const stream = last();
  stream.open();
  await flush();
  stream.emit("signals", '{"id":1}');
  stream.emit("pool-creations", '{"id":2}');
  stream.emit("pool-creations", "not json");
  stream.emit("swaps", '{"id":3}');
  stream.end();
  await flush();

  expect(events).toEqual([
    ["signals", { id: 1 }],
    ["pool-creations", { id: 2 }],
  ]);
  expect(warn).toHaveBeenCalledOnce();
  expect(statuses).toEqual(["live", "reconnecting"]);
});
