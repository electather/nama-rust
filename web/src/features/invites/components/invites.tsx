import { InviteRow } from "@/features/invites/components/invite-row";
import { MakeAnInvite } from "@/features/invites/components/make-an-invite";
import { useInvites } from "@/features/invites/data/use-invites";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { AdminList } from "@/shared/components/admin-list/admin-list";

/**
 * What an Invite is, the pending ones, and a form to make one while the Server has a Server URL to
 * link to.
 */
export function Invites({ you }: { you: User | undefined }) {
  const data = useInvites();
  return (
    <>
      <p className="mt-1 text-sm text-pretty text-muted-foreground">
        A link someone opens to create their own User, with the Permissions you pick.
      </p>
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
