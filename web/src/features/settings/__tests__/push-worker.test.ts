// oxlint-disable-next-line import/no-absolute-path -- public/ has no alias, and Vite reads `/` as web/
import source from "/public/push-worker.js?raw";
import { describe, expect, test, vi } from "vite-plus/test";

/** Runs the worker against a fake `self` with these app `windows` open, and returns that `self`. */
function runWorker(windows: { url: string; focus: () => Promise<void> }[] = []) {
  const listeners = new Map<string, (event: unknown) => void>();
  const self = {
    addEventListener: (type: string, listener: (event: unknown) => void) => {
      listeners.set(type, listener);
    },
    registration: { showNotification: vi.fn<(title: string, options: unknown) => void>() },
    clients: {
      matchAll: vi.fn<() => Promise<typeof windows>>().mockResolvedValue(windows),
      openWindow: vi.fn<(url: string) => void>(),
    },
  };
  // oxlint-disable-next-line no-new-func, typescript/no-implied-eval, typescript/no-unsafe-call -- runs the worker's own file, as a browser does
  new Function("self", source)(self);
  return { listeners, self };
}

/** Dispatches `event` to `listener`, then waits for what it passed to `event.waitUntil`. */
async function dispatch(listener: ((event: unknown) => void) | undefined, event: object) {
  let done: Promise<unknown> = Promise.resolve();
  listener?.({
    ...event,
    waitUntil: (promise: Promise<unknown>) => {
      done = promise;
    },
  });
  await done;
}

// web/AGENTS.md, "Service worker: push only".
describe("the push-only service worker", () => {
  test("has no fetch listener and no Cache API, so it can't serve an old app", () => {
    expect(source).not.toMatch(/["']fetch["']|onfetch|caches|skipWaiting/u);
    expect([...runWorker().listeners.keys()]).toStrictEqual(["push", "notificationclick"]);
  });

  test("shows a pushed notification titled by its Season", async () => {
    const { listeners, self } = runWorker();
    const message = {
      notification: { kind: "NOTIFICATION_KIND_AVAILABLE", title: "Severance", seasonNumber: 2 },
      link: "https://nama.example/requests",
    };

    await dispatch(listeners.get("push"), { data: { json: () => message } });

    expect(self.registration.showNotification).toHaveBeenCalledWith("Severance, Season 2", {
      body: "Your Request is Available",
      icon: "/icon-192.png",
      data: { link: "https://nama.example/requests" },
    });
  });

  test("a click focuses the window already at the link, else opens it", async () => {
    const focus = vi.fn<() => Promise<void>>();
    const open = runWorker([{ url: "https://nama.example/requests", focus }]);
    const notification = {
      close: vi.fn<() => void>(),
      data: { link: "https://nama.example/requests" },
    };

    await dispatch(open.listeners.get("notificationclick"), { notification });

    expect(focus).toHaveBeenCalledWith();
    expect(open.self.clients.openWindow).not.toHaveBeenCalled();
    expect(notification.close).toHaveBeenCalledWith();

    const closed = runWorker();
    await dispatch(closed.listeners.get("notificationclick"), {
      notification: { close: () => {}, data: {} },
    });
    expect(closed.self.clients.openWindow).toHaveBeenCalledWith("/");
  });
});
