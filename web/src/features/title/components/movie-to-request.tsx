import { notFound } from "@tanstack/react-router";

import { CastAndCrew } from "@/features/title/components/cast-and-crew";
import { MovieRequestHero } from "@/features/title/components/movie-request-hero";
import { TitleDetails } from "@/features/title/components/title-details";
import { TitlePage } from "@/features/title/components/title-page";
import { useFoundMovie } from "@/features/title/data/use-found-movie";
import { crewWithJob } from "@/features/title/lib/crew-with-job";
import { originalAudio } from "@/features/title/lib/original-audio";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * A Movie from the title source, for the signed-in User `you` to Request: its top, with Request or
 * where its Request is, then its Details.
 */
export function MovieToRequest({ movieId, you }: { movieId: ExternalId; you: bigint | undefined }) {
  const movie = useFoundMovie({ movieId });
  if (!movie) {
    throw notFound();
  }
  return (
    <TitlePage hero={<MovieRequestHero movieId={movieId} movie={movie} you={you} />}>
      <CastAndCrew cast={movie.cast} crew={movie.crew} />
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
