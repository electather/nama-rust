import type { ServerChoice } from "@/features/title/lib/server-choices";
import type { Version } from "@/gen/nama/v1/browse_pb";
import { PlayButton } from "@/shared/components/watch-state/play-button";

/**
 * Play the Movie `movieId`, as its URL names it, at `version` on the Server holding `choice`, or
 * Resume it there with how far in you are once started.
 */
export function Play({
  movieId,
  choice,
  version,
}: {
  movieId: string;
  choice: ServerChoice;
  version: Version;
}) {
  return (
    <PlayButton
      link={{
        to: "/libraries/$libraryId/movies/$movieId/play",
        params: { libraryId: String(choice.libraryId), movieId },
        search: { version: Number(version.id), server: choice.server?.id },
      }}
      state={choice.watchState}
    />
  );
}
