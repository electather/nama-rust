import { type ComponentProps, Suspense, lazy } from "react";

import type { EpisodeList as List } from "@/features/player/components/episode-list";
import { EpisodeRowsSkeleton } from "@/features/player/components/episode-rows-skeleton";
import { ErrorBoundary } from "@/shared/components/error-boundary";

// Not on screen until the panel opens, so it stays out of the player's first code.
const EpisodeList = lazy(async () => {
  const module = await import("@/features/player/components/episode-list");
  return { default: module.EpisodeList };
});

const failed = (
  <p className="p-5 text-muted-foreground">
    Couldn’t load the Episodes. Check that the Server is running.
  </p>
);

/** The Episodes panel's list, its skeleton while its code and Seasons load, or that they failed. */
export function EpisodesLoader(props: ComponentProps<typeof List>) {
  return (
    <ErrorBoundary failed={failed}>
      <Suspense fallback={<EpisodeRowsSkeleton />}>
        <EpisodeList {...props} />
      </Suspense>
    </ErrorBoundary>
  );
}
