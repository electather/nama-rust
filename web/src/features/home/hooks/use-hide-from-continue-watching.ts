import { useState } from "react";

import { useSetHiddenFromContinueWatching } from "@/features/home/data/use-set-hidden-from-continue-watching";
import type { Hiding } from "@/features/home/lib/hiding";

/** Hiding titles from Continue Watching: the one `lastHidden`, to `hide` one, and to `undo` it. */
export function useHideFromContinueWatching() {
  const setHidden = useSetHiddenFromContinueWatching();
  const [lastHidden, setLastHidden] = useState<Hiding>();
  const ask = ({ libraryId, target }: Hiding, hidden: boolean, then: () => void) => {
    setHidden({ libraryId, target, hidden }, { onSuccess: then });
  };
  return {
    lastHidden,
    hide: (hiding: Hiding) => {
      ask(hiding, true, () => {
        setLastHidden(hiding);
      });
    },
    undo: (hiding: Hiding) => {
      ask(hiding, false, () => {
        setLastHidden(undefined);
      });
    },
  };
}
