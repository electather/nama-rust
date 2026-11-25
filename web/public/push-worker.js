// Push only: no `fetch` listener and no Cache API, so it can't serve an old app (web/AGENTS.md).

// What happened to the Request, the notification's body, by `NotificationKind`'s name in the JSON
// mapping of `WebPushMessage` (proto/nama/v1/web_push.proto).
const bodies = {
  NOTIFICATION_KIND_APPROVED: "Your Request is approved",
  NOTIFICATION_KIND_DECLINED: "Your Request is declined",
  NOTIFICATION_KIND_FAILED: "Your Request Failed",
  NOTIFICATION_KIND_PARTIALLY_AVAILABLE: "Your Request is Partially Available",
  NOTIFICATION_KIND_AVAILABLE: "Your Request is Available",
  NOTIFICATION_KIND_CANCELLED_BY_DELETION:
    "Your Request is cancelled because its media was deleted",
  NOTIFICATION_KIND_NEW_PENDING: "A Request needs approving",
};

self.addEventListener("push", (event) => {
  const { notification, link } = event.data.json();
  const title =
    notification.seasonNumber === undefined
      ? notification.title
      : `${notification.title}, Season ${notification.seasonNumber}`;
  event.waitUntil(
    self.registration.showNotification(title, {
      body: bodies[notification.kind] ?? "Your Request changed",
      icon: "/icon-192.png",
      data: { link },
    }),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const link = event.notification.data.link ?? "/";
  event.waitUntil(focusOrOpen(link));
});

async function focusOrOpen(link) {
  const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
  const open = windows.find((client) => client.url === link);
  await (open === undefined ? self.clients.openWindow(link) : open.focus());
}
