import type { RefObject } from "react";

import { DeleteMovieItem } from "@/features/title/components/delete-movie-item";
import { MarkPlayedItem } from "@/features/title/components/mark-played-item";
import { MatchItems } from "@/features/title/components/match-items";
import { OpenerItem } from "@/features/title/components/opener-item";
import { ServerItems } from "@/features/title/components/server-items";
import { VersionItems } from "@/features/title/components/version-items";
import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { PlayFrom } from "@/features/title/lib/server-choices";
import type { Movie, Version } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";
import type { Opener } from "@/shared/lib/opener";

/**
 * What else may be done to `movie`: pick which Server Play plays from, at the Hub, and which of its
 * `versions` there, when there's a choice of either, mark it watched, edit its images and fix its
 * Match with Manage Libraries, and delete it with Delete media, unless `readOnlyFolder` holds it.
 * `images`, `fix`, `unmatch` and `remove` open those dialogs.
 */
export function MovieMenuItems({
  libraryId,
  movieId,
  movie,
  readOnlyFolder,
  versions,
  version,
  onChoose,
  playFrom,
  images,
  fix,
  unmatch,
  remove,
}: {
  libraryId: bigint;
  movieId: ExternalId;
  movie: Movie;
  readOnlyFolder: string | undefined;
  versions: Version[];
  version: Version;
  onChoose: (versionId: number) => void;
  playFrom: PlayFrom;
  images: RefObject<Opener | null>;
  fix: RefObject<Opener | null>;
  unmatch: RefObject<Opener | null>;
  remove: RefObject<Opener | null>;
}) {
  const mayEditImages = useMay(Permission.MANAGE_LIBRARIES);
  const mayDelete = useMayDelete();
  return (
    <>
      <ServerItems {...playFrom} />
      {versions.length > 1 && (
        <VersionItems versions={versions} version={version} onChoose={onChoose} />
      )}
      <MarkPlayedItem
        libraryId={libraryId}
        target={{ case: "movieId", value: movieId }}
        watchState={movie.watchState}
      />
      {mayEditImages && <OpenerItem opener={images}>Edit images</OpenerItem>}
      <MatchItems fix={fix} unmatch={unmatch} />
      {mayDelete && <DeleteMovieItem remove={remove} readOnlyFolder={readOnlyFolder} />}
    </>
  );
}
