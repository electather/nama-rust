import { useEffect, useState } from "react";

import { Spinner } from "@/features/player/components/spinner";
import { TopBar } from "@/features/player/components/top-bar";
import type { PlayerProps } from "@/features/player/lib/player-props";
import { useServerUrl } from "@/shared/hooks/use-server-url";

// React keeps a Suspense fallback up for at least 300 ms, so without this wait a quick plan, as most
// later ones are, would flash this and have it announced. Shorter, it is also the status
// `player-limit.test.tsx` finds during a quick plan, where it expects `DroppedChoice`'s or none.
const NOTICEABLE_WAIT_MS = 1000;

/**
 * The title bar over the title's art under a scrim, and once the wait is noticeable, that the Server
 * is getting the Version ready.
 */
export function GettingReady({
  title,
  heading,
  line,
  back,
  backdropUrl,
}: Pick<PlayerProps, "title" | "heading" | "line" | "back" | "backdropUrl">) {
  const backdrop = useServerUrl(backdropUrl);
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
      {backdropUrl !== undefined && (
        <img src={backdrop} alt="" className="absolute inset-0 size-full object-cover" />
      )}
      <div aria-hidden className="absolute inset-0 bg-scrim" />
      <div className="relative">
        <TopBar title={heading ?? title} line={line} back={back} shaded={false} />
      </div>
      {noticeable && (
        <output className="relative m-auto flex flex-col items-center gap-3.5 px-6 text-sm">
          <Spinner />
          Getting this ready…
        </output>
      )}
    </div>
  );
}
