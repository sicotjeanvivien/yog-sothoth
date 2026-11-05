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

  addEventListener(): void {}

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

    vi.advanceTimersByTime(4_999);
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
    expect([0, 1, 2, 3].map((n) => reopenDelayMs(n, 1))).toEqual([5_000, 10_000, 20_000, 40_000]);
  });

  // Mutation: no cap — the fifth refusal would wait 80 s, the tenth over an hour.
  it("is capped at 60 s", () => {
    expect(reopenDelayMs(4, 1)).toBe(60_000);
    expect(reopenDelayMs(30, 1)).toBe(60_000);
  });

  // Mutation: no random part — every tab refused together comes back at the same second.
  it("keeps a random half", () => {
    expect(reopenDelayMs(0, 0)).toBe(2_500);
    expect(reopenDelayMs(30, 0)).toBe(30_000);
  });

  // Mutation: an opening does not reset the count — a later refusal waits as if the first ones had just happened.
  it("starts over after an opening", () => {
    openLive(LIVE_URL, ["signals"], handlers(), false);
    sources()[0]!.fail(FakeEventSource.CLOSED);
    vi.advanceTimersByTime(5_000);
    sources()[1]!.fail(FakeEventSource.CLOSED);
    vi.advanceTimersByTime(10_000);
    expect(sources()).toHaveLength(3);

    sources()[2]!.open();
    sources()[2]!.fail(FakeEventSource.CLOSED);
    vi.advanceTimersByTime(5_000);
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
