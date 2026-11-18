import { useQueryClient } from "@tanstack/react-query";

import { requestsKey } from "@/lib/events";

/** Mutation options that refetch the Requests once a change to one is done or refused. */
export function useRefetchRequests() {
  const queryClient = useQueryClient();
  // Waits for the Requests to be read back, so the button stays busy until the change shows.
  return {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: requestsKey });
    },
  };
}
