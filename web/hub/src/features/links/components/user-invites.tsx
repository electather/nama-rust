import { UserInviteItem } from "@hub/features/links/components/user-invite-item";
import { useUserInvites } from "@hub/features/links/data/invites";

/**
 * The Users Admins created from the signed-in Account, which it accepts to sign in to their
 * Servers with it. Nothing shows while there are none.
 */
export function UserInvites() {
  const invites = useUserInvites();
  if (invites.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        The Hub couldn’t list the Users waiting for you just now. Reload the page to try again.
      </p>
    );
  }
  if (invites.data === undefined || invites.data.invites.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby="users-waiting" className="flex flex-col gap-2">
      <h2 id="users-waiting" className="text-lg/6 font-medium">
        Users waiting for you
      </h2>
      <p className="text-sm text-pretty text-muted-foreground">
        A Server’s Admin created these from your Account. Accept one to sign in to that Server with
        your Account.
      </p>
      <ul className="flex flex-col divide-y">
        {invites.data.invites.map((invite) => (
          <UserInviteItem key={invite.id} invite={invite} />
        ))}
      </ul>
    </section>
  );
}
