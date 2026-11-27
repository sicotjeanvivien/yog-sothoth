import { describe, expect, it } from "vitest";

import { formatTokenText } from "../format-token-text";

describe("formatTokenText", () => {
  // Mutation: a category dropped from the rule — its character passes unseen.
  it.each([
    ["C0 control", "BELL\u0007", "BELL⟨U+0007⟩"],
    ["right-to-left override", "\u202ETDSU", "⟨U+202E⟩TDSU"],
    ["left-to-right isolate", "\u2066SOL", "⟨U+2066⟩SOL"],
    ["left-to-right mark", "\u200E", "⟨U+200E⟩"],
    ["arabic letter mark", "A\u061CB", "A⟨U+061C⟩B"],
    ["zero-width space", "US\u200BDC", "US⟨U+200B⟩DC"],
    ["byte order mark", "\uFEFFJUP", "⟨U+FEFF⟩JUP"],
    ["soft hyphen", "US\u00ADDT", "US⟨U+00AD⟩DT"],
    ["combining grapheme joiner", "SO\u034FL", "SO⟨U+034F⟩L"],
    ["hangul filler", "\u3164", "⟨U+3164⟩"],
    ["line separator", "A\u2028B", "A⟨U+2028⟩B"],
    ["interlinear annotation anchor", "A\uFFF9B", "A⟨U+FFF9⟩B"],
    ["tag character outside an emoji", "USDT\u{E0041}", "USDT⟨U+E0041⟩"],
  ])("shows a %s", (_, text, shown) => {
    expect(formatTokenText(text)).toBe(shown);
  });

  // Mutation: the glue kept wherever it sits — "USD\u200DT" reads "USDT".
  it("shows a joiner or a variation selector that sits between letters", () => {
    expect(formatTokenText("USD\u200DT")).toBe("USD⟨U+200D⟩T");
    expect(formatTokenText("A\uFE0F")).toBe("A⟨U+FE0F⟩");
  });

  // Mutation: the glue marked inside an emoji — a composed emoji falls apart.
  it.each([
    ["family", "\u{1F468}\u200D\u{1F469}\u200D\u{1F467}"],
    ["red heart", "❤\uFE0F"],
    ["skin tone and profession", "\u{1F469}\u{1F3FD}\u200D\u{1F4BB}"],
    ["keycap", "1\uFE0F\u20E3"],
    ["flag of England", "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}"],
  ])("leaves a composed emoji whole: %s", (_, emoji) => {
    expect(formatTokenText(emoji)).toBe(emoji);
  });

  // Mutation: a drawn character marked — an ordinary or right-to-left symbol altered.
  it("leaves a drawn symbol unchanged, right-to-left script included", () => {
    expect(formatTokenText("USDC")).toBe("USDC");
    expect(formatTokenText("دولار")).toBe("دولار");
  });

  // Mutation: only the first undrawn character marked.
  it("shows every undrawn character of a run", () => {
    expect(formatTokenText("\u200E \u200E")).toBe("⟨U+200E⟩ ⟨U+200E⟩");
  });
});
