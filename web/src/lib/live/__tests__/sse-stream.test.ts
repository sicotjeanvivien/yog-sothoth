import { afterEach, expect, it, vi } from "vitest";

import type { SseEvent } from "../sse-parser";
import { openSseStream } from "../sse-stream";

const URL_ = "http://api.test/api/live?topics=signals";

function body(...chunks: string[]): ReadableStream<Uint8Array> {
  return new ReadableStream({
    start(controller) {
      for (const chunk of chunks) controller.enqueue(new TextEncoder().encode(chunk));
      controller.close();
    },
  });
}

function stubFetch(response: Response | Error) {
  const fetched = vi.fn(async () => {
    if (response instanceof Error) throw response;
    return response;
  });
  vi.stubGlobal("fetch", fetched);
  return fetched;
}

function read(onOpen = () => {}) {
  const events: SseEvent[] = [];
  const end = openSseStream(URL_, "yogt_abc", { onOpen, onEvent: (e) => events.push(e) }, new AbortController().signal);
  return { events, end };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

// Mutation: the ticket put in the URL, or left out — the API refuses, and a URL is logged.
it("sends the ticket in the Authorization header, not in the URL", async () => {
  const fetched = stubFetch(new Response(body(), { status: 200 }));
  await read().end;
  expect(fetched).toHaveBeenCalledWith(
    URL_,
    expect.objectContaining({
      headers: { Accept: "text/event-stream", Authorization: "Bearer yogt_abc" },
    }),
  );
});

// Mutations: a chunk read as a whole event; the stream's end not reported.
it("hands on each event of a body cut anywhere, then reports the stream closed", async () => {
  stubFetch(new Response(body("event: sig", "nals\ndata: {\"id\":1}\n", "\nevent: signals\ndata: 2\n\n"), { status: 200 }));
  const opened = vi.fn();
  const { events, end } = read(opened);

  expect(await end).toEqual({ kind: "closed" });
  expect(opened).toHaveBeenCalledOnce();
  expect(events.map((e) => [e.event, e.data])).toEqual([
    ["signals", '{"id":1}'],
    ["signals", "2"],
  ]);
});

// Mutations: a refusal read as an open stream; its body or its Retry-After lost — the page reopens too soon, the explorer shows nothing.
it("reports a refusal with its status, body and Retry-After, without opening", async () => {
  stubFetch(new Response('{"detail":"full"}', { status: 503, statusText: "Service Unavailable", headers: { "Retry-After": "5" } }));
  const opened = vi.fn();
  expect(await read(opened).end).toEqual({
    kind: "refused",
    status: 503,
    statusText: "Service Unavailable",
    body: '{"detail":"full"}',
    retryAfterMs: 5_000,
  });
  expect(opened).not.toHaveBeenCalled();
});

it("reads no Retry-After when the API sends none", async () => {
  stubFetch(new Response("{}", { status: 401 }));
  expect(await read().end).toMatchObject({ kind: "refused", status: 401, retryAfterMs: null });
});

// Mutation: a network failure thrown — the live connection's loop dies with it.
it("reports an unreachable API instead of throwing", async () => {
  const error = new TypeError("fetch failed");
  stubFetch(error);
  expect(await read().end).toEqual({ kind: "unreachable", error });
});
