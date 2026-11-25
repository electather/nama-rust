import { useLinkedUsers } from "@/features/auth/data/use-linked-users";
import { useUserInvites } from "@/features/auth/data/use-user-invites";
import { Notice } from "@/shared/components/sign-in-layout/notice";

/**
 * What an Account with no Linked User on any Server can do next; nothing once it has one, or while
 * a User waits for it to accept them, which shows instead.
 */
export function NoUserNotice() {
  const users = useLinkedUsers();
  const invites = useUserInvites();
  if (users.length > 0 || invites.length > 0) {
    return null;
  }
  return (
    <Notice kind="info">
      Your Account has no User on a Server yet. Set up a Server of your own, or ask a Server’s Admin
      for an Invite.
    </Notice>
  );
}
