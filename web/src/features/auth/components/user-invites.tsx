import { UserInviteItem } from "@/features/auth/components/user-invite-item";
import { useUserInvites } from "@/features/auth/data/use-user-invites";

/** The Users Admins created from the signed-in Account, to accept; nothing while there are none. */
export function UserInvites() {
  const invites = useUserInvites();
  if (invites.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby="users-waiting" className="flex flex-col gap-2">
      <h2 id="users-waiting" className="text-lg font-semibold">
        Users waiting for you
      </h2>
      <p className="text-sm text-pretty text-muted-foreground">
        A Server’s Admin created these from your Account. Accept one to sign in to that Server with
        your Account.
      </p>
      <ul className="flex flex-col divide-y">
        {invites.map((invite) => (
          <UserInviteItem key={invite.id} invite={invite} />
        ))}
      </ul>
    </section>
  );
}
