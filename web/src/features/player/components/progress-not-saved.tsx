import { useRouteContext } from "@tanstack/react-router";
import { TriangleAlert } from "lucide-react";

import { MatchIt } from "@/features/player/components/match-it";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { StateLabel } from "@/shared/components/state-label";
import { may } from "@/shared/lib/permissions";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/**
 * Under the player's title bar: that `item`, Unmatched, keeps no Watch State (ADR 0013). With
 * Manage Libraries, Match it opens its Match flow.
 */
export function ProgressNotSaved({ item, libraryId }: { item: ItemToMatch; libraryId: bigint }) {
  const { signedIn } = useRouteContext({ from: "/_member/_server" });
  return (
    // Clickable only while the controls show, as the title bar is.
    <div className="pointer-events-auto mt-4 flex flex-wrap items-center gap-2 px-5 group-data-controls-hidden:pointer-events-none">
      <StateLabel
        tone="warning"
        Icon={TriangleAlert}
        label="Not matched, so progress isn’t saved."
      />
      {may(signedIn.user, Permission.MANAGE_LIBRARIES) && (
        <MatchIt item={item} libraryId={libraryId} />
      )}
    </div>
  );
}
