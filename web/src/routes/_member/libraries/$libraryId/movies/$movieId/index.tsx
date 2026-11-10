import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";
import { type ReactNode, useState } from "react";

import { NotFound } from "@/components/route-error";
import { TitlePage } from "@/components/title-page";
import { buttonVariants } from "@/components/ui/button";
import { PlayedButton, WatchedBar } from "@/components/watch-state";
import { BrowseService, type Version } from "@/gen/nama/v1/browse_pb";
import {
  metadataLine,
  loadMovie,
  runtimeOf,
  sizeOf,
  versionLine,
  versionToPlay,
  yearOf,
} from "@/lib/browse";
import { useSetPlayed } from "@/lib/watch-state";

export const Route = createFileRoute("/_member/libraries/$libraryId/movies/$movieId/")({
  loader: ({ context, params }) => loadMovie(context, params),
  notFoundComponent: NotFound,
  component: Movie,
});

function Movie() {
  const input = Route.useLoaderData();
  const { data } = useSuspenseQuery(BrowseService.method.getMovie, input);
  const setPlayed = useSetPlayed();
  const [chosen, setChosen] = useState<number>();
  const { movie } = data;
  if (!movie) return <NotFound />;
  // The largest until one is chosen, and again if an Event removed the chosen one.
  const version = versionToPlay(movie.versions, chosen);
  return (
    <TitlePage title={movie.title} backdropUrl={movie.backdropUrl}>
      <p className="text-sm text-muted-foreground">
        {metadataLine(
          yearOf(movie.releaseDate),
          runtimeOf(movie.runtimeMinutes),
          versionLine(version),
        )}
      </p>
      <div className="mt-4 max-w-xs empty:hidden">
        <WatchedBar state={movie.watchState} />
      </div>
      <Play versions={movie.versions} version={version} onChoose={setChosen}>
        <PlayedButton
          played={movie.watchState?.played === true}
          onToggle={(played) => {
            setPlayed({
              libraryId: input.libraryId,
              target: { case: "movieId", value: input.movieId },
              played,
            });
          }}
        />
      </Play>
      {movie.overview !== undefined && (
        <p className="mt-8 max-w-prose text-pretty">{movie.overview}</p>
      )}
    </TitlePage>
  );
}

/**
 * Play `version`, and a choice of Version, each with its picture and sound, when there's more
 * than one, then `children`.
 */
function Play({
  versions,
  version,
  onChoose,
  children,
}: {
  versions: Version[];
  version: Version;
  onChoose: (versionId: number) => void;
  children: ReactNode;
}) {
  const versionId = String(version.id);
  return (
    <div className="mt-6 flex flex-wrap items-center gap-x-4 gap-y-3">
      <Link
        from={Route.fullPath}
        to="./play"
        search={{ version: Number(versionId) }}
        className={buttonVariants()}
      >
        Play
      </Link>
      {versions.length > 1 && (
        <label className="flex items-center gap-2 text-sm text-muted-foreground">
          Version
          <select
            value={versionId}
            onChange={(event) => {
              onChoose(Number(event.target.value));
            }}
            className="h-9 max-w-64 truncate rounded-lg border border-input bg-transparent px-3 text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25"
          >
            {versions.map((option) => (
              <option key={String(option.id)} value={String(option.id)}>
                {option.fileName} ({metadataLine(versionLine(option), sizeOf(option.size))})
              </option>
            ))}
          </select>
        </label>
      )}
      {children}
    </div>
  );
}
