import { useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { pluginsKey } from "@/features/admin-plugins/data/plugins-key";

/** Refetches the list and each Plugin's view, for a change that may leave either out of date. */
export function useRefreshPlugins() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return async () => {
    await queryClient.invalidateQueries({ queryKey: pluginsKey(transport) });
  };
}
