import type { MessageInitShape } from "@bufbuild/protobuf";

import { DeletedWhilePlaying } from "@/features/player/components/deleted-while-playing";
import { Player } from "@/features/player/components/player";
import { useMovieToPlay } from "@/features/player/data/use-movie-to-play";
import type { GetMovieRequestSchema } from "@/gen/nama/v1/browse_pb";
import { isGone } from "@/shared/hooks/browse";
import { versionToPlay } from "@/shared/lib/version-to-play";

/** Plays a Movie's Version, `version` if it names one and else the largest, from where you stopped. */
export function MoviePlayer({
  input,
  version,
  params,
}: {
  input: MessageInitShape<typeof GetMovieRequestSchema>;
  version: number | undefined;
  params: { libraryId: string; movieId: string };
}) {
  const { data, error } = useMovieToPlay(input);
  const { movie } = data;
  // An Event removed it since the loader ran.
  if (!movie || isGone(error)) {
    return <DeletedWhilePlaying />;
  }
  const { id, tracks } = versionToPlay(movie.versions, version);
  return (
    <Player
      title={movie.title}
      versionId={id}
      tracks={tracks}
      watchState={movie.watchState}
      posterUrl={movie.posterUrl}
      back={{ to: "/libraries/$libraryId/movies/$movieId", params }}
    />
  );
}
