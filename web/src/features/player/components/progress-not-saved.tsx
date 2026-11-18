import { useRouteContext } from "@tanstack/react-router";

import { MatchIt } from "@/features/player/components/match-it";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/**
 * Under the player's title bar: that `item`, Unmatched, keeps no Watch State (ADR 0013). With
 * Manage Libraries, Match it opens its Match flow.
 */
export function ProgressNotSaved({ item, libraryId }: { item: ItemToMatch; libraryId: bigint }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return (
    // Clickable only while the controls show, as the title bar is.
    <div className="pointer-events-auto mt-2 flex justify-center px-4 group-data-controls-hidden:pointer-events-none">
      <div className="flex max-w-md flex-wrap items-center justify-center gap-x-2 rounded-lg bg-scrim px-4 py-1 text-center text-sm text-balance">
        <p className="py-1">Not matched, so progress isn’t saved.</p>
        {may(signedIn.user, Permission.MANAGE_LIBRARIES) && (
          <MatchIt item={item} libraryId={libraryId} />
        )}
      </div>
    </div>
  );
}
