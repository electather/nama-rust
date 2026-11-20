import { Overview } from "@/features/title/components/overview";
import { TitleActions } from "@/features/title/components/title-actions";
import { TitleFacts } from "@/features/title/components/title-facts";
import { TitleGenres } from "@/features/title/components/title-genres";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import type { FoundMovie } from "@/gen/nama/v1/request_pb";
import { FoundMovieState } from "@/shared/components/found-movie-state/found-movie-state";
import { runtimeOf } from "@/shared/lib/title-facts";

/**
 * The top of a Movie's page that isn't on the Server: its title over its Backdrop, its overview and
 * facts, then Request, or where its Request is, for the signed-in User `you`.
 */
export function MovieRequestHero({ movie, you }: { movie: FoundMovie; you: bigint | undefined }) {
  return (
    <TitleHero backdrop={{ url: movie.backdropUrl, thumbhash: movie.backdropThumbhash }}>
      <TitleName title={movie.title} logoUrl={undefined} />
      <TitleGenres kind="Movie" genres={[]} />
      {movie.overview !== undefined && <Overview text={movie.overview} />}
      <TitleFacts
        facts={[movie.year?.toString(), runtimeOf(movie.runtimeMinutes, "short")]}
        boxed={[]}
      />
      <TitleActions>
        <FoundMovieState movie={movie} you={you} primary />
      </TitleActions>
    </TitleHero>
  );
}
