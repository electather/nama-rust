import { useLinkedUsers } from "@/features/auth/data/use-linked-users";
import { Notice } from "@/shared/components/sign-in-layout/notice";

/** What an Account with no Linked User on any Server can do next; nothing once it has one. */
export function NoUserNotice() {
  const users = useLinkedUsers();
  if (users.length > 0) {
    return null;
  }
  return (
    <Notice kind="info">
      Your Account has no User on a Server yet. Set up a Server of your own, or ask a Server’s Admin
      for an Invite.
    </Notice>
  );
}
