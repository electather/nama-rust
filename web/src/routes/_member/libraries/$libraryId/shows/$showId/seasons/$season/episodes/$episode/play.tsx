import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound } from "@tanstack/react-router";

import { Player } from "@/components/player";
import { NotFound } from "@/components/route-error";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import {
  externalIdFromParam,
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
      libraryId: BigInt(wholeNumberFromParam(params.libraryId)),
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
  component: PlayEpisode,
});

function PlayEpisode() {
  const { input, episodeNumber } = Route.useLoaderData();
  const { episodes } = useSuspenseQuery(BrowseService.method.listEpisodes, input).data;
  const episode = episodes.find((each) => each.episodeNumber === episodeNumber);
  const { version } = Route.useSearch();
  const { libraryId, showId, season } = Route.useParams();
  // An Event may have removed it since the loader ran.
  if (!episode) return <NotFound />;
  return (
    <Player
      title={episode.title}
      versionId={versionToPlay(episode.versions, version).id}
      titleHref={`/libraries/${libraryId}/shows/${showId}?season=${season}`}
    />
  );
}
