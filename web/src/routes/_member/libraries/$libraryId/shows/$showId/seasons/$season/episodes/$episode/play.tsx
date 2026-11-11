import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound } from "@tanstack/react-router";

import { Player } from "@/components/player";
import { DeletedWhilePlaying, NotFound, PlayerError } from "@/components/route-error";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import {
  externalIdFromParam,
  isGone,
  libraryFromParam,
  queryOrNotFound,
  versionSearch,
  versionToPlay,
  wholeNumberFromParam,
} from "@/lib/browse";

export const Route = createFileRoute(
  "/_member/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
)({
  validateSearch: versionSearch,
  loader: async ({ context, params }) => {
    const input = {
      libraryId: (await libraryFromParam(context, params.libraryId)).id,
      showId: externalIdFromParam(params.showId),
      seasonNumber: wholeNumberFromParam(params.season),
    };
    const { episodes } = await queryOrNotFound(context, BrowseService.method.listEpisodes, input);
    const episodeNumber = wholeNumberFromParam(params.episode);
    if (!episodes.some((episode) => episode.episodeNumber === episodeNumber)) throw notFound();
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
  const { version } = Route.useSearch();
  const { libraryId, showId } = Route.useParams();
  // An Event removed it, or its whole Season, since the loader ran.
  if (!episode || isGone(error)) return <DeletedWhilePlaying />;
  const { id, tracks } = versionToPlay(episode.versions, version);
  return (
    <Player
      title={episode.title}
      versionId={id}
      tracks={tracks}
      watchState={episode.watchState}
      back={{
        to: "/libraries/$libraryId/shows/$showId",
        params: { libraryId, showId },
        search: { season: input.seasonNumber },
      }}
    />
  );
}
