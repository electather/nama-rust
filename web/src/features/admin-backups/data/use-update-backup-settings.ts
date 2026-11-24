import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { backupsKey } from "@/features/admin-backups/data/use-backups";
import { ServerService } from "@/gen/nama/v1/server_pb";

/** Saves where Backups go and how many are kept, then lists the folder now in use. */
export function useUpdateBackupSettings() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(ServerService.method.updateBackupSettings, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: backupsKey(transport) });
    },
  });
}
