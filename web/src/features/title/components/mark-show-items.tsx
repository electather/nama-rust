import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { useSetPlayed } from "@/shared/hooks/use-set-played";
import { DropdownMenuItem } from "@/shared/ui/dropdown-menu";

/** Marks the whole Show watched or unwatched. */
export function MarkShowItems({ libraryId, showId }: { libraryId: bigint; showId: ExternalId }) {
  const setPlayed = useSetPlayed();
  const mark = (played: boolean) => () => {
    setPlayed({ libraryId, target: { case: "showId", value: showId }, played });
  };
  return (
    <>
      <DropdownMenuItem onClick={mark(true)}>Mark Show as watched</DropdownMenuItem>
      <DropdownMenuItem onClick={mark(false)}>Mark Show as unwatched</DropdownMenuItem>
    </>
  );
}
