import type { Version } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { PlayButton } from "@/shared/components/watch-state/play-button";

/** Play the Movie at `params`' `version`, or Resume it with how far in you are once started. */
export function Play({
  params,
  version,
  watchState,
}: {
  params: { libraryId: string; movieId: string };
  version: Version;
  watchState: WatchState | undefined;
}) {
  return (
    <PlayButton
      link={{
        to: "/libraries/$libraryId/movies/$movieId/play",
        params,
        search: { version: Number(version.id) },
      }}
      state={watchState}
    />
  );
}
