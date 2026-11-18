import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { imageSettingsKey } from "@/features/settings/data/use-image-settings";
import { ServerService } from "@/gen/nama/v1/server_pb";

/** Switches where the images are kept, then refetches the image settings. */
export function useSwitchImageStore() {
  const queryClient = useQueryClient();
  return useMutation(ServerService.method.switchImageStore, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: imageSettingsKey });
    },
  });
}
