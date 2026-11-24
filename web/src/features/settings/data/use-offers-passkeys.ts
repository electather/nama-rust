import { useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";

import { hubInfoQuery } from "@/shared/hooks/access";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Whether a passkey can be added here: the Hub offers passkeys and the browser has them. */
export function useOffersPasskeys() {
  const { data } = useSuspenseQuery({
    ...hubInfoQuery(useTransport()),
    staleTime: loaderStaleTime,
  });
  return data?.passkeys === true && "PublicKeyCredential" in globalThis;
}
