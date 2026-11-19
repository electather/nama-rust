import type { Episode } from "@/gen/nama/v1/browse_pb";
import { episodeName, metadataLine, runtimeOf } from "@/shared/lib/title-facts";

function stateOf(episode: Episode, playing: boolean) {
  if (playing) {
    return "Playing";
  }
  return episode.watchState?.played === true ? "Played" : undefined;
}

/** An Episode row's number, length and state over its title. */
export function EpisodeText({ episode, playing }: { episode: Episode; playing: boolean }) {
  return (
    <span className="flex min-w-0 flex-1 flex-col gap-0.5 text-sm">
      <span className="text-xs text-muted-foreground group-aria-[current=page]/row:text-primary">
        {metadataLine(
          episodeName(episode.seasonNumber, episode.episodeNumber, "inSeason"),
          runtimeOf(episode.runtimeMinutes, "short"),
          stateOf(episode, playing),
        )}
      </span>
      <span className="line-clamp-2 font-semibold text-balance group-aria-[current=page]/row:text-primary">
        {episode.title}
      </span>
    </span>
  );
}
