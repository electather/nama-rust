import type { MessageInitShape } from "@bufbuild/protobuf";

import type { Version } from "@/gen/nama/v1/browse_pb";
import type { DeleteMediaResponse, MediaToDeleteSchema } from "@/gen/nama/v1/library_pb";

/** The choice that deletes every Version, where any other choice is one Version's id. */
export const EVERY_VERSION = "every";

/** A Movie, Show, Season or Episode, as deleting it needs it. */
export interface Deletable {
  /** What the dialog asks about deleting, such as the Movie's title. */
  title: string;
  /** Its Versions, to choose one of when there are several. A Show or Season offers none. */
  versions: Version[];
  /** What deleting every Version names, or with an id, only that Version. */
  target: (versionId?: bigint) => MessageInitShape<typeof MediaToDeleteSchema>;
}

/** What the last delete reports: the files it couldn't delete and the Fulfillers it couldn't tell. */
export type Outcome = Pick<DeleteMediaResponse, "notDeleted" | "fulfillersNotTold">;

/** What deleting `chosen` of `deletable` names: every Version, or the one with that id. */
export function targetOf({ target }: Deletable, chosen: string) {
  return chosen === EVERY_VERSION ? target() : target(BigInt(chosen));
}
