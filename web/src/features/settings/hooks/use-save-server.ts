import { useUpdateImageSettings } from "@/features/settings/data/use-update-image-settings";
import { useUpdateServerSettings } from "@/features/settings/data/use-update-server-settings";
import { type ServerValues, serverSettingKeys } from "@/features/settings/lib/server-values";

/**
 * Saves the Server pane: the Server settings, which the Server replaces as a whole, and the image
 * settings, each only if one of them changed.
 */
export function useSaveServer() {
  const server = useUpdateServerSettings();
  const images = useUpdateImageSettings();
  return async (values: ServerValues, changed: (keyof ServerValues)[]) => {
    const { url, shareDiagnostics, shareUsageAnalytics } = values;
    await Promise.all([
      changed.some((key) => serverSettingKeys.includes(key)) &&
        server.mutateAsync({
          settings: { url: url.trim(), shareDiagnostics, shareUsageAnalytics },
        }),
      changed.some((key) => !serverSettingKeys.includes(key)) &&
        images.mutateAsync({
          settings: { resizeImages: values.resize, resizedCacheMib: Number(values.space) },
        }),
    ]);
  };
}
