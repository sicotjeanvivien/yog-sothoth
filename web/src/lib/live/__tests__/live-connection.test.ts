import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { openLive, reopenDelayMs, type LiveHandlers } from "../live-connection";

/** One `fetch` the connection made, answered by the test. */
type Call = {
  url: string;
  authorization: string | null;
  signal: AbortSignal;
  answer: (response: Response) => void;
  stream?: ReadableStreamDefaultController<Uint8Array>;
};

let calls: Call[] = [];
let issued = 0;

function fakeFetch(url: string, init: RequestInit): Promise<Response> {
  return new Promise((resolve) => {
    calls.push({
      url,
      authorization: new Headers(init.headers).get("authorization"),
      signal: init.signal!,
      answer: resolve,
    });
  });
}

/** A token per call, numbered, so two openings show two tokens. */
const token = () => Promise.resolve(`token-${++issued}`);

/** Answer `call` with an open stream. */
function accept(call: Call): void {
  const body = new ReadableStream<Uint8Array>({
    start: (controller) => {
      call.stream = controller;
    },
  });
  call.answer(new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } }));
}

function send(call: Call, text: string): void {
  call.stream!.enqueue(new TextEncoder().encode(text));
}

const LIVE_URL = "http://api.test/api/live?topics=signals";

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

/** Let the connection's promises settle. */
const settle = () => vi.advanceTimersByTimeAsync(0);
const last = () => calls[calls.length - 1]!;

beforeEach(() => {
  calls = [];
  issued = 0;
  vi.stubGlobal("fetch", fakeFetch);
  vi.useFakeTimers();
  vi.spyOn(Math, "random").mockReturnValue(1);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

// Mutation: the token asked once and reused — the API refuses its second opening.
it("opens with a live token in Authorization, a fresh one at each opening", async () => {
  openLive(LIVE_URL, ["signals"], handlers(), false, token);
  await settle();
  expect(calls[0]!.url).toBe(LIVE_URL);
  expect(calls[0]!.authorization).toBe("Bearer token-1");

  accept(calls[0]!);
  await settle();
  calls[0]!.stream!.close();
  await vi.advanceTimersByTimeAsync(10_000);

  expect(calls).toHaveLength(2);
  expect(calls[1]!.authorization).toBe("Bearer token-2");
});

// Mutation: a stream that ends is not reopened — the page stays frozen until it is reloaded.
describe("a lost connection", () => {
  it("reopens on the same URL after the delay, and that opening fills the gap", async () => {
    const h = handlers();
    openLive(LIVE_URL, ["signals"], h, false, token);
    await settle();
    accept(calls[0]!);
    await settle();
    calls[0]!.stream!.close();
    await settle();

    await vi.advanceTimersByTimeAsync(9_999);
    expect(calls).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(calls).toHaveLength(2);
    expect(last().url).toBe(LIVE_URL);

    accept(last());
    await settle();
    expect(h.reopens).toBe(1);
  });

  // Mutation: a refusal reopened at once — every tab hammers a full API.
  it("waits the refusal's Retry-After when it is longer than the delay", async () => {
    openLive(LIVE_URL, ["signals"], handlers(), false, token);
    await settle();
    calls[0]!.answer(new Response("full", { status: 503, headers: { "retry-after": "120" } }));
    await settle();

    await vi.advanceTimersByTimeAsync(119_999);
    expect(calls).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(calls).toHaveLength(2);
  });

  // Mutation: the refusal's body left unread and open, holding its connection until collected.
  it("releases the body of a refusal", async () => {
    let cancelled = false;
    const body = new ReadableStream({ cancel: () => void (cancelled = true) });
    openLive(LIVE_URL, ["signals"], handlers(), false, token);
    await settle();
    calls[0]!.answer(new Response(body, { status: 401 }));
    await settle();

    expect(cancelled).toBe(true);
  });

  // Mutation: any 200 read as the stream — an error page from a proxy shows "live" and streams nothing.
  it("takes a 200 that is no event stream for a refusal", async () => {
    const statuses: string[] = [];
    openLive(LIVE_URL, ["signals"], { ...handlers(), onStatus: (s) => statuses.push(s) }, false, token);
    await settle();
    calls[0]!.answer(new Response("<html></html>", { status: 200, headers: { "content-type": "text/html" } }));
    await settle();

    expect(statuses).toEqual(["reconnecting"]);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(calls).toHaveLength(2);
  });

  // Mutation: a token refused by the dashboard's server stops the connection for good.
  it("reopens when no token could be had", async () => {
    let failing = true;
    const flaky = () => (failing ? Promise.reject(new Error("down")) : token());
    openLive(LIVE_URL, ["signals"], handlers(), false, flaky);
    await settle();
    expect(calls).toHaveLength(0);

    failing = false;
    await vi.advanceTimersByTimeAsync(10_000);
    expect(calls).toHaveLength(1);
  });
});

describe("the reopening delay", () => {
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
    openLive(LIVE_URL, ["signals"], handlers(), false, token);
    await settle();
    calls[0]!.answer(new Response("", { status: 503 }));
    await vi.advanceTimersByTimeAsync(10_000);
    calls[1]!.answer(new Response("", { status: 503 }));
    await vi.advanceTimersByTimeAsync(20_000);
    expect(calls).toHaveLength(3);

    accept(calls[2]!);
    await settle();
    calls[2]!.stream!.close();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(calls).toHaveLength(4);
  });
});

// Mutation: `close` leaves the request or the timer — an unmounted page, or the old topics, keep a connection.
describe("close", () => {
  it("aborts the open request", async () => {
    const connection = openLive(LIVE_URL, ["signals"], handlers(), false, token);
    await settle();
    connection.close();
    expect(calls[0]!.signal.aborted).toBe(true);
  });

  it("opens nothing once closed during the wait", async () => {
    const connection = openLive(LIVE_URL, ["signals"], handlers(), false, token);
    await settle();
    calls[0]!.answer(new Response("", { status: 503 }));
    await settle();
    connection.close();
    await vi.runAllTimersAsync();
    expect(calls).toHaveLength(1);
  });
});

// Mutation: a refusal before any opening counts as no loss — the next connection would not mark the gap.
it("counts a refusal before the first opening as a gap", async () => {
  const h = handlers();
  const first = openLive(LIVE_URL, ["signals"], h, false, token);
  await settle();
  calls[0]!.answer(new Response("", { status: 503 }));
  await vi.advanceTimersByTimeAsync(10_000);
  accept(last());
  await settle();
  expect(h.reopens).toBe(1);
  first.close();

  const second = openLive(LIVE_URL, ["signals"], handlers(), false, token);
  await settle();
  last().answer(new Response("", { status: 503 }));
  await settle();
  expect(second.close()).toBe(true);
});

// Mutation: `close` reports no gap after an opening — the next connection would not mark it.
it("hands the gap to the next connection", async () => {
  const first = openLive(LIVE_URL, ["signals"], handlers(), false, token);
  await settle();
  accept(calls[0]!);
  await settle();
  expect(first.close()).toBe(true);

  const h = handlers();
  openLive(LIVE_URL, ["signals"], h, true, token);
  await settle();
  accept(last());
  await settle();
  expect(h.reopens).toBe(1);
});

// Mutations: no dispatch per topic, the wrong topic handed on, no status — the feed falls silent or stays "reconnecting".
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
    token,
  );
  await settle();
  const call = calls[0]!;
  accept(call);
  await settle();
  send(call, 'event: signals\ndata: {"id":1}\n\n');
  send(call, 'event: pool-creations\ndata: {"id":2}\n\nevent: pool-creations\ndata: not json\n\n');
  send(call, 'event: swaps\ndata: {"id":3}\n\n: keep-alive\n\n');
  await settle();
  call.stream!.close();
  await settle();

  expect(events).toEqual([
    ["signals", { id: 1 }],
    ["pool-creations", { id: 2 }],
  ]);
  expect(warn).toHaveBeenCalledOnce();
  expect(statuses).toEqual(["live", "reconnecting"]);
});
