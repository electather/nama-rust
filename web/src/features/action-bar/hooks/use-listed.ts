import { useState } from "react";

import { useLate } from "@/features/action-bar/hooks/use-late";

/**
 * The words the list answers, `typed` once its search (for `words`, the debounced typing) is in. The
 * list stays as it was meanwhile, so nothing moves until the answer is in.
 */
export function useListed(typed: string, words: string, searching: boolean) {
  const [listed, setListed] = useState(typed);
  if (words === typed && !searching && listed !== typed) {
    setListed(typed);
  }
  return {
    listed,
    /** A search still unanswered a second after the typing: only then is it worth a spinner. */
    slow: useLate(listed !== typed, 1000),
    clear: () => {
      setListed("");
    },
  };
}
