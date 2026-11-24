import { DeletedWhilePlaying } from "@/features/player/components/deleted-while-playing";
import { Player } from "@/features/player/components/player";
import { useEpisodeAfter } from "@/features/player/data/use-episode-after";
import { useEpisodeBefore } from "@/features/player/data/use-episode-before";
import { useEpisodeToPlay } from "@/features/player/data/use-episode-to-play";
import { useAutoplayedEpisode } from "@/features/player/hooks/use-autoplayed-episode";
import { useEpisodeHeading } from "@/features/player/hooks/use-episode-heading";
import { useShowForLockScreen } from "@/features/player/hooks/use-show-for-lock-screen";
import { nextEpisode, playLink } from "@/features/player/lib/next-episode";
import type { PlayedSeason } from "@/features/player/lib/played-season";
import { isGone } from "@/shared/hooks/browse";
import { versionToPlay } from "@/shared/lib/version-to-play";

/**
 * Plays an Episode's Version, `version` if it names one and else the largest, from where you
 * stopped, and counts down to the next Episode if the User has autoplay on. `streak` is how many
 * Episodes in a row autoplay opened, ending with this one.
 */
export function EpisodePlayer({
  input,
  episodeNumber,
  version,
  streak,
  params,
}: {
  input: PlayedSeason;
  episodeNumber: number;
  version: number | undefined;
  streak: number | undefined;
  params: { libraryId: string; showId: string };
}) {
  const { episode, error } = useEpisodeToPlay(input, episodeNumber);
  const { libraryId, showId } = params;
  const next = useAutoplayedEpisode({ ...input, episodeNumber });
  const after = useEpisodeAfter({ ...input, episodeNumber });
  const before = useEpisodeBefore({ ...input, episodeNumber });
  const lockScreen = useShowForLockScreen(input, episodeNumber);
  const heading = useEpisodeHeading(input, episodeNumber, episode?.title);
  // An Event removed it, or its whole Season, since the loader ran.
  if (!episode || isGone(error)) {
    return <DeletedWhilePlaying />;
  }
  const { id, tracks } = versionToPlay(episode.versions, version);
  return (
    <Player
      // The router keeps this route mounted when autoplay opens another Episode in it, which needs
      // a player of its own.
      key={`${input.seasonNumber}/${episodeNumber}`}
      title={episode.title}
      {...heading}
      versionId={id}
      tracks={tracks}
      versions={episode.versions}
      watchState={episode.watchState}
      backdropUrl={episode.stillUrl}
      {...lockScreen}
      back={{
        to: "/libraries/$libraryId/shows/$showId",
        params: { libraryId, showId },
        search: { season: input.seasonNumber },
      }}
      next={nextEpisode(next, streak, { libraryId, showId })}
      inShow={{
        season: input,
        episodeNumber,
        after,
        before,
        link: (each) => playLink(each, { libraryId, showId }),
      }}
    />
  );
}
