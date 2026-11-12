import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute, notFound } from "@tanstack/react-router";

import { DeleteEpisode, DeleteFromShow } from "@/components/delete-from-show";
import { EditImages, showImages } from "@/components/edit-images";
import { NotFound } from "@/components/route-error";
import { TitlePage } from "@/components/title-page";
import { MarkPlayedMenu } from "@/components/mark-played-menu";
import { PlayedIconButton, WatchedBar } from "@/components/watch-state";
import { BrowseService, type Episode, type Season } from "@/gen/nama/v1/browse_pb";
import {
  type DeletedSearch,
  dateOf,
  deletedFromSearch,
  externalIdFromParam,
  isGone,
  libraryFromParam,
  metadataLine,
  queryOrNotFound,
  runtimeOf,
  seasonName,
  versionLine,
  versionToPlay,
} from "@/lib/browse";
// oxlint-disable-next-line import/max-dependencies -- the page puts together a Show's Seasons, Episodes, Watch State and what an admin may change
import { useSetPlayed } from "@/lib/watch-state";

export const Route = createFileRoute("/_member/libraries/$libraryId/shows/$showId/")({
  validateSearch: (search): { season?: number } & DeletedSearch => {
    const season = Number(search["season"]);
    const deleted = deletedFromSearch(search);
    return Number.isInteger(season) && season >= 0 ? { season, ...deleted } : deleted;
  },
  loaderDeps: ({ search }) => ({ season: search.season }),
  loader: async ({ context, params, deps }) => {
    const library = await libraryFromParam(context, params.libraryId);
    const show = { libraryId: library.id, showId: externalIdFromParam(params.showId) };
    const { seasons } = await queryOrNotFound(context, BrowseService.method.getShow, show);
    // A Season the Show doesn't have, or none asked for, means its first. Specials (0) come
    // first only when they're all there is.
    const season =
      seasons.find(({ number }) => number === deps.season) ??
      seasons.find(({ number }) => number > 0) ??
      seasons[0];
    if (!season) throw notFound();
    const episodes = { ...show, seasonNumber: season.number };
    await queryOrNotFound(context, BrowseService.method.listEpisodes, episodes);
    return episodes;
  },
  notFoundComponent: NotFound,
  component: Show,
});

function Show() {
  const { seasonNumber, ...show } = Route.useLoaderData();
  const { data, error } = useSuspenseQuery(BrowseService.method.getShow, show);
  const { data: season } = useSuspenseQuery(BrowseService.method.listEpisodes, {
    ...show,
    seasonNumber,
  });
  if (!data.show || isGone(error)) return <NotFound />;
  return (
    <TitlePage
      title={data.show.title}
      backdropUrl={data.show.backdropUrl}
      backdropThumbhash={data.show.backdropThumbhash}
    >
      {/* Pulled out like the Episodes below, so the menu lines up with their toggles. */}
      <div className="-me-3 flex max-w-3xl items-start gap-2">
        <Seasons seasons={data.seasons} current={seasonNumber} />
        <MarkPlayedMenu
          {...show}
          seasonNumber={seasonNumber}
          seasonPlayed={
            season.episodes.length > 0 &&
            season.episodes.every(({ watchState }) => watchState?.played === true)
          }
        />
      </div>
      <SeasonShown seasons={data.seasons} current={seasonNumber} />
      {/* Out of the way below the Episodes, and pulled out by the buttons' own padding. */}
      <div className="mt-6 -ms-4 flex max-w-3xl flex-wrap gap-2 empty:hidden">
        <EditImages
          title={data.show.title}
          images={showImages(
            show.showId,
            data.show,
            data.seasons.find(({ number }) => number === seasonNumber),
          )}
        />
        <DeleteFromShow
          {...show}
          title={data.show.title}
          seasons={data.seasons}
          current={seasonNumber}
        />
      </div>
    </TitlePage>
  );
}

/** Links to the Show's Seasons, one shown at a time. With only one, it names it. */
function Seasons({ seasons, current }: { seasons: Season[]; current: number }) {
  return (
    // Pulled out by the links' own padding, so the first lines up with the title.
    <nav aria-label="Seasons" className="-ms-3 flex flex-1 flex-wrap gap-1">
      {seasons.map(({ number }) => (
        <Link
          key={number}
          from={Route.fullPath}
          search={{ season: number }}
          aria-current={number === current ? "page" : undefined}
          className="rounded-lg px-3 py-2 text-sm font-medium text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:bg-muted aria-[current=page]:text-foreground"
        >
          {seasonName(number)}
        </Link>
      ))}
    </nav>
  );
}

/** The Season shown's Episodes, or that it's gone, once an Event says it was deleted. */
function SeasonShown({ seasons, current }: { seasons: Season[]; current: number }) {
  if (seasons.some(({ number }) => number === current)) return <Episodes />;
  return (
    <p className="mt-6 text-muted-foreground">
      {seasonName(current)} is no longer in this Library.
    </p>
  );
}

function Episodes() {
  const input = Route.useLoaderData();
  const { data } = useSuspenseQuery(BrowseService.method.listEpisodes, input);
  return (
    <section aria-label={`${seasonName(input.seasonNumber)} Episodes`} className="mt-6 max-w-3xl">
      <ol className="-mx-3">
        {data.episodes.map((episode) => (
          <EpisodeRow key={episode.episodeNumber} libraryId={input.libraryId} episode={episode} />
        ))}
      </ol>
    </section>
  );
}

/**
 * An Episode, which plays its largest Version, with that Version's picture and sound, your Watch
 * State and a toggle for it.
 */
function EpisodeRow({ libraryId, episode }: { libraryId: bigint; episode: Episode }) {
  const setPlayed = useSetPlayed();
  // Without its own ID there's nothing to mark or delete.
  const episodeId = episode.id;
  return (
    <li className="flex flex-wrap items-center gap-1 border-b last:border-b-0">
      <Link
        from={Route.fullPath}
        to="./seasons/$season/episodes/$episode/play"
        params={{ season: String(episode.seasonNumber), episode: String(episode.episodeNumber) }}
        className="block min-w-0 flex-1 rounded-lg px-3 py-4 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <span className="font-medium">
          {episode.episodeNumber}. {episode.title}
        </span>
        <span className="mt-0.5 block text-sm text-muted-foreground">
          {metadataLine(
            dateOf(episode.airDate),
            runtimeOf(episode.runtimeMinutes),
            versionLine(versionToPlay(episode.versions)),
          )}
        </span>
        {episode.overview !== undefined && (
          <span className="mt-2 block max-w-prose text-sm text-pretty text-muted-foreground">
            {episode.overview}
          </span>
        )}
        <span className="mt-3 block max-w-xs empty:hidden">
          <WatchedBar state={episode.watchState} />
        </span>
      </Link>
      {episodeId && (
        <>
          <PlayedIconButton
            played={episode.watchState?.played === true}
            label={`Played: ${episode.title}`}
            onToggle={(played) => {
              setPlayed({ libraryId, target: { case: "episodeId", value: episodeId }, played });
            }}
          />
          <DeleteEpisode libraryId={libraryId} episodeId={episodeId} episode={episode} />
        </>
      )}
    </li>
  );
}
