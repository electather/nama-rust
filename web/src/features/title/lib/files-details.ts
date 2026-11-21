import type { ReactNode } from "react";

import type { DetailGroup } from "@/features/title/lib/detail-group";
import { originalAudio } from "@/features/title/lib/original-audio";
import { audioOf, sizeOfAll, subtitlesOf, videoOf } from "@/features/title/lib/version-details";
import type { Version } from "@/gen/nama/v1/browse_pb";

/**
 * The Details a title's files on the Server give: the language it was made in and its files', and,
 * after `library`, the title's Library, and `more` facts, such as how many Episodes, their picture
 * and the room they take.
 */
export function filesDetails(
  { versions, originalLanguage }: { versions: Version[]; originalLanguage?: string | undefined },
  library: ReactNode,
  more: DetailGroup["facts"] = [],
): DetailGroup[] {
  return [
    {
      heading: "Languages",
      facts: [
        originalAudio(originalLanguage),
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
