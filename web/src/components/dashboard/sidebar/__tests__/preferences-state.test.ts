import { describe, expect, it } from "vitest";

import { preferencesClick } from "../preferences-state";

describe("preferencesClick", () => {
  // Mutation: `expand` dropped — the panel opens inside a rail too narrow to show it.
  it("unfolds the collapsed rail and opens the panel", () => {
    expect(preferencesClick({ collapsed: true, open: false })).toEqual({ open: true, expand: true });
    expect(preferencesClick({ collapsed: true, open: true })).toEqual({ open: true, expand: true });
  });

  // Mutation: the collapsed rule applied everywhere — the panel can no longer close.
  it("toggles the panel on the unfolded rail", () => {
    expect(preferencesClick({ collapsed: false, open: false })).toEqual({ open: true, expand: false });
    expect(preferencesClick({ collapsed: false, open: true })).toEqual({ open: false, expand: false });
  });
});
