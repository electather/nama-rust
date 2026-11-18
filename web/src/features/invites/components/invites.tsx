import { InviteRow } from "@/features/invites/components/invite-row";
import { MakeAnInvite } from "@/features/invites/components/make-an-invite";
import { useInvites } from "@/features/invites/data/use-invites";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { AdminList } from "@/shared/components/admin-list/admin-list";

/** The pending Invites, and a form to make one while the Server has a Server URL to link to. */
export function Invites({ you }: { you: User | undefined }) {
  const data = useInvites();
  return (
    <>
      {data.serverUrlSet ? (
        <MakeAnInvite you={you} />
      ) : (
        <p className="mt-4 text-sm text-muted-foreground">
          An Invite’s link needs this Server’s Server URL, which is set in Settings.
        </p>
      )}
      {data.invites.length > 0 && (
        <AdminList>
          {data.invites.map((invite) => (
            <InviteRow key={invite.id} invite={invite} />
          ))}
        </AdminList>
      )}
    </>
  );
}
