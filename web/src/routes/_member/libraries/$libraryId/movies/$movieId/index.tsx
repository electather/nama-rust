import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";
import { type ReactNode, useState } from "react";

import { NotFound } from "@/components/route-error";
import { TitlePage } from "@/components/title-page";
import { buttonVariants } from "@/components/ui/button";
import { PlayedButton, WatchedBar } from "@/components/watch-state";
import { BrowseService, type Version } from "@/gen/nama/v1/browse_pb";
import { largestVersion, metadataLine, loadMovie, runtimeOf, sizeOf, yearOf } from "@/lib/browse";
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
  const { movie } = data;
  if (!movie) return <NotFound />;
  return (
    <TitlePage title={movie.title} backdropUrl={movie.backdropUrl}>
      <p className="text-sm text-muted-foreground">
        {metadataLine(yearOf(movie.releaseDate), runtimeOf(movie.runtimeMinutes))}
      </p>
      <div className="mt-4 max-w-xs empty:hidden">
        <WatchedBar state={movie.watchState} />
      </div>
      <Play versions={movie.versions}>
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
 * Play, and a choice of Version when there's more than one, starting on the largest, then
 * `children`.
 */
function Play({ versions, children }: { versions: Version[]; children: ReactNode }) {
  const [chosenId, setChosenId] = useState<string>();
  // The largest until one is chosen, and again if an Event removed the chosen one.
  const versionId = String(
    (versions.find(({ id }) => String(id) === chosenId) ?? largestVersion(versions)).id,
  );
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
              setChosenId(event.target.value);
            }}
            className="h-9 max-w-64 truncate rounded-lg border border-input bg-transparent px-3 text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25"
          >
            {versions.map((version) => (
              <option key={String(version.id)} value={String(version.id)}>
                {version.fileName} ({sizeOf(version.size)})
              </option>
            ))}
          </select>
        </label>
      )}
      {children}
    </div>
  );
}
