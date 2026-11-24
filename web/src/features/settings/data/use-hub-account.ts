import { useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";

import { signedInAccountQuery } from "@/shared/hooks/access";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Account signed in at the Hub: its email, and whether it has a password. */
export function useHubAccount() {
  const { data } = useSuspenseQuery({
    ...signedInAccountQuery(useTransport()),
    staleTime: loaderStaleTime,
  });
  return { email: data?.account?.email ?? "", hasPassword: data?.hasPassword === true };
}
