import { create } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vite-plus/test";

import {
  type SubscribeWebPushRequest,
  SubscribeWebPushRequestSchema,
  type UnsubscribeWebPushRequest,
  UnsubscribeWebPushRequestSchema,
  WebPushService,
} from "@/gen/nama/v1/web_push_pb";
import { fakeServer, newUser } from "@/test-app";
import { openSettings } from "@/test-settings";

describe("notify this browser, in Settings' Connections", () => {
  const serverKey = new Uint8Array([4, 1, 2]);
  const endpoint = "https://push.example/abc";

  /**
   * The Server's Web Push RPCs, recording what each subscribe and unsubscribe asked, refusing every
   * subscribe if `refuses`.
   */
  function server(refuses = false) {
    const subscribed: SubscribeWebPushRequest[] = [];
    const unsubscribed: UnsubscribeWebPushRequest[] = [];
    const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
      router.service(WebPushService, {
        getWebPushKey: () => ({ publicKey: serverKey }),
        subscribeWebPush: (request) => {
          subscribed.push(request);
          if (refuses) {
            throw new ConnectError("down", Code.Unavailable);
          }
          return {};
        },
        unsubscribeWebPush: (request) => {
          unsubscribed.push(request);
          return {};
        },
      });
    });
    return { transport, subscribed, unsubscribed };
  }

  /**
   * A secure page in a browser with Web Push, whose notification permission is `permission`, with
   * this browser already subscribed if `subscribed`. Its push service hands out `endpoint`.
   */
  function browserWithPush(permission: NotificationPermission, subscribed = false) {
    const subscription = {
      endpoint,
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

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    Reflect.deleteProperty(navigator, "serviceWorker");
  });

  test("turning it on registers the worker and sends the Server the browser's subscription", async () => {
    const browser = browserWithPush("granted");
    const { transport, subscribed } = server();
    openSettings("connections", transport);

    const toggle = await screen.findByRole("switch", { name: "Notify this browser" });
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(toggle);

    await expect.poll(() => subscribed).toHaveLength(1);
    expect(browser.register).toHaveBeenCalledWith("/push-worker.js");
    expect(browser.pushManager.subscribe).toHaveBeenCalledWith({
      userVisibleOnly: true,
      applicationServerKey: serverKey,
    });
    expect(subscribed).toStrictEqual([
      create(SubscribeWebPushRequestSchema, {
        endpoint,
        p256dh: new TextEncoder().encode("p256dh"),
        auth: new TextEncoder().encode("auth"),
      }),
    ]);
    await expect.poll(() => toggle.getAttribute("aria-checked")).toBe("true");
  });

  test("turning it off unsubscribes at the Server and in the browser", async () => {
    const browser = browserWithPush("granted", true);
    const { transport, unsubscribed } = server();
    openSettings("connections", transport);

    const toggle = await screen.findByRole("switch", { name: "Notify this browser" });
    await expect.poll(() => toggle.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(toggle);

    await expect
      .poll(() => unsubscribed)
      .toStrictEqual([create(UnsubscribeWebPushRequestSchema, { endpoint })]);
    expect(browser.subscription.unsubscribe).toHaveBeenCalledWith();
    await expect.poll(() => toggle.getAttribute("aria-checked")).toBe("false");
  });

  test("declining the browser's prompt leaves it off and subscribes nothing", async () => {
    const browser = browserWithPush("default");
    browser.notifications.requestPermission.mockResolvedValue("denied");
    const { transport, subscribed } = server();
    openSettings("connections", transport);

    const toggle = await screen.findByRole("switch", { name: "Notify this browser" });
    fireEvent.click(toggle);

    await expect.poll(() => browser.notifications.requestPermission.mock.calls).toHaveLength(1);
    await expect.poll(() => toggle.getAttribute("aria-checked")).toBe("false");
    expect(browser.pushManager.subscribe).not.toHaveBeenCalled();
    expect(subscribed).toStrictEqual([]);
  });

  test("a Server that refuses the subscription leaves the browser unsubscribed", async () => {
    const browser = browserWithPush("granted");
    openSettings("connections", server(true).transport);

    const toggle = await screen.findByRole("switch", { name: "Notify this browser" });
    fireEvent.click(toggle);

    await expect(screen.findByText(/Check that the Server is running/u)).resolves.toBeDefined();
    expect(browser.subscription.unsubscribe).toHaveBeenCalledWith();
    expect(toggle.getAttribute("aria-checked")).toBe("false");
  });

  test("a push service the browser can't reach says the browser failed", async () => {
    const browser = browserWithPush("granted");
    browser.pushManager.subscribe.mockRejectedValue(new DOMException("no service", "AbortError"));
    openSettings("connections", server().transport);

    fireEvent.click(await screen.findByRole("switch", { name: "Notify this browser" }));

    await expect(
      screen.findByText(/This browser couldn’t change its notifications/u),
    ).resolves.toBeDefined();
  });

  test("a browser that blocks notifications says how to allow them", async () => {
    browserWithPush("denied");
    openSettings("connections", server().transport);

    const toggle = await screen.findByRole("switch", { name: "Notify this browser" });
    expect(toggle.getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByText(/browser blocks notifications from Nama/u)).toBeDefined();
  });

  test("a browser without Web Push says so in place of the switch", async () => {
    vi.stubGlobal("isSecureContext", true);
    openSettings("connections", server().transport);

    await expect(screen.findByText(/browser can’t show notifications/u)).resolves.toBeDefined();
    expect(screen.queryByRole("switch", { name: "Notify this browser" })).toBeNull();
  });

  test("a page not opened over HTTPS says notifications need it, in place of the switch", async () => {
    browserWithPush("default");
    vi.stubGlobal("isSecureContext", false);
    openSettings("connections", server().transport);

    await expect(screen.findByText(/opened at an https:\/\/ address/u)).resolves.toBeDefined();
    expect(screen.queryByRole("switch", { name: "Notify this browser" })).toBeNull();
  });
});
