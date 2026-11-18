import { Link } from "@tanstack/react-router";

import type { EpisodeInLibrary } from "@/gen/nama/v1/browse_pb";
import { metadataLine, seasonName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** An Episode, opening its Show at its Season. Styled as the Show page's Episode rows. */
export function EpisodeFound({ episode }: { episode: EpisodeInLibrary }) {
  return (
    <li className="border-b last:border-b-0">
      <Link
        to="/libraries/$libraryId/shows/$showId"
        params={{ libraryId: String(episode.libraryId), showId: titleParam(episode.showId) }}
        search={{ season: episode.seasonNumber }}
        className="block rounded-lg px-3 py-4 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <span className="font-medium">
          {episode.episodeNumber}. {episode.title}
        </span>
        <span className="mt-0.5 block text-sm text-muted-foreground">
          {metadataLine(episode.showTitle, seasonName(episode.seasonNumber))}
        </span>
      </Link>
    </li>
  );
}
