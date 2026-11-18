import type { MessageInitShape } from "@bufbuild/protobuf";
import { useQuery, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound } from "@tanstack/react-router";

import { type NextEpisode, streakSearch } from "@/components/next-episode";
import { Player } from "@/components/player";
import { DeletedWhilePlaying, NotFound, PlayerError } from "@/components/route-error";
import {
  BrowseService,
  type Episode,
  type GetNextEpisodeRequestSchema,
} from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import {
  episodeName,
  externalIdFromParam,
  isGone,
  libraryFromParam,
  metadataLine,
  queryOrNotFound,
  versionSearch,
  versionToPlay,
  wholeNumberFromParam,
} from "@/lib/browse";

export const Route = createFileRoute(
  "/_member/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
)({
  validateSearch: (search) => ({ ...versionSearch(search), ...streakSearch(search) }),
  loader: async ({ context, params }) => {
    const input = {
      libraryId: (await libraryFromParam(context, params.libraryId)).id,
      showId: externalIdFromParam(params.showId),
      seasonNumber: wholeNumberFromParam(params.season),
    };
    const { episodes } = await queryOrNotFound(context, BrowseService.method.listEpisodes, input);
    const episodeNumber = wholeNumberFromParam(params.episode);
    if (!episodes.some((episode) => episode.episodeNumber === episodeNumber)) {
      throw notFound();
    }
    return { input, episodeNumber };
  },
  staticData: { fullScreen: true },
  notFoundComponent: NotFound,
  errorComponent: PlayerError,
  component: PlayEpisode,
});

function PlayEpisode() {
  const { input, episodeNumber } = Route.useLoaderData();
  const { data, error } = useSuspenseQuery(BrowseService.method.listEpisodes, input);
  const episode = data.episodes.find((each) => each.episodeNumber === episodeNumber);
  const { version, streak } = Route.useSearch();
  const { libraryId, showId } = Route.useParams();
  const next = useAutoplayedEpisode({ ...input, episodeNumber });
  const lockScreen = useShowForLockScreen(input, episodeNumber);
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
      versionId={id}
      tracks={tracks}
      watchState={episode.watchState}
      {...lockScreen}
      back={{
        to: "/libraries/$libraryId/shows/$showId",
        params: { libraryId, showId },
        search: { season: input.seasonNumber },
      }}
      next={nextEpisode(next, streak, { libraryId, showId })}
    />
  );
}

/** The Episode after the one `episode` names, if the User has autoplay on. */
function useAutoplayedEpisode(episode: MessageInitShape<typeof GetNextEpisodeRequestSchema>) {
  // Neither suspends: the Episode plays while the Server finds the next one.
  const next = useQuery(BrowseService.method.getNextEpisode, episode).data?.next;
  // Off until the Server says it's on, so a User who turned it off never sees a countdown.
  const on = useQuery(PlaybackService.method.getAutoplay, {}).data?.on === true;
  return on ? next : undefined;
}

/**
 * What the lock screen shows beside the Episode's title: its Show and which Episode it is, by the
 * Show's Poster. Nothing until the Show arrives, which the Episode doesn't wait for.
 */
function useShowForLockScreen(
  {
    libraryId,
    showId,
    seasonNumber,
  }: { libraryId: bigint; showId: ExternalId; seasonNumber: number },
  episodeNumber: number,
) {
  const { data } = useQuery(BrowseService.method.getShow, { libraryId, showId });
  const show = data?.show;
  return (
    show && {
      artist: metadataLine(show.title, episodeName(seasonNumber, episodeNumber, "short")),
      posterUrl: show.posterUrl,
    }
  );
}

/** What the player autoplays next: `episode` of the Show `params` names, after one autoplay `streak` opened. */
function nextEpisode(
  episode: Episode | undefined,
  streak: number | undefined,
  { libraryId, showId }: { libraryId: string; showId: string },
): NextEpisode | undefined {
  if (!episode) {
    return undefined;
  }
  return {
    episode,
    streak: streak ?? 0,
    link: (nextStreak) => ({
      to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
      params: {
        libraryId,
        showId,
        season: String(episode.seasonNumber),
        episode: String(episode.episodeNumber),
      },
      search: { streak: nextStreak },
    }),
  };
}
