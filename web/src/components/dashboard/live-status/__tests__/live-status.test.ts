import { describe, expect, it } from "vitest";

import messages from "@/messages/fr/dashboard.json";

import { LIVE_LEGEND, liveStatusOf } from "../live-status";

const LABELS = messages.Dashboard.liveStatus.states;

// Mutation: any one state's dot, text colour or label swapped for another's.
describe("liveStatusOf", () => {
  it.each([
    ["live", "bg-dash-up", "text-dash-up", "En direct"],
    ["delayed", "bg-dash-warn", "text-dash-warn", "Retardé"],
    ["stale", "bg-dash-ink-3", "text-dash-ink-3", "Figé"],
  ] as const)("reads %s from a fresh answer", (freshness, dot, text, label) => {
    const status = liveStatusOf({
      phase: "ready",
      data: { freshness, lastEventAt: null },
    });

    expect(status.state).toBe(freshness);
    expect(status.dotClass).toBe(dot);
    expect(status.textClass).toBe(text);
    expect(LABELS[status.state]).toBe(label);
  });

  it("reads offline, in light red, from a failed call", () => {
    const status = liveStatusOf({ phase: "error" });

    expect(status.state).toBe("offline");
    expect(status.dotClass).toBe("bg-dash-down");
    expect(status.textClass).toBe("text-dash-down");
    expect(LABELS[status.state]).toBe("Hors ligne");
  });

  it("reads connecting, never live, before the first answer", () => {
    expect(liveStatusOf({ phase: "loading" }).state).toBe("connecting");
  });

  it("lists the four states in the legend, connecting left out", () => {
    expect(LIVE_LEGEND.map((entry) => entry.state)).toEqual([
      "live",
      "delayed",
      "stale",
      "offline",
    ]);
  });
});
