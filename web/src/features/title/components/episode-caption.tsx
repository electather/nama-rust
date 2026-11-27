import { EpisodeProgress } from "@/features/title/components/episode-progress";
import { foundEpisodeStatus } from "@/features/title/lib/found-episode-label";
import type { RowEpisode } from "@/features/title/lib/show-row";
import { episodeName } from "@/shared/lib/title-facts";

/**
 * An Episode card's caption over its Still: which Episode, its title, muted while it's missing
 * from the Server, and synopsis, then its time in the page's Library, else where it is.
 */
export function EpisodeCaption({ item }: { item: RowEpisode }) {
  const { episode } = item;
  const muted = item.kind === "missing" ? "text-muted-foreground" : "";
  return (
    <>
      <span className="text-sm/5 text-foreground/80">
        {episodeName(item.seasonNumber, item.episodeNumber, "inSeason")}
      </span>
      <span className={`truncate leading-6 font-semibold ${muted}`}>{episode.title}</span>
      {episode.overview !== undefined && (
        <span className="line-clamp-3 text-sm/5 text-pretty text-foreground/80">
          {episode.overview}
        </span>
      )}
      {item.kind === "here" ? (
        <EpisodeProgress episode={item.episode} />
      ) : (
        <span className="mt-1 text-sm text-muted-foreground tabular-nums">
          {foundEpisodeStatus(item)}
        </span>
      )}
    </>
  );
}
