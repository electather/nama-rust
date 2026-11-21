import type { Version } from "@/gen/nama/v1/browse_pb";
import { pictureOf } from "@/shared/lib/version-facts";

/**
 * The first of `playable` among `versions`, with its video Track's picture size, such as "1080p",
 * or `undefined` when `versions` has none of them.
 */
export function otherVersion(versions: Version[] | undefined, playable: bigint[]) {
  const version = versions?.find(({ id }) => id === playable[0]);
  return version && { id: version.id, picture: pictureOfVersion(version) };
}

/** `version`'s first video Track's picture size, if it has one. */
function pictureOfVersion({ tracks }: Version) {
  const [video] = tracks.flatMap(({ kind }) => (kind.case === "video" ? [kind.value] : []));
  return video && pictureOf(video);
}
