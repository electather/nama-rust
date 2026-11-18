import { useQueryClient } from "@tanstack/react-query";

import { pluginsKey } from "@/lib/plugins";

/** Refetches the list and each details page, for a change that may leave either out of date. */
export function useRefreshPlugins() {
  const queryClient = useQueryClient();
  return async () => {
    await queryClient.invalidateQueries({ queryKey: pluginsKey });
  };
}
