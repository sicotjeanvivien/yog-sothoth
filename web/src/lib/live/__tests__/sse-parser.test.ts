import { describe, expect, it } from "vitest";

import { createSseParser, type SseEvent } from "../sse-parser";

function parse(chunks: string[]): SseEvent[] {
  const events: SseEvent[] = [];
  const parser = createSseParser((event) => events.push(event));
  chunks.forEach((chunk) => parser.push(chunk));
  return events;
}

describe("the event-stream parser", () => {
  // Mutation: the event name dropped — every message would land on `message`.
  it("reads a named event, its data and its id", () => {
    expect(parse(['event: signals\nid: 7\ndata: {"id":1}\n\n'])).toEqual([
      { event: "signals", data: '{"id":1}', id: "7" },
    ]);
  });

  // Mutation: a chunk read as whole lines — an event cut between two chunks is lost.
  it("joins an event cut anywhere between chunks", () => {
    const whole = 'event: signals\ndata: {"id":1}\n\n';
    for (let at = 1; at < whole.length; at += 1) {
      expect(parse([whole.slice(0, at), whole.slice(at)]), `cut at ${at}`).toEqual([
        { event: "signals", data: '{"id":1}', id: null },
      ]);
    }
  });

  // Mutation: `\r\n` read as two line ends — the blank line it hides would dispatch half an event.
  it("reads CRLF and CR line ends, a CRLF split across chunks included", () => {
    expect(parse(["event: a\r\ndata: 1\r", "\n\r\n", "event: b\rdata: 2\r\r"])).toEqual([
      { event: "a", data: "1", id: null },
      { event: "b", data: "2", id: null },
    ]);
  });

  // Mutation: each data line its own event.
  it("joins the data lines of one event with a newline", () => {
    expect(parse(["data: a\ndata: b\n\n"])).toEqual([{ event: "message", data: "a\nb", id: null }]);
  });

  // Mutation: a comment read as a field — the keep-alive would dispatch an empty event.
  it("skips the keep-alive comments and a blank line without data", () => {
    expect(parse([":\n\n: keep-alive\n\n"])).toEqual([]);
  });

  // Mutation: the name carried over to the next event.
  it("forgets the event name once dispatched", () => {
    expect(parse(["event: a\ndata: 1\n\ndata: 2\n\n"]).map((e) => e.event)).toEqual(["a", "message"]);
  });
});
