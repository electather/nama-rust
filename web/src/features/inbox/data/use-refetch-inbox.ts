import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { inboxKey } from "@/features/inbox/data/inbox-key";

/** Mutation options that refetch the inbox once marking is done or refused. */
export function useRefetchInbox() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  // Waits for the inbox to be read back, so "Mark all read" stays busy until the count shows.
  return {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: inboxKey(transport) });
    },
  };
}
