import type { EpisodeTakenAlong } from "@/gen/nama/v1/library_pb";
import { episodeName } from "@/shared/lib/title-facts";

/** The other Episodes the files that go hold, which lose them too. */
export function TakenAlong({ episodes }: { episodes: EpisodeTakenAlong[] }) {
  if (episodes.length === 0) {
    return null;
  }
  return (
    <div>
      <p>A file that goes also holds:</p>
      <ul className="mt-2 flex flex-col gap-1 text-muted-foreground">
        {episodes.map((episode) => (
          <li key={`${String(episode.seasonNumber)} ${String(episode.episodeNumber)}`}>
            {episodeName(episode.seasonNumber, episode.episodeNumber, "full")}: {episode.title}
          </li>
        ))}
      </ul>
    </div>
  );
}
