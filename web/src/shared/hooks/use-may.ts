import { useTransport } from "@connectrpc/connect-query";
import { useQuery } from "@tanstack/react-query";

import type { Permission } from "@/gen/nama/v1/sign_in_pb";
import { signedInQuery } from "@/shared/hooks/access";
import { may } from "@/shared/lib/permissions";

/** Whether the signed-in User has `permission`, which is false until it's known. */
export function useMay(permission: Permission) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the signed-in User is read for a Permission only, and what it allows is just left out until it's known
  const you = useQuery({ ...signedInQuery(useTransport()), staleTime: "static" }).data;
  return may(you?.user, permission);
}
