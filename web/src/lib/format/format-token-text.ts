/** A token's symbol or name as the chain holds it, each character the browser would not draw shown as `⟨U+XXXX⟩`. */

/** Controls, format characters, default-ignorables and line/paragraph separators: drawn as nothing. */
const UNDRAWN = /[\p{Cc}\p{Cf}\p{Default_Ignorable_Code_Point}\p{Zl}\p{Zp}]/u;

const EMOJI = /\p{Extended_Pictographic}|\u20E3/u;

/** ZWJ, variation selectors and tag characters: what composes an emoji.
 * ⚠️ Kept only inside an emoji's grapheme; between letters a ZWJ would hide "USD\u200DT" as "USDT". */
const EMOJI_GLUE = /[\u200D\uFE00-\uFE0F\u{E0020}-\u{E007F}]/u;

const GRAPHEMES = new Intl.Segmenter("en", { granularity: "grapheme" });

function mark(char: string): string {
  const codePoint = char.codePointAt(0) ?? 0;
  return `⟨U+${codePoint.toString(16).toUpperCase().padStart(4, "0")}⟩`;
}

/** `"\u202ETDSU"` → `"⟨U+202E⟩TDSU"`: the reader sees every character, in the chain's order. */
export function formatTokenText(text: string): string {
  let shown = "";
  for (const { segment } of GRAPHEMES.segment(text)) {
    const inEmoji = EMOJI.test(segment);
    for (const char of segment) {
      shown += UNDRAWN.test(char) && !(inEmoji && EMOJI_GLUE.test(char)) ? mark(char) : char;
    }
  }
  return shown;
}
