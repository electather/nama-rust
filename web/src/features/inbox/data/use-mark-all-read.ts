import { useMutation } from "@connectrpc/connect-query";

import { useRefetchInbox } from "@/features/inbox/data/use-refetch-inbox";
import { InboxService } from "@/gen/nama/v1/inbox_pb";

/** The RPC that marks every notification read, then refetches the inbox. */
export function useMarkAllRead() {
  return useMutation(InboxService.method.markAllInboxNotificationsRead, useRefetchInbox());
}
