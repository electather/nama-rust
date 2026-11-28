import { TriangleAlert } from "lucide-react";

import { MatchIt } from "@/shared/components/find-a-match/match-it";
import { StateLabel } from "@/shared/components/state-label";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/**
 * Under the player's title bar: that `item`, Unmatched, keeps no Watch State (ADR 0013). With
 * Manage Libraries, Match it opens its Match flow.
 */
export function ProgressNotSaved({ item, libraryId }: { item: ItemToMatch; libraryId: bigint }) {
  return (
    // Clickable only while the controls show, as the title bar is.
    <div className="pointer-events-auto mt-4 flex flex-wrap items-center gap-2 px-5 group-data-controls-hidden:pointer-events-none">
      <StateLabel Icon={TriangleAlert} label="Not matched, so progress isn’t saved." />
      <MatchIt item={item} libraryId={libraryId} />
    </div>
  );
}
