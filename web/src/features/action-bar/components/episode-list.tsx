import { EpisodeFound } from "@/features/action-bar/components/episode-found";
import type { EpisodeInLibrary } from "@/gen/nama/v1/browse_pb";
import { titleParam } from "@/shared/lib/title-param";

/** The Episodes a search found, as rows. */
export function EpisodeList({ episodes }: { episodes: EpisodeInLibrary[] }) {
  return (
    <ul className="-mx-3 mt-4 max-w-3xl">
      {episodes.map((episode) => (
        <EpisodeFound key={`${episode.libraryId}/${titleParam(episode.id)}`} episode={episode} />
      ))}
    </ul>
  );
}
