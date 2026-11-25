// `web/public/push-worker.js`, at the root so its scope covers every page.
const PUSH_WORKER = "/push-worker.js";

/** Whether this page can use Web Push: a secure page in a browser with a service worker and push. */
function hasWebPush() {
  return (
    globalThis.isSecureContext &&
    "serviceWorker" in navigator &&
    "PushManager" in globalThis &&
    "Notification" in globalThis
  );
}

/** Why this browser can't turn on Web Push here, in words for the User, or `undefined` if it can. */
export function webPushUnavailable() {
  if (!globalThis.isSecureContext) {
    return "This browser shows notifications only from a page opened over HTTPS.";
  }
  if (!hasWebPush()) {
    return "This browser can’t show notifications from Nama. On an iPhone or iPad, add Nama to your Home Screen first.";
  }
  return Notification.permission === "denied"
    ? "Your browser blocks notifications from Nama. Allow them in its settings for this site, then turn this on."
    : undefined;
}

/** This browser's Web Push subscription, or `null` while it has none or can't have one. */
export async function browserSubscription() {
  if (!hasWebPush()) {
    return null;
  }
  const registration = await navigator.serviceWorker.getRegistration();
  return (await registration?.pushManager.getSubscription()) ?? null;
}

/**
 * Asks the User to allow notifications. Call it first thing after their click: some browsers
 * ask only then.
 */
export async function askToNotify() {
  return (await Notification.requestPermission()) === "granted";
}

/** Registers the push worker and subscribes this browser to push with the Server's VAPID `key`. */
export async function subscribeBrowser(key: Uint8Array) {
  await navigator.serviceWorker.register(PUSH_WORKER);
  // `subscribe` needs an active worker, which a fresh registration may not have yet.
  const registration = await navigator.serviceWorker.ready;
  // A copy, since `subscribe` takes no view of a SharedArrayBuffer, which a message's bytes may be.
  const applicationServerKey = new Uint8Array(key);
  return registration.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey });
}

/** The subscription's encryption key `name` as the Server takes it, empty when it has none. */
export function encryptionKey(subscription: PushSubscription, name: PushEncryptionKeyName) {
  return new Uint8Array(subscription.getKey(name) ?? new ArrayBuffer(0));
}
