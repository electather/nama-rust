import { createFileRoute, notFound } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { PlayerError } from "@/app/route-error/player-error";
import { EpisodePlayer } from "@/features/player/components/episode-player";
import { streakSearch } from "@/features/player/lib/next-episode";
import { versionSearch } from "@/features/player/lib/version-search";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import {
  externalIdFromParam,
  libraryFromParam,
  queryOrNotFound,
  wholeNumberFromParam,
} from "@/shared/hooks/browse";

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
  const { version, streak } = Route.useSearch();
  return (
    <EpisodePlayer
      input={input}
      episodeNumber={episodeNumber}
      version={version}
      streak={streak}
      params={Route.useParams()}
    />
  );
}
