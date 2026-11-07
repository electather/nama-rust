import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute, notFound } from "@tanstack/react-router";

import { NotFound } from "@/components/route-error";
import { TitlePage } from "@/components/title-page";
import { MarkPlayedMenu } from "@/components/mark-played-menu";
import { PlayedIconButton, WatchProgress } from "@/components/watch-state";
import { BrowseService, type Episode, type Season } from "@/gen/nama/v1/browse_pb";
import {
  dateOf,
  externalIdFromParam,
  libraryFromParam,
  metadataLine,
  queryOrNotFound,
  runtimeOf,
  seasonName,
} from "@/lib/browse";
import { useSetPlayed } from "@/lib/watch-state";

export const Route = createFileRoute("/_member/libraries/$libraryId/shows/$showId/")({
  validateSearch: (search): { season?: number } => {
    const season = Number(search["season"]);
    return Number.isInteger(season) && season >= 0 ? { season } : {};
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
  const { data } = useSuspenseQuery(BrowseService.method.getShow, show);
  const { data: season } = useSuspenseQuery(BrowseService.method.listEpisodes, {
    ...show,
    seasonNumber,
  });
  if (!data.show) return <NotFound />;
  return (
    <TitlePage title={data.show.title} backdropUrl={data.show.backdropUrl}>
      <div className="flex max-w-3xl items-start gap-2">
        <Seasons seasons={data.seasons} current={seasonNumber} />
        <MarkPlayedMenu
          {...show}
          seasonNumber={seasonNumber}
          seasonPlayed={season.episodes.every(({ watchState }) => watchState?.played === true)}
        />
      </div>
      <Episodes />
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

/** An Episode, which plays its largest Version, with your Watch State and a toggle for it. */
function EpisodeRow({ libraryId, episode }: { libraryId: bigint; episode: Episode }) {
  const play = `${location.pathname}/seasons/${episode.seasonNumber}/episodes/${episode.episodeNumber}/play`;
  const setPlayed = useSetPlayed();
  return (
    <li className="flex items-center gap-1 border-b last:border-b-0">
      {/* ponytail: a plain link until the player's route (#72) exists to type a <Link> against. */}
      <a
        href={play}
        className="block min-w-0 flex-1 rounded-lg px-3 py-4 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <span className="font-medium">
          {episode.episodeNumber}. {episode.title}
        </span>
        <span className="mt-0.5 block text-sm text-muted-foreground">
          {metadataLine(dateOf(episode.airDate), runtimeOf(episode.runtimeMinutes))}
        </span>
        {episode.overview !== undefined && (
          <span className="mt-2 block max-w-prose text-sm text-pretty text-muted-foreground">
            {episode.overview}
          </span>
        )}
        <span className="mt-3 block max-w-xs empty:hidden">
          <WatchProgress state={episode.watchState} />
        </span>
      </a>
      <PlayedIconButton
        played={episode.watchState?.played === true}
        label={`Played: ${episode.title}`}
        onToggle={(played) => {
          setPlayed({ libraryId, target: { case: "episodeId", value: episode.id ?? {} }, played });
        }}
      />
    </li>
  );
}
