import type { UserInvite } from "@hub/gen/nama/hub/v1/link_pb";

const createdOn = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

/** Who a User waiting for the Account is, on which Server, and since when. */
export function InviteWho({ invite }: { invite: UserInvite }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-0.5">
      <span className="text-base wrap-break-word">
        {invite.name} on {invite.serverName}
      </span>
      <span className="text-xs text-muted-foreground">
        Created {createdOn.format(new Date(Number(invite.invitedAt) * 1000))}
      </span>
    </div>
  );
}
