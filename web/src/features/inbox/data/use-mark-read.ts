import { useMutation } from "@connectrpc/connect-query";

import { useRefetchInbox } from "@/features/inbox/data/use-refetch-inbox";
import { InboxService } from "@/gen/nama/v1/inbox_pb";

/** The RPC that marks one notification read, then refetches the inbox. */
export function useMarkRead() {
  return useMutation(InboxService.method.markInboxNotificationRead, useRefetchInbox());
}
