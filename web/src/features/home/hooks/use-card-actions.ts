import { useState } from "react";

import { useSetHiddenFromContinueWatching } from "@/features/home/data/use-set-hidden-from-continue-watching";
import type { Hiding } from "@/features/home/lib/hiding";
import type { Marking } from "@/features/home/lib/marking";
import { useSetPlayed } from "@/shared/hooks/use-set-played";

/** What a Continue Watching card's menu last did, for its notice to say. */
export type Done = { kind: "removed"; hiding: Hiding } | { kind: "watched"; marking: Marking };

/**
 * What a Continue Watching card's menu does: `markWatched` a card, `remove` one, and `undo` that,
 * with what it last `done`.
 */
export function useCardActions() {
  const setHidden = useSetHiddenFromContinueWatching();
  const setPlayed = useSetPlayed();
  const [done, setDone] = useState<Done>();
  const hide = ({ libraryId, target }: Hiding, hidden: boolean, then?: Done) => {
    setHidden(
      { libraryId, target, hidden },
      {
        onSuccess: () => {
          setDone(then);
        },
      },
    );
  };
  return {
    done,
    markWatched: (marking: Marking) => {
      const { libraryId, target } = marking;
      setPlayed(
        { libraryId, target, played: true },
        {
          onSuccess: () => {
            setDone({ kind: "watched", marking });
          },
        },
      );
    },
    remove: (hiding: Hiding) => {
      hide(hiding, true, { kind: "removed", hiding });
    },
    undo: (hiding: Hiding) => {
      hide(hiding, false);
    },
  };
}
