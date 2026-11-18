import { useRevokeInvite } from "@/features/invites/data/use-revoke-invite";
import { revokeInviteRefusal } from "@/features/invites/lib/invite-refusals";
import type { Invite } from "@/gen/nama/v1/user_pb";
import { AdminRow } from "@/shared/components/admin-list/admin-row";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { labelsOf } from "@/shared/lib/permissions";
import { dayOf } from "@/shared/lib/title-facts";

/** A pending Invite: who it's for, who made it, until when and what it grants, to revoke. */
export function InviteRow({ invite }: { invite: Invite }) {
  const revoke = useRevokeInvite();
  const name = invite.note === "" ? "Invite" : `Invite for ${invite.note}`;
  return (
    <AdminRow
      end={
        <ConfirmDialog
          action="Revoke"
          name={`Revoke ${name}`}
          title={`Revoke ${name}?`}
          description="Its link stops working. Nobody can create a User with it."
          onConfirm={async () => {
            await revoke.mutateAsync({ inviteId: invite.id });
          }}
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
