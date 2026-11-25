import { useQuery } from "@connectrpc/connect-query";

import { InboxService } from "@/gen/nama/v1/inbox_pb";

/** How many of the signed-in User's notifications are unread: 0 until the Server says. */
export function useUnreadCount() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the bell is in the top bar, which mustn't wait for its count
  return useQuery(InboxService.method.countUnreadInboxNotifications, {}).data?.unread ?? 0;
}
