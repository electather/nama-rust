import { foundEpisodeStatus } from "@/features/title/lib/found-episode-label";
import type { FoundRowEpisode } from "@/features/title/lib/show-row";
import { episodeName } from "@/shared/lib/title-facts";

/**
 * A found Episode card's caption over its Still, as a Library Episode's is: which Episode, its
 * title and synopsis, then where it is, muted while it's missing from the Server.
 */
export function FoundEpisodeCaption({ item }: { item: FoundRowEpisode }) {
  const { episode } = item;
  const missing = item.kind === "missing";
  return (
    <>
      <span className="text-sm/5 text-foreground/80">
        {episodeName(item.seasonNumber, item.episodeNumber, "inSeason")}
      </span>
      <span
        className={`truncate leading-6 font-semibold ${missing ? "text-muted-foreground" : ""}`}
      >
        {episode.title}
      </span>
      {episode.overview !== undefined && (
        <span className="line-clamp-3 text-sm/5 text-pretty text-foreground/80">
          {episode.overview}
        </span>
      )}
      <span className="mt-1 text-sm text-muted-foreground tabular-nums">
        {foundEpisodeStatus(item)}
      </span>
    </>
  );
}
