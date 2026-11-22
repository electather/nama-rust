import { InviteWho } from "@hub/features/links/components/invite-who";
import { LinkRemedy } from "@hub/features/links/components/link-remedy";
import { useAcceptUserInvite } from "@hub/features/links/data/invites";
import { linkErrors } from "@hub/features/links/lib/link-refusal";
import type { UserInvite } from "@hub/gen/nama/hub/v1/link_pb";
import { Button } from "nama-ui/components/button";

/** One User created from the signed-in Account: who, on which Server, since when, and accepting them. */
export function UserInviteItem({ invite }: { invite: UserInvite }) {
  const accept = useAcceptUserInvite();
  const errors = linkErrors(accept.error);
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
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {errors.form}
      </p>
      <LinkRemedy errors={errors} />
    </li>
  );
}
