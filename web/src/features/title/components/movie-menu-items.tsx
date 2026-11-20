import type { RefObject } from "react";

import { DeleteMovieItem } from "@/features/title/components/delete-movie-item";
import { MarkPlayedItem } from "@/features/title/components/mark-played-item";
import { OpenerItem } from "@/features/title/components/opener-item";
import { VersionItems } from "@/features/title/components/version-items";
import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { Movie, Version } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";
import type { Opener } from "@/shared/lib/opener";

/**
 * What else may be done to `movie`: pick which Version Play plays, when there's a choice, mark it
 * watched, edit its images with Manage Libraries, and delete it with Delete media, unless
 * `readOnlyFolder` holds it. `images` and `remove` open those two dialogs.
 */
export function MovieMenuItems({
  libraryId,
  movieId,
  movie,
  readOnlyFolder,
  version,
  onChoose,
  images,
  remove,
}: {
  libraryId: bigint;
  movieId: ExternalId;
  movie: Movie;
  readOnlyFolder: string | undefined;
  version: Version;
  onChoose: (versionId: number) => void;
  images: RefObject<Opener | null>;
  remove: RefObject<Opener | null>;
}) {
  const mayEditImages = useMay(Permission.MANAGE_LIBRARIES);
  const mayDelete = useMayDelete();
  return (
    <>
      {movie.versions.length > 1 && (
        <VersionItems versions={movie.versions} version={version} onChoose={onChoose} />
      )}
      <MarkPlayedItem
        libraryId={libraryId}
        target={{ case: "movieId", value: movieId }}
        watchState={movie.watchState}
      />
      {mayEditImages && <OpenerItem opener={images}>Edit images</OpenerItem>}
      {mayDelete && <DeleteMovieItem remove={remove} readOnlyFolder={readOnlyFolder} />}
    </>
  );
}
