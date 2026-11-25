import { useSuspenseQuery } from "@connectrpc/connect-query";

import { InboxService } from "@/gen/nama/v1/inbox_pb";

/** The signed-in User's notifications, newest first. */
// ponytail: the first page only, the newest 50; page with `usePaged` if Users miss older ones.
export function useInboxNotifications() {
  return useSuspenseQuery(InboxService.method.listInboxNotifications, {}).data.notifications;
}
