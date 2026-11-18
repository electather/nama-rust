import { Link } from "@tanstack/react-router";

import { dateOf } from "@/features/title/lib/date-of";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { versionToPlay } from "@/lib/browse";
import { Still } from "@/shared/components/still";
import { WatchedBar } from "@/shared/components/watch-state/watched-bar";
import { posterLink } from "@/shared/lib/poster-classes";
import { episodeName, metadataLine, runtimeOf } from "@/shared/lib/title-facts";
import { versionLine } from "@/shared/lib/version-facts";

/** An Episode's Still, number, title, synopsis, facts and progress, as a link that plays it. */
export function EpisodeLink({ episode }: { episode: Episode }) {
  return (
    <Link
      from="/libraries/$libraryId/shows/$showId/"
      to="./seasons/$season/episodes/$episode/play"
      params={{ season: String(episode.seasonNumber), episode: String(episode.episodeNumber) }}
      // Grows so the toggles under the cards in a row line up.
      className={`${posterLink} flex-1`}
    >
      <Still url={episode.stillUrl} thumbhash={episode.stillThumbhash} />
      <span className="mt-3 block text-sm text-muted-foreground">
        {episodeName(episode.seasonNumber, episode.episodeNumber, "inSeason")}
      </span>
      <span className="block font-medium text-balance">{episode.title}</span>
      {episode.overview !== undefined && (
        <span className="mt-1 line-clamp-3 text-sm text-pretty text-muted-foreground">
          {episode.overview}
        </span>
      )}
      <span className="mt-1 block text-sm text-muted-foreground">
        {metadataLine(
          dateOf(episode.airDate),
          runtimeOf(episode.runtimeMinutes),
          versionLine(versionToPlay(episode.versions)),
        )}
      </span>
      <span className="mt-2 block empty:hidden">
        <WatchedBar state={episode.watchState} />
      </span>
    </Link>
  );
}
