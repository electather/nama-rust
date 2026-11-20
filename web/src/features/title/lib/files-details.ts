import type { ReactNode } from "react";

import type { DetailGroup } from "@/features/title/lib/detail-group";
import { audioOf, sizeOfAll, subtitlesOf, videoOf } from "@/features/title/lib/version-details";
import type { Version } from "@/gen/nama/v1/browse_pb";

/**
 * The Details a title's files on the Server give: their languages, and, after `library`, the
 * title's Library, and `more` facts, such as how many Episodes, their picture and the room they take.
 */
export function filesDetails(
  versions: Version[],
  library: ReactNode,
  more: DetailGroup["facts"] = [],
): DetailGroup[] {
  return [
    {
      heading: "Languages",
      facts: [
        ["Audio", audioOf(versions)],
        ["Subtitles", subtitlesOf(versions)],
      ],
    },
    {
      heading: "On this Server",
      facts: [
        ["Library", library],
        ...more,
        ["Video", videoOf(versions)],
        ["Size", sizeOfAll(versions)],
      ],
    },
  ];
}
