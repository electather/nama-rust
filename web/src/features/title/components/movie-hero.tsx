import { useState } from "react";

import { MovieMoreLoader } from "@/features/title/components/movie-more-loader";
import { Play } from "@/features/title/components/play";
import { TitleTop } from "@/features/title/components/title-top";
import { useMovieElsewhere } from "@/features/title/data/use-movie-elsewhere";
import { usePlayFrom } from "@/features/title/hooks/use-play-from";
import { titleBadges } from "@/features/title/lib/title-badges";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { runtimeOf, yearOf } from "@/shared/lib/title-facts";
import { versionToPlay } from "@/shared/lib/version-to-play";

/**
 * The top of a Movie's page, which says what it is and plays it, at the Hub from whichever of the
 * Account's Servers has it best. `params` are the page's URL, which Play links from.
 */
export function MovieHero({
  libraryId,
  movieId,
  movie,
  readOnlyFolder,
  params,
}: {
  libraryId: bigint;
  movieId: ExternalId;
  movie: Movie;
  readOnlyFolder: string | undefined;
  params: { libraryId: string; movieId: string };
}) {
  const { versions, watchState } = movie;
  const playFrom = usePlayFrom({ libraryId, versions, watchState }, useMovieElsewhere(movieId));
  const { choice } = playFrom;
  const [chosen, setChosen] = useState<number>();
  // The largest until one is chosen, and again if an Event removed the chosen one.
  const version = versionToPlay(choice.versions, chosen);
  return (
    <TitleTop
      title={movie}
      kind="Movie"
      id={movieId}
      logoUrl={movie.titleLogoUrl}
      facts={[yearOf(movie.releaseDate), runtimeOf(movie.runtimeMinutes, "short")]}
      boxed={titleBadges(movie.versions)}
      actions={<Play movieId={params.movieId} choice={choice} version={version} />}
      more={
        <MovieMoreLoader
          {...{ libraryId, movieId, movie, readOnlyFolder, version }}
          versions={choice.versions}
          onChoose={(versionId) => {
            // A Version is its Server's own, so picking one keeps Play there when a Server with a
            // bigger one answers later.
            if (choice.server) {
              playFrom.onChoose(choice.server.id);
            }
            setChosen(versionId);
          }}
          playFrom={{
            ...playFrom,
            // Version ids are each Server's own, so a choice made on one names nothing on another.
            onChoose: (serverId) => {
              playFrom.onChoose(serverId);
              setChosen(undefined);
            },
          }}
        />
      }
    />
  );
}
