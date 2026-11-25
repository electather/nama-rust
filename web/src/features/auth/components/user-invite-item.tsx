import { Button } from "nama-ui/components/button";

import { InviteWho } from "@/features/auth/components/invite-who";
import { LinkRemedy } from "@/features/auth/components/link-remedy";
import { useAcceptUserInvite } from "@/features/auth/data/use-accept-user-invite";
import { linkErrors } from "@/features/auth/lib/link-errors";
import type { UserInvite } from "@/gen/nama/hub/v1/link_pb";
import { Notice } from "@/shared/components/sign-in-layout/notice";

/** One User created from the signed-in Account: who, on which Server, since when, and accepting them. */
export function UserInviteItem({ invite }: { invite: UserInvite }) {
  const accept = useAcceptUserInvite();
  return (
    <li className="flex flex-col gap-2 py-2.5">
      <div className="flex items-center justify-between gap-3">
        <InviteWho invite={invite} />
        <Button
          aria-label={`Accept ${invite.name} on ${invite.serverName}`}
          disabled={accept.isPending}
          onClick={() => {
            accept.mutate({ id: invite.id });
          }}
        >
          {accept.isPending ? "Accepting…" : "Accept"}
        </Button>
      </div>
      {accept.error && <Notice kind="error">{linkErrors(accept.error).form}</Notice>}
      <LinkRemedy error={accept.error} />
    </li>
  );
}
