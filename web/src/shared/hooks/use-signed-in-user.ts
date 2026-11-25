import { useTransport } from "@connectrpc/connect-query";
import { useQuery } from "@tanstack/react-query";

import { signedInQuery } from "@/shared/hooks/access";

/**
 * The User signed in on the Server in use, as at the Hub a picked Server's Settings run against
 * theirs, or `undefined` until it's known.
 */
export function useSignedInUser() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the signed-in User is read for who they are and their Permissions only, and what that allows is just left out until it's known
  return useQuery({ ...signedInQuery(useTransport()), staleTime: "static" }).data?.user;
}
