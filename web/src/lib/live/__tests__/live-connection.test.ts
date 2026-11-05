import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { openLive, reopenDelayMs, type LiveHandlers } from "../live-connection";

class FakeEventSource {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 2;
  static instances: FakeEventSource[] = [];

  readyState = FakeEventSource.CONNECTING;
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  closed = false;

  constructor(readonly url: string) {
    FakeEventSource.instances.push(this);
  }

  listeners = new Map<string, ((event: { data: string }) => void)[]>();

  addEventListener(topic: string, listener: (event: { data: string }) => void): void {
    this.listeners.set(topic, [...(this.listeners.get(topic) ?? []), listener]);
  }

  emit(topic: string, data: string): void {
    this.listeners.get(topic)?.forEach((listener) => listener({ data }));
  }

  close(): void {
    this.closed = true;
  }

  open(): void {
    this.readyState = FakeEventSource.OPEN;
    this.onopen?.();
  }

  fail(readyState: number): void {
    this.readyState = readyState;
    this.onerror?.();
  }
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

const sources = () => FakeEventSource.instances;
const last = () => sources()[sources().length - 1]!;

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.useFakeTimers();
  vi.spyOn(Math, "random").mockReturnValue(1);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

// Mutation: `onerror` never reopens — the page stays frozen until it is reloaded.
describe("a refused connection", () => {
  it("reopens on the same URL after the delay, and that opening fills the gap", () => {
    const h = handlers();
    openLive(LIVE_URL, ["signals"], h, false);
    sources()[0]!.open();
    sources()[0]!.fail(FakeEventSource.CLOSED);

    vi.advanceTimersByTime(9_999);
    expect(sources()).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(sources()).toHaveLength(2);
    expect(last().url).toBe(LIVE_URL);

    last().open();
    expect(h.reopens).toBe(1);
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
  it("starts over after an opening", () => {
    openLive(LIVE_URL, ["signals"], handlers(), false);
    sources()[0]!.fail(FakeEventSource.CLOSED);
    vi.advanceTimersByTime(10_000);
    sources()[1]!.fail(FakeEventSource.CLOSED);
    vi.advanceTimersByTime(20_000);
    expect(sources()).toHaveLength(3);

    sources()[2]!.open();
    sources()[2]!.fail(FakeEventSource.CLOSED);
    vi.advanceTimersByTime(10_000);
    expect(sources()).toHaveLength(4);
  });
});

// Mutation: reopen on every error — a second connection beside the one the browser is already retrying.
it("leaves a CONNECTING error to the browser's own retry", () => {
  openLive(LIVE_URL, ["signals"], handlers(), false);
  sources()[0]!.open();
  sources()[0]!.fail(FakeEventSource.CONNECTING);
  vi.runAllTimers();
  expect(sources()).toHaveLength(1);
});

// Mutation: `close` leaves the timer — an unmounted page, or the old topics, reconnect behind its back.
it("opens nothing once closed during the wait", () => {
  const connection = openLive(LIVE_URL, ["signals"], handlers(), false);
  sources()[0]!.fail(FakeEventSource.CLOSED);
  connection.close();
  vi.runAllTimers();
  expect(sources()).toHaveLength(1);
  expect(sources()[0]!.closed).toBe(true);
});

// Mutation: `close` reports no gap after an opening — new topics would not refill what they missed.
it("hands the gap to the next connection", () => {
  const first = openLive(LIVE_URL, ["signals"], handlers(), false);
  sources()[0]!.open();
  expect(first.close()).toBe(true);

  const h = handlers();
  openLive(LIVE_URL, ["signals"], h, true);
  last().open();
  expect(h.reopens).toBe(1);
});

// Mutations: no listener per topic, the wrong topic handed on, no status — the feed falls silent or stays "reconnecting".
it("hands each topic's parsed events on, and the status", () => {
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
  );
  const source = sources()[0]!;
  source.open();
  source.emit("signals", '{"id":1}');
  source.emit("pool-creations", '{"id":2}');
  source.emit("pool-creations", "not json");
  source.emit("swaps", '{"id":3}');
  source.fail(FakeEventSource.CONNECTING);

  expect(events).toEqual([
    ["signals", { id: 1 }],
    ["pool-creations", { id: 2 }],
  ]);
  expect(warn).toHaveBeenCalledOnce();
  expect(statuses).toEqual(["live", "reconnecting"]);
});
