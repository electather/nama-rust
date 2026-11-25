import { LinkedServersGroup } from "@/features/settings/components/linked-servers-group";
import { LinkedUsersGroup } from "@/features/settings/components/linked-users-group";
import { PaneFrame } from "@/features/settings/components/pane-frame";

/** The Servers the Account signed in at the Hub owns or has a User on, and its Linked Users. */
export function AccountServersPane() {
  return (
    <PaneFrame section="account-servers">
      <LinkedServersGroup />
      <LinkedUsersGroup />
    </PaneFrame>
  );
}
