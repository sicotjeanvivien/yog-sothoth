import { describe, expect, it } from "vitest";

import { worstSeverity } from "../worst-severity";

describe("worstSeverity", () => {
  it("returns null for an empty list", () => {
    expect(worstSeverity([])).toBeNull();
  });

  it("returns the single severity of a one-item list", () => {
    expect(worstSeverity([{ severity: "info" }])).toBe("info");
  });

  it("picks critical over warning and info", () => {
    expect(
      worstSeverity([
        { severity: "warning" },
        { severity: "critical" },
        { severity: "info" },
      ]),
    ).toBe("critical");
  });

  it("picks warning over info regardless of order", () => {
    expect(worstSeverity([{ severity: "info" }, { severity: "warning" }])).toBe(
      "warning",
    );
    expect(worstSeverity([{ severity: "warning" }, { severity: "info" }])).toBe(
      "warning",
    );
  });
});
