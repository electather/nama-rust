import { Overview } from "@/features/title/components/overview";
import { TitleActions } from "@/features/title/components/title-actions";
import { TitleFacts } from "@/features/title/components/title-facts";
import { TitleGenres } from "@/features/title/components/title-genres";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import { TitleStarring } from "@/features/title/components/title-starring";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import type { FoundMovie } from "@/gen/nama/v1/request_pb";
import { FoundMovieState } from "@/shared/components/found-movie-state/found-movie-state";
import { WatchlistButton } from "@/shared/components/watchlist-button";
import { runtimeOf } from "@/shared/lib/title-facts";

/**
 * The top of a Movie's page that isn't on the Server: its title over its Backdrop, its overview and
 * facts, then Request, or where its Request is, for the signed-in User `you`, and the Watchlist's
 * button for the Movie `movieId`.
 */
export function MovieRequestHero({
  movieId,
  movie,
  you,
}: {
  movieId: ExternalId;
  movie: FoundMovie;
  you: bigint | undefined;
}) {
  return (
    <TitleHero backdrop={{ url: movie.backdropUrl, thumbhash: movie.backdropThumbhash }}>
      <TitleName title={movie.title} logoUrl={undefined} />
      <TitleGenres kind="Movie" genres={movie.genres} />
      {movie.overview !== undefined && <Overview text={movie.overview} />}
      <TitleStarring cast={movie.cast} />
      <TitleFacts
        score={movie.score}
        facts={[movie.year?.toString(), runtimeOf(movie.runtimeMinutes, "short")]}
        boxed={[movie.contentRating]}
      />
      <TitleActions>
        <FoundMovieState movie={movie} you={you} primary />
        <WatchlistButton
          watchlist={{ kind: TitleKind.MOVIE, id: movieId, on: movie.onWatchlist }}
        />
      </TitleActions>
    </TitleHero>
  );
}
