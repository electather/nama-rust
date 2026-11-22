import { create } from "@bufbuild/protobuf";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { BackupSettingsSchema, ServerService } from "@/gen/nama/v1/server_pb";

/** The Backup folder, its settings and the Backups in it, with the default settings while none are saved. */
export function useBackups() {
  const { settings, folder, backups } = useSuspenseQuery(ServerService.method.getBackups, {}).data;
  return { settings: settings ?? create(BackupSettingsSchema), folder, backups };
}

/** The key of `useBackups`' query, to list the Backups again. */
export const backupsKey = createConnectQueryKey({
  schema: ServerService.method.getBackups,
  cardinality: "finite",
});
