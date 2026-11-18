import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { AdminRow } from "@/components/admin-list";
import { ConfirmDialog } from "@/components/confirm-dialog";
import { type Invite, UserService } from "@/gen/nama/v1/user_pb";
import { dayOf } from "@/lib/browse";
import { labelsOf } from "@/lib/permissions";
import { invitesKey, revokeInviteRefusal } from "@/lib/users";

/** A pending Invite: who it's for, who made it, until when and what it grants, to revoke. */
export function InviteRow({ invite }: { invite: Invite }) {
  const queryClient = useQueryClient();
  const revoke = useMutation(UserService.method.revokeInvite, {
    // Refused or not, the list may be out of date.
    onSettled: () => queryClient.invalidateQueries({ queryKey: invitesKey }),
  });
  const name = invite.note === "" ? "Invite" : `Invite for ${invite.note}`;
  return (
    <AdminRow
      end={
        <ConfirmDialog
          action="Revoke"
          name={`Revoke ${name}`}
          title={`Revoke ${name}?`}
          description="Its link stops working. Nobody can create a User with it."
          onConfirm={() => revoke.mutateAsync({ inviteId: invite.id })}
          refusal={revokeInviteRefusal}
        />
      }
    >
      <p className="truncate font-medium">{invite.note === "" ? "No note" : invite.note}</p>
      <p className="text-sm text-muted-foreground">
        {/* Each phrase stays whole when the line wraps. */}
        <span className="whitespace-nowrap">By {invite.creatorName}</span> ·{" "}
        <span className="whitespace-nowrap">Works until {dayOf(invite.expiresAt)}</span>
      </p>
      <p className="text-sm text-muted-foreground">{labelsOf(invite.permissions)}</p>
    </AdminRow>
  );
}
