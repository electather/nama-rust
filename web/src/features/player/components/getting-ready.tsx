import { useEffect, useState } from "react";

import { TopBar } from "@/features/player/components/top-bar";
import type { PlayerProps } from "@/features/player/lib/player-props";

// React keeps a Suspense fallback up for at least 300 ms, so without this wait a quick plan, as most
// later ones are, would flash this and have it announced. Shorter, it is also the status
// `player-limit.test.tsx` finds during a quick plan, where it expects `DroppedChoice`'s or none.
const NOTICEABLE_WAIT_MS = 1000;

/** The title bar, and once the wait is noticeable, that the Server is getting the Version ready. */
export function GettingReady({ title, back }: Pick<PlayerProps, "title" | "back">) {
  const [noticeable, setNoticeable] = useState(false);
  useEffect(() => {
    const timer = setTimeout(() => {
      setNoticeable(true);
    }, NOTICEABLE_WAIT_MS);
    return () => {
      clearTimeout(timer);
    };
  }, []);
  return (
    <div className="fixed inset-0 flex flex-col bg-background">
      <TopBar title={title} back={back} />
      {noticeable && (
        <output className="m-auto px-6 text-muted-foreground">Getting this ready…</output>
      )}
    </div>
  );
}
