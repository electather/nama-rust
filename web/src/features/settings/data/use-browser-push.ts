import type { Transport } from "@connectrpc/connect";
import { useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";

import { browserSubscription } from "@/features/settings/lib/web-push";
import { serverKey } from "@/shared/hooks/server-key";

/** The query key of this browser's Web Push subscription, to read again after changing it. */
export function browserPushKey(transport: Transport) {
  return ["browser-push", serverKey(transport)];
}

/** This browser's Web Push subscription, or `null` while it has none. */
export function useBrowserPush() {
  return useSuspenseQuery({
    queryKey: browserPushKey(useTransport()),
    queryFn: browserSubscription,
  }).data;
}
