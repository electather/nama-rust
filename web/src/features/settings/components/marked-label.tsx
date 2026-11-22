import { markedParts, typedWords } from "@/features/settings/lib/search-words";

/** A result's `label`, each start of a word that a word `typed` starts marked. */
export function MarkedLabel({ label, typed }: { label: string; typed: string }) {
  return (
    <span className="text-pretty">
      {markedParts(label, typedWords(typed)).map((part, index) =>
        part.marked ? (
          <mark
            // oxlint-disable-next-line react/no-array-index-key -- a part is its place in the label, which is made again whole
            key={index}
            className="bg-transparent font-medium text-foreground underline underline-offset-2"
          >
            {part.text}
          </mark>
        ) : (
          part.text
        ),
      )}
    </span>
  );
}
