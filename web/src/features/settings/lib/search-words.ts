import type { SettingText } from "@/shared/lib/setting-text";

const WORD = /[\p{L}\p{N}]+/gu;

/** `text` as search compares it: without accents, in lower case. */
function folded(text: string) {
  return text.normalize("NFD").replaceAll(/\p{M}/gu, "").toLowerCase();
}

/** The words `typed` into a search, as it compares them. */
export function typedWords(typed: string) {
  return folded(typed).match(WORD) ?? [];
}

/** Whether every word `asked` starts a word of what `text`'s row says or answers to. */
export function answers(text: SettingText, asked: string[]) {
  const words = typedWords(`${text.label} ${text.hint ?? ""} ${text.words ?? ""}`);
  return asked.every((typed) => words.some((word) => word.startsWith(typed)));
}

/** A run of a label's letters, marked where a word asked starts one of its words. */
export interface LabelPart {
  text: string;
  marked: boolean;
}

/** `label` in parts, each start of a word that a word `asked` starts marked. */
export function markedParts(label: string, asked: string[]): LabelPart[] {
  const parts: LabelPart[] = [];
  let at = 0;
  for (const { 0: word, index } of label.matchAll(WORD)) {
    const length = markedLength(word, asked);
    if (length > 0) {
      parts.push(
        { text: label.slice(at, index), marked: false },
        { text: label.slice(index, index + length), marked: true },
      );
      at = index + length;
    }
  }
  parts.push({ text: label.slice(at), marked: false });
  return parts.filter((part) => part.text !== "");
}

/** How many of `word`'s letters the longest word `asked` that starts it covers, accents and all. */
function markedLength(word: string, asked: string[]) {
  const wordFolded = folded(word);
  const longest = Math.max(
    0,
    ...asked.filter((typed) => wordFolded.startsWith(typed)).map((typed) => typed.length),
  );
  // Folding can change a letter's length, as "é" written as two code points, so count as it folds.
  let length = 0;
  let covered = 0;
  for (const letter of word) {
    if (covered >= longest) {
      break;
    }
    covered += folded(letter).length;
    length += letter.length;
  }
  return length;
}
