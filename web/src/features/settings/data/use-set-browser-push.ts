import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { browserPushKey } from "@/features/settings/data/use-browser-push";
import {
  askToNotify,
  browserSubscription,
  subscribeBrowser,
  encryptionKey,
} from "@/features/settings/lib/web-push";
import { WebPushService } from "@/gen/nama/v1/web_push_pb";

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
  const subscription = await subscribeBrowser(publicKey);
  try {
    await callUnaryMethod(transport, WebPushService.method.subscribeWebPush, {
      endpoint: subscription.endpoint,
      p256dh: encryptionKey(subscription, "p256dh"),
      auth: encryptionKey(subscription, "auth"),
    });
  } catch (error) {
    // Kept, the switch would show on while the Server pushes nothing.
    await subscription.unsubscribe();
    throw error;
  }
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
