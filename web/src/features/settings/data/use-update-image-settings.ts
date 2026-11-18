import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { imageSettingsKey } from "@/features/settings/data/use-image-settings";
import { ServerService } from "@/gen/nama/v1/server_pb";

/** Saves the image settings, then refetches them. */
export function useUpdateImageSettings() {
  const queryClient = useQueryClient();
  return useMutation(ServerService.method.updateImageSettings, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: imageSettingsKey });
    },
  });
}
