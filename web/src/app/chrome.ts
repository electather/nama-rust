import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { adminPagesFor } from "@/shared/hooks/access";
import { may } from "@/shared/lib/permissions";

/** What the top bar and the action bar show of the signed-in `user`. */
export function chromeFor(user: User | undefined) {
  return {
    hasAdminPages: adminPagesFor(user).length > 0,
    managesRequests: may(user, Permission.MANAGE_REQUESTS),
    userName: user?.name ?? "",
  };
}
