import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { requestsKey } from "@/shared/hooks/requests-key";

/** Mutation options that refetch the Requests once a change to one is done or refused. */
export function useRefetchRequests() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  // Waits for the Requests to be read back, so the button stays busy until the change shows.
  return {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: requestsKey(transport) });
    },
  };
}
