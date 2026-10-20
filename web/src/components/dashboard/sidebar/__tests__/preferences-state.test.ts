import { describe, expect, it } from "vitest";

import { preferencesClick } from "../preferences-state";

describe("preferencesClick", () => {
  // Mutation: `expand` dropped — the panel opens inside a rail too narrow to show it.
  it("unfolds the collapsed rail and opens the panel", () => {
    for (const open of [false, true]) {
      expect(preferencesClick({ collapsed: true, onRail: true, open })).toEqual({
        open: true,
        expand: true,
      });
    }
  });

  // Mutation: the collapsed rule applied everywhere — the panel can no longer close.
  it("toggles the panel on the unfolded rail", () => {
    expect(preferencesClick({ collapsed: false, onRail: true, open: false })).toEqual({ open: true, expand: false });
    expect(preferencesClick({ collapsed: false, onRail: true, open: true })).toEqual({ open: false, expand: false });
  });

  // Mutation: `onRail` ignored — in the drawer, with the collapse cookie set, the panel never closes.
  it("toggles the panel in the drawer, whatever the collapse cookie says", () => {
    expect(preferencesClick({ collapsed: true, onRail: false, open: true })).toEqual({ open: false, expand: false });
  });
});
