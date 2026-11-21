import { EpisodeProgress } from "@/features/title/components/episode-progress";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { episodeName } from "@/shared/lib/title-facts";

/** An Episode card's caption over its Still: which Episode, its title and synopsis, then its time. */
export function EpisodeCaption({ episode }: { episode: Episode }) {
  return (
    <>
      <span className="text-sm/5 text-foreground/80">
        {episodeName(episode.seasonNumber, episode.episodeNumber, "inSeason")}
      </span>
      <span className="truncate leading-6 font-semibold">{episode.title}</span>
      {episode.overview !== undefined && (
        <span className="line-clamp-3 text-sm/5 text-pretty text-foreground/80">
          {episode.overview}
        </span>
      )}
      <EpisodeProgress episode={episode} />
    </>
  );
}
