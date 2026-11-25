import { type MessageInitShape, create } from "@bufbuild/protobuf";
import type { ConnectRouter } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { type InboxNotificationSchema, InboxService } from "@/gen/nama/v1/inbox_pb";
import { RequestSchema } from "@/gen/nama/v1/request_pb";
import { NotificationKind } from "@/gen/nama/v1/user_pb";
import { eventFeed, fakeServer, newUser, openApp } from "@/test-app";

type Notification = MessageInitShape<typeof InboxNotificationSchema>;

/** A fake Server's inbox holding `notifications`, newest first, which marking reads in place. */
function inboxOf(notifications: Notification[]) {
  const unread = () => notifications.filter(({ readAt }) => readAt === undefined);
  return (router: ConnectRouter) => {
    router.service(InboxService, {
      listInboxNotifications: () => ({ notifications }),
      countUnreadInboxNotifications: () => ({ unread: unread().length }),
      markInboxNotificationRead: ({ id }) => {
        const marked = notifications.find((notification) => notification.id === id);
        if (marked) {
          marked.readAt = 2n;
        }
        return {};
      },
      markAllInboxNotificationsRead: () => {
        for (const notification of unread()) {
          notification.readAt = 2n;
        }
        return {};
      },
    });
  };
}

function openWithInbox(notifications: Notification[], events?: ReadableStream<SubscribeResponse>) {
  const transport = fakeServer(
    { signedIn: newUser, ...(events && { events }) },
    {},
    {},
    inboxOf(notifications),
  );
  return openApp("/code", transport);
}

afterEach(cleanup);

describe("the notification inbox", () => {
  const available = {
    id: 2n,
    kind: NotificationKind.AVAILABLE,
    title: "Arrival",
    requestId: 7n,
    createdAt: 1n,
  };
  const approved = {
    id: 1n,
    kind: NotificationKind.APPROVED,
    title: "Severance",
    seasonNumber: 2,
    requestId: 8n,
    createdAt: 1n,
    readAt: 1n,
  };

  test("the bell counts the unread and lists every notification newest first", async () => {
    openWithInbox([{ ...available }, { ...approved }]);

    fireEvent.click(await screen.findByRole("button", { name: "Notifications, 1 unread" }));

    const items = await screen.findAllByRole("menuitem", { name: /^(?!Mark all read)/u });
    expect(items.map((item) => item.textContent)).toStrictEqual([
      "Unread: Arrival is Available",
      "Severance Season 2 is approved",
    ]);
  });

  test("the bell shows no count with nothing unread", async () => {
    openWithInbox([{ ...approved }]);

    fireEvent.click(await screen.findByRole("button", { name: "Notifications" }));
    await screen.findByRole("menuitem", { name: "Severance Season 2 is approved" });

    expect(screen.getByRole("button", { name: "Notifications" }).textContent).toBe("");
  });

  test("opening a notification marks it read and opens your Requests", async () => {
    const router = openWithInbox([{ ...available }]);

    fireEvent.click(await screen.findByRole("button", { name: "Notifications, 1 unread" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Unread: Arrival is Available" }));

    await waitFor(() => {
      expect(router.state.location.search.settings).toBe("requests");
    });
    await screen.findByRole("button", { name: "Notifications" });
  });

  test("mark all read zeroes the count", async () => {
    openWithInbox([{ ...available }, { ...approved, readAt: undefined }]);

    const bell = await screen.findByRole("button", { name: "Notifications, 2 unread" });
    fireEvent.click(bell);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Mark all read" }));

    await expect.poll(() => bell.getAttribute("aria-label")).toBe("Notifications");
  });

  test("with no notifications the list says so", async () => {
    openWithInbox([]);

    fireEvent.click(await screen.findByRole("button", { name: "Notifications" }));

    await screen.findByText("No notifications yet");
    expect(screen.queryByRole("menuitem", { name: "Mark all read" })).toBeNull();
  });

  test("a Request event counts again", async () => {
    const feed = eventFeed();
    const notifications: Notification[] = [{ ...available }];
    openWithInbox(notifications, feed.stream);
    const bell = await screen.findByRole("button", { name: "Notifications, 1 unread" });

    notifications.unshift({ ...approved, id: 3n, readAt: undefined });
    const request = create(RequestSchema, { id: 8n });
    feed.send({ event: { case: "requestStateChanged", value: { request } } });

    await expect.poll(() => bell.getAttribute("aria-label")).toBe("Notifications, 2 unread");
  });
});
