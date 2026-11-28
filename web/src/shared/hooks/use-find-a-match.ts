import { useState } from "react";

import { useDebounced } from "@/shared/hooks/use-debounced";
import { useSearchTitles } from "@/shared/hooks/use-search-titles";
import { useSetMatch } from "@/shared/hooks/use-set-match";
import {
  type ToMatch,
  candidateKey,
  matchTarget,
  notInShow,
  setMatchRefusal,
  yearTyped,
} from "@/shared/lib/find-a-match";

/** How long typing pauses before Find a Match searches for it. */
const TYPING_PAUSE_MS = 300;

/**
 * Find a Match's state for `toMatch`: the Title and Year typed, searched for once typing pauses,
 * the titles found, the one picked (the first until another is), and Matching to it.
 */
export function useFindAMatch({ item, libraryId, onMatched }: ToMatch) {
  const [typed, setTyped] = useState({
    title: item.title,
    year: item.year === undefined ? "" : String(item.year),
  });
  const settled = useDebounced(typed, TYPING_PAUSE_MS);
  const title = settled.title.trim();
  const found = useSearchTitles({
    kind: item.kind,
    title,
    year: yearTyped(settled.year),
    libraryId,
  });
  const [chosen, setChosen] = useState<string>();
  const candidates = title === "" ? [] : (found.data?.candidates ?? []);
  const picked =
    candidates.find((candidate) => candidateKey(candidate) === chosen) ?? candidates[0];
  const setMatch = useSetMatch(libraryId);
  return {
    typed,
    setTyped,
    found: { ...found, candidates, blank: title === "" },
    picked,
    setChosen,
    matching: setMatch.isPending,
    notice: setMatch.data && "itemId" in item ? notInShow(item, setMatch.data) : "",
    refusal: setMatch.error ? setMatchRefusal(setMatch.error) : undefined,
    match: () => {
      if (picked !== undefined) {
        setMatch.mutate(
          { ...matchTarget(item), titleId: picked.id },
          {
            onSuccess: (matched) => {
              onMatched?.(picked.id, matched);
            },
          },
        );
      }
    },
  };
}
