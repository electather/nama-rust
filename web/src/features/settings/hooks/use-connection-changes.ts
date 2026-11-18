import { useDeletePluginConnection } from "@/features/settings/data/use-delete-plugin-connection";
import { useStartPluginConnection } from "@/features/settings/data/use-start-plugin-connection";
import { failure } from "@/features/settings/lib/connection-wording";

/**
 * Starting and deleting a connection to the Plugin `name`, each listing the connections again once
 * it's done, and why the last one failed, if it did.
 */
export function useConnectionChanges(name: string) {
  const start = useStartPluginConnection();
  const remove = useDeletePluginConnection();
  const failed =
    failure(start.error, `connect ${name}`) ?? failure(remove.error, `disconnect ${name}`);
  return { start, remove, failed };
}
