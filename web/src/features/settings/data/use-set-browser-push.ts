import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { browserPushKey } from "@/features/settings/data/use-browser-push";
import { WebPushService } from "@/gen/nama/v1/web_push_pb";
import { subscribeAtServer } from "@/shared/hooks/browser-push";
import { askToNotify, browserSubscription, subscribeBrowser } from "@/shared/lib/web-push";

/**
 * Turns Web Push on or off for this browser, both in the browser and at the Server, then reads the
 * browser's subscription again. Turning it on does nothing if the User doesn't allow notifications.
 */
export function useSetBrowserPush() {
  const transport = useTransport();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (on: boolean) => {
      await (on ? turnOn(transport) : turnOff(transport));
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: browserPushKey(transport) });
    },
  });
}

async function turnOn(transport: Transport) {
  if (!(await askToNotify())) {
    return;
  }
  const { publicKey } = await callUnaryMethod(transport, WebPushService.method.getWebPushKey, {});
  await subscribeAtServer(transport, await subscribeBrowser(publicKey));
}

async function turnOff(transport: Transport) {
  const subscription = await browserSubscription();
  if (subscription === null) {
    return;
  }
  await callUnaryMethod(transport, WebPushService.method.unsubscribeWebPush, {
    endpoint: subscription.endpoint,
  });
  await subscription.unsubscribe();
}
