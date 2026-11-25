import type { Copy } from "@/features/title/lib/copies";
import type { Version } from "@/gen/nama/v1/browse_pb";
import { PlayButton } from "@/shared/components/watch-state/play-button";

/**
 * Play the Movie `movieId`, as its URL names it, at `version` on the Server holding `copy`, or
 * Resume it there with how far in you are once started.
 */
export function Play({
  movieId,
  copy,
  version,
}: {
  movieId: string;
  copy: Copy;
  version: Version;
}) {
  return (
    <PlayButton
      link={{
        to: "/libraries/$libraryId/movies/$movieId/play",
        params: { libraryId: String(copy.libraryId), movieId },
        search: { version: Number(version.id), server: copy.server?.id },
      }}
      state={copy.watchState}
    />
  );
}
