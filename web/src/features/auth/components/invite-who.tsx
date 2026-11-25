import type { UserInvite } from "@/gen/nama/hub/v1/link_pb";
import { dayOf } from "@/shared/lib/title-facts";

/** Who a User waiting for the Account is, on which Server, and since when. */
export function InviteWho({ invite }: { invite: UserInvite }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <span className="text-base wrap-break-word">
        {invite.name} on {invite.serverName}
      </span>
      <span className="text-xs text-muted-foreground">Created {dayOf(invite.invitedAt)}</span>
    </div>
  );
}
