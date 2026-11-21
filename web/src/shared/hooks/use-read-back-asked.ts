import { useQueryClient } from "@tanstack/react-query";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { requestsKey } from "@/shared/hooks/requests-key";
import { searchKey } from "@/shared/hooks/search-query";
import { refusalOf } from "@/shared/lib/forms";

/**
 * Mutation options for asking for a title, which read the Requests back once it's asked for, or
 * once it's refused because a scan Matched it since it was found, to show it in its Library.
 */
export function useReadBackAsked() {
  const queryClient = useQueryClient();
  // Waits for the Requests to be read back, so the button stays busy until the change shows.
  const readBack = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: requestsKey }),
      queryClient.invalidateQueries({ queryKey: searchKey }),
    ]);
  };
  return {
    onSuccess: readBack,
    onError: async (error: unknown) => {
      if (refusalOf(error).reason === RefusalReason.ALREADY_IN_LIBRARY) {
        await readBack();
      }
    },
  };
}
