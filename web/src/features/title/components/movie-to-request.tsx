import { notFound } from "@tanstack/react-router";

import { CastAndCrew } from "@/features/title/components/cast-and-crew";
import { MovieRequestHero } from "@/features/title/components/movie-request-hero";
import { SimilarTitles } from "@/features/title/components/similar-titles";
import { TitleDetails } from "@/features/title/components/title-details";
import { TitlePage } from "@/features/title/components/title-page";
import { TitleTrailers } from "@/features/title/components/title-trailers";
import { useFoundMovie } from "@/features/title/data/use-found-movie";
import { crewWithJob } from "@/features/title/lib/crew-with-job";
import { originalAudio } from "@/features/title/lib/original-audio";
import { similarMovies } from "@/features/title/lib/similar-titles";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * A Movie from the title source, for the signed-in User `you` to Request: its top, with Request or
 * where its Request is, its Trailers, the Similar titles on this Server, then its Details.
 */
export function MovieToRequest({ movieId, you }: { movieId: ExternalId; you: bigint | undefined }) {
  const movie = useFoundMovie({ movieId });
  if (!movie) {
    throw notFound();
  }
  return (
    <TitlePage hero={<MovieRequestHero movieId={movieId} movie={movie} you={you} />}>
      <CastAndCrew cast={movie.cast} crew={movie.crew} />
      <TitleTrailers trailers={movie.trailers} />
      <SimilarTitles heading="On this Server" titles={similarMovies(movie.similar)} />
      <TitleDetails
        groups={[
          {
            heading: "Information",
            facts: [
              ["Released", movie.year?.toString()],
              ["Directed by", crewWithJob(movie.crew, "Director")],
            ],
          },
          { heading: "Languages", facts: [originalAudio(movie.originalLanguage)] },
        ]}
      />
    </TitlePage>
  );
}
