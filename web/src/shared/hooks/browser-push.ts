import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod } from "@connectrpc/connect-query";

import { WebPushService } from "@/gen/nama/v1/web_push_pb";
import { browserSubscription, encryptionKey } from "@/shared/lib/web-push";

// `web/src/AGENTS.md`, "One module changes who is signed in".

/**
 * Sends the Server this browser's `subscription` for the User signed in. If the Server refuses,
 * the browser unsubscribes too, so it never shows on while the Server pushes it nothing, or
 * another User's notifications.
 */
export async function subscribeAtServer(transport: Transport, subscription: PushSubscription) {
  try {
    await callUnaryMethod(transport, WebPushService.method.subscribeWebPush, {
      endpoint: subscription.endpoint,
      p256dh: encryptionKey(subscription, "p256dh"),
      auth: encryptionKey(subscription, "auth"),
    });
  } catch (error) {
    await subscription.unsubscribe();
    throw error;
  }
}

/**
 * Hands this browser's push subscription, if it has one, to the User the Device now acts as, after
 * a sign-in or a switch. A failure doesn't undo either.
 */
export async function moveBrowserPush(transport: Transport) {
  try {
    const subscription = await browserSubscription();
    if (subscription !== null) {
      await subscribeAtServer(transport, subscription);
    }
  } catch {
    // A refused subscription was unsubscribed in the browser too, so it shows off for them. A
    // failed read leaves the browser as it was: nothing more can be done from here.
  }
}

/**
 * Stops pushes to this browser for the User about to sign out, at the Server and in the browser.
 * Call it while they're still signed in. A failure doesn't stop the sign-out.
 */
export async function leaveBrowserPush(transport: Transport) {
  try {
    const subscription = await browserSubscription();
    if (subscription !== null) {
      // Both, even if the Server can't be reached: the browser's unsubscribe alone ends its pushes.
      await Promise.allSettled([
        callUnaryMethod(transport, WebPushService.method.unsubscribeWebPush, {
          endpoint: subscription.endpoint,
        }),
        subscription.unsubscribe(),
      ]);
    }
  } catch {
    // Reading the subscription failed, so there is nothing this browser can stop.
  }
}
