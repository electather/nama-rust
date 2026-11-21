import { useUpdateImageSettings } from "@/features/settings/data/use-update-image-settings";
import { useUpdateServerSettings } from "@/features/settings/data/use-update-server-settings";
import type { ServerValues } from "@/features/settings/lib/server-values";

/** Saves the Server pane: the Server URL and the image settings, each only if it changed. */
export function useSaveServer() {
  const server = useUpdateServerSettings();
  const images = useUpdateImageSettings();
  return async (values: ServerValues, changed: (keyof ServerValues)[]) => {
    await Promise.all([
      changed.includes("url") && server.mutateAsync({ settings: { url: values.url.trim() } }),
      changed.some((key) => key !== "url") &&
        images.mutateAsync({
          settings: { resizeImages: values.resize, resizedCacheMib: Number(values.space) },
        }),
    ]);
  };
}
