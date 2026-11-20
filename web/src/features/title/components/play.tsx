import type { ReactNode } from "react";

import { VersionChoice } from "@/features/title/components/version-choice";
import type { Version } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { PlayButton } from "@/shared/components/watch-state/play-button";

/**
 * Play `version`, or Resume it with how far in you are once started, and a choice of Version, each
 * with its picture and sound, when there's more than one, then `children`.
 */
export function Play({
  params,
  versions,
  version,
  watchState,
  onChoose,
  children,
}: {
  params: { libraryId: string; movieId: string };
  versions: Version[];
  version: Version;
  watchState: WatchState | undefined;
  onChoose: (versionId: number) => void;
  children: ReactNode;
}) {
  const versionId = String(version.id);
  return (
    <div className="mt-6 flex flex-wrap items-center gap-x-4 gap-y-3">
      <PlayButton
        link={{
          to: "/libraries/$libraryId/movies/$movieId/play",
          params,
          search: { version: Number(versionId) },
        }}
        state={watchState}
      />
      {versions.length > 1 && (
        <VersionChoice versions={versions} versionId={versionId} onChoose={onChoose} />
      )}
      {children}
    </div>
  );
}
