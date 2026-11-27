import { createQueryOptions, useSuspenseQuery, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

const { listServerFolders } = ServerService.method;

/** The folders at the top of the Server's disks: `/`'s, or on Windows its drives. */
export function useServerRoots() {
  return useSuspenseQuery(listServerFolders, { path: "" }, { staleTime: loaderStaleTime }).data
    .folders;
}

/**
 * Lists the folders directly inside a path on the Server. A path asked for again within
 * `loaderStaleTime`, or while it's being asked, isn't sent twice, so listing a path's every level
 * at once lets a tree open them later without asking again.
 */
export function useListServerFolders() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return async (path: string) => {
    const { folders } = await queryClient.query({
      ...createQueryOptions(listServerFolders, { path }, { transport }),
      staleTime: loaderStaleTime,
    });
    return folders;
  };
}
