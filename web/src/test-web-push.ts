import { vi } from "vite-plus/test";

/** The endpoint the fake push service of `browserWithPush` hands out. */
export const PUSH_ENDPOINT = "https://push.example/abc";

/**
 * A secure page in a browser with Web Push, whose notification permission is `permission`, with
 * this browser already subscribed if `subscribed`. Its push service hands out `PUSH_ENDPOINT`.
 * Undo it with `forgetBrowserPush`.
 */
export function browserWithPush(permission: NotificationPermission, subscribed = false) {
  const subscription = {
    endpoint: PUSH_ENDPOINT,
    getKey: (name: string) => new TextEncoder().encode(name).buffer,
    unsubscribe: vi.fn<() => Promise<boolean>>().mockResolvedValue(true),
  };
  const pushManager = {
    subscribe: vi
      .fn<(options: PushSubscriptionOptionsInit) => Promise<unknown>>()
      .mockResolvedValue(subscription),
    // oxlint-disable-next-line require-await, typescript/require-await -- getSubscription is async, and this fake answers at once
    getSubscription: async () => {
      const has = subscribed || pushManager.subscribe.mock.calls.length > 0;
      return has && subscription.unsubscribe.mock.calls.length === 0 ? subscription : null;
    },
  };
  const registration = { pushManager };
  const register = vi.fn<(url: string) => Promise<unknown>>().mockResolvedValue(registration);
  const notifications = {
    permission,
    requestPermission: vi.fn<() => Promise<string>>().mockResolvedValue(permission),
  };
  vi.stubGlobal("isSecureContext", true);
  vi.stubGlobal("PushManager", Object);
  vi.stubGlobal("Notification", notifications);
  const serviceWorker = {
    register,
    ready: Promise.resolve(registration),
    getRegistration: vi.fn<() => Promise<unknown>>().mockResolvedValue(registration),
  };
  Object.defineProperty(navigator, "serviceWorker", { value: serviceWorker, configurable: true });
  return { pushManager, register, subscription, notifications };
}

/** Takes away what `browserWithPush` stubbed. */
export function forgetBrowserPush() {
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, "serviceWorker");
}
