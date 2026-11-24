import { Monitor } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { ItemRow } from "@/features/settings/components/item-row";
import { lastUsed } from "@/features/settings/lib/last-used";
import type { Session } from "@/gen/nama/hub/v1/account_pb";
import { IconTile } from "@/shared/components/icon-tile";

/** One browser the Account is signed in on, and, unless it's this one, signing it out. */
export function SessionItem({
  session,
  signingOut,
  onSignOut,
}: {
  session: Session;
  /** Whether a session is being signed out, this one or another. */
  signingOut: boolean;
  onSignOut: () => void;
}) {
  return (
    <ItemRow
      beside
      start={<IconTile Icon={Monitor} />}
      name={session.name}
      meta={session.isCurrent ? "This browser" : lastUsed(session.lastUsedAt)}
    >
      {!session.isCurrent && (
        <Button
          variant="ghost"
          aria-label={`Sign out ${session.name}`}
          disabled={signingOut}
          onClick={onSignOut}
        >
          Sign out
        </Button>
      )}
    </ItemRow>
  );
}
