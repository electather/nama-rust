import { EpisodeRowSkeleton } from "@/features/player/components/episode-row-skeleton";

/** Where Episode rows will be while they load. */
export function EpisodeRowsSkeleton() {
  return (
    <div aria-busy className="flex flex-col gap-1.5 p-2 pt-1">
      <span className="sr-only">Loading Episodes</span>
      <EpisodeRowSkeleton />
      <EpisodeRowSkeleton />
      <EpisodeRowSkeleton />
    </div>
  );
}
