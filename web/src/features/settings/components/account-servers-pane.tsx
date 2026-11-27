import { AccountLinks } from "@/features/auth/components/account-links";
import { LinkServerForm } from "@/features/auth/components/link-server-form";
import { LinkUserForm } from "@/features/auth/components/link-user-form";
import { LinkedServersGroup } from "@/features/settings/components/linked-servers-group";
import { LinkedUsersGroup } from "@/features/settings/components/linked-users-group";
import { PaneFrame } from "@/features/settings/components/pane-frame";

/**
 * The Servers the Account signed in at the Hub owns or has a User on, and its Linked Users, each
 * with Linking another by the code it shows, and the Users waiting for the Account to accept them.
 */
export function AccountServersPane() {
  return (
    <PaneFrame section="account-servers">
      <LinkedServersGroup />
      <LinkServerForm />
      <LinkedUsersGroup />
      <AccountLinks />
      <LinkUserForm />
    </PaneFrame>
  );
}
