import { useRouteContext } from "@tanstack/react-router";

import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/** Whether the signed-in User may delete media. */
export function useMayDelete() {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return may(signedIn.user, Permission.DELETE_MEDIA);
}
