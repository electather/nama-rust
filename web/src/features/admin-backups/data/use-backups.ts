import { create } from "@bufbuild/protobuf";
import type { Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useSuspenseQuery,
} from "@connectrpc/connect-query";

import { BackupSettingsSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Backup folder, its settings and the Backups in it, for the Backups page to load. */
export function backupsQuery(transport: Transport) {
  return createQueryOptions(ServerService.method.getBackups, {}, { transport });
}

/** What `backupsQuery` loads, with the default settings while none are saved. */
export function useBackups() {
  const { settings, folder, backups } = useSuspenseQuery(
    ServerService.method.getBackups,
    {},
    { staleTime: loaderStaleTime },
  ).data;
  return { settings: settings ?? create(BackupSettingsSchema), folder, backups };
}

/** The key of `useBackups`' query, to list the Backups again. */
export const backupsKey = createConnectQueryKey({
  schema: ServerService.method.getBackups,
  cardinality: "finite",
});
