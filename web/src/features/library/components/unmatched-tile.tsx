import type { UnmatchedItemSummary } from "@/gen/nama/v1/browse_pb";
import { MatchIt } from "@/shared/components/find-a-match/match-it";
import { PlayUnmatched } from "@/shared/components/play-unmatched";
import { PosterArt } from "@/shared/components/poster/poster-art";
import { parsedAs } from "@/shared/lib/unmatched-names";

/**
 * An Unmatched Item in a Library's grid: a poster box titled by what its file's name was parsed
 * as, never where the file is (ADR 0069), with Play, and for a User who may, Find a Match, always
 * shown over its foot.
 */
export function UnmatchedTile({
  item,
  libraryId,
}: {
  item: UnmatchedItemSummary;
  libraryId: bigint;
}) {
  const name = parsedAs(item);
  return (
    <div className="relative">
      <PosterArt art={undefined} title={name} size="tile" />
      <div className="absolute inset-x-0 bottom-0 flex flex-wrap justify-center gap-1 p-1.5">
        <PlayUnmatched
          libraryId={libraryId}
          versionId={item.versionId}
          name={name}
          variant="overlay"
        />
        <MatchIt item={item} libraryId={libraryId} variant="overlay" label="Find a Match" />
      </div>
    </div>
  );
}
