/** A JSON body, pretty-printed and coloured line by line: keys, strings, numbers, literals. */

import { memo } from "react";

const TOKEN =
  /("(?:\\.|[^"\\])*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|\b(true|false|null)\b/g;

const COLOR = {
  key: "text-dash-accent-2",
  string: "text-dash-ink",
  number: "text-dash-ink-2",
  literal: "text-dash-ink-2 italic",
  punct: "text-dash-ink-3",
} as const;

type Segment = { kind: keyof typeof COLOR; text: string };

function segmentsOf(line: string): Segment[] {
  const out: Segment[] = [];
  let last = 0;
  for (const m of line.matchAll(TOKEN)) {
    const at = m.index ?? 0;
    if (at > last) out.push({ kind: "punct", text: line.slice(last, at) });
    if (m[1] && m[2]) {
      out.push({ kind: "key", text: m[1] }, { kind: "punct", text: m[2] });
    } else if (m[1]) out.push({ kind: "string", text: m[1] });
    else if (m[3]) out.push({ kind: "number", text: m[3] });
    else out.push({ kind: "literal", text: m[4] ?? "" });
    last = at + m[0].length;
  }
  if (last < line.length) out.push({ kind: "punct", text: line.slice(last) });
  return out;
}

/** Memoised: a keystroke in a parameter must not re-colour a long body. */
export const JsonView = memo(function JsonView({ text, label }: { text: string; label: string }) {
  return (
    <pre
      tabIndex={0}
      aria-label={label}
      className="mt-2.5 max-h-[520px] overflow-auto border border-dash-rule bg-dash-bg px-4 py-3.5 font-dash-mono text-[12px] leading-[1.6] text-dash-ink-3"
    >
      {text.split("\n").map((line, i) => (
        <span key={i} className="block whitespace-pre">
          {line === ""
            ? " "
            : segmentsOf(line).map((s, j) => (
                <span key={j} className={COLOR[s.kind]}>
                  {s.text}
                </span>
              ))}
        </span>
      ))}
    </pre>
  );
});
