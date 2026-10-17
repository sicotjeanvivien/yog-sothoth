/** Mutations: a technical line counted as visible, or `internalOnly` true with a visible line. */

import { describe, expect, it } from "vitest";

import type { Release } from "../releases";
import { releaseView } from "../release-view";

function release(sections: Release["sections"]): Release {
  return { version: "v9.9.9", date: "2026-01-01", summary: "s", sections };
}

describe("releaseView", () => {
  it("folds technical lines apart from the visible ones, section by section", () => {
    const view = releaseView(
      release([
        {
          kind: "features",
          items: [
            { text: "f1", technical: false },
            { text: "t1", technical: true },
          ],
        },
        {
          kind: "fixes",
          items: [
            { text: "x1", technical: false },
            { text: "t2", technical: true },
          ],
        },
      ]),
    );
    expect(view).toEqual({
      features: ["f1"],
      fixes: ["x1"],
      technical: ["t1", "t2"],
      internalOnly: false,
    });
  });

  it("is internal only when every line is technical", () => {
    const view = releaseView(
      release([{ kind: "fixes", items: [{ text: "t", technical: true }] }]),
    );
    expect(view.internalOnly).toBe(true);
    expect(view.technical).toEqual(["t"]);
  });

  it("is not internal only with a single visible fix", () => {
    const view = releaseView(
      release([
        { kind: "features", items: [{ text: "t", technical: true }] },
        { kind: "fixes", items: [{ text: "x", technical: false }] },
      ]),
    );
    expect(view.internalOnly).toBe(false);
  });
});
