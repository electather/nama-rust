import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute, notFound } from "@tanstack/react-router";

import { NotFound } from "@/components/route-error";
import { TitlePage } from "@/components/title-page";
import { BrowseService, type Episode, type Season } from "@/gen/nama/v1/browse_pb";
import {
  dateOf,
  externalIdFromParam,
  libraryFromParam,
  metadataLine,
  queryOrNotFound,
  runtimeOf,
} from "@/lib/browse";

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
  if (!data.show) return <NotFound />;
  return (
    <TitlePage title={data.show.title} backdropUrl={data.show.backdropUrl}>
      <Seasons seasons={data.seasons} current={seasonNumber} />
      <Episodes />
    </TitlePage>
  );
}

function seasonName(number: number) {
  return number === 0 ? "Specials" : `Season ${number}`;
}

/** Links to the Show's Seasons, one shown at a time. With only one, it names it. */
function Seasons({ seasons, current }: { seasons: Season[]; current: number }) {
  return (
    // Pulled out by the links' own padding, so the first lines up with the title.
    <nav aria-label="Seasons" className="-mx-3 flex flex-wrap gap-1">
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
          <EpisodeRow key={episode.episodeNumber} episode={episode} />
        ))}
      </ol>
    </section>
  );
}

/** An Episode, which plays its largest Version. */
function EpisodeRow({ episode }: { episode: Episode }) {
  return (
    <li className="border-b last:border-b-0">
      <Link
        from={Route.fullPath}
        to="./seasons/$season/episodes/$episode/play"
        params={{ season: String(episode.seasonNumber), episode: String(episode.episodeNumber) }}
        className="block rounded-lg px-3 py-4 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
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
      </Link>
    </li>
  );
}
