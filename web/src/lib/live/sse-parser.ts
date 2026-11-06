/**
 * Server-sent events read off a `fetch` body, chunk by chunk, as the WHATWG event-stream format
 * cuts them: a chunk may end anywhere, a line in `\n`, `\r\n` or `\r`.
 */

export type SseEvent = { event: string; data: string; id: string | null };

/** `push` each decoded chunk; `onEvent` receives every event a blank line completes. */
export function createSseParser(onEvent: (event: SseEvent) => void): { push: (chunk: string) => void } {
  let line = "";
  // ⚠️ A `\r` ending a chunk may be the first half of a `\r\n`: its `\n` must not end a second line.
  let afterCr = false;
  let event = "";
  let data: string[] = [];
  let id: string | null = null;

  function field(text: string): void {
    if (text === "") {
      if (data.length > 0) {
        onEvent({ event: event || "message", data: data.join("\n"), id });
      }
      event = "";
      data = [];
      return;
    }
    if (text.startsWith(":")) return;
    const colon = text.indexOf(":");
    const name = colon === -1 ? text : text.slice(0, colon);
    let value = colon === -1 ? "" : text.slice(colon + 1);
    if (value.startsWith(" ")) value = value.slice(1);
    if (name === "event") event = value;
    else if (name === "data") data.push(value);
    else if (name === "id" && !value.includes("\0")) id = value;
  }

  return {
    push(chunk: string): void {
      for (const char of chunk) {
        if (afterCr && char === "\n") {
          afterCr = false;
          continue;
        }
        afterCr = char === "\r";
        if (char === "\n" || char === "\r") {
          field(line);
          line = "";
        } else {
          line += char;
        }
      }
    },
  };
}
