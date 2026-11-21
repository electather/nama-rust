import { useOfferedProfiles } from "@/features/admin-plugins/data/use-offered-profiles";
import { qualityProfile } from "@/features/admin-plugins/lib/qualities";
import { serverMessage } from "@/shared/lib/forms";
import { type PluginDetails, pluginName } from "@/shared/lib/plugins";

/**
 * The profiles `plugin`'s settings check offers on its stored settings, none until it answers, and
 * why there are none when it failed. A secret isn't sent, so the Server uses the stored one.
 */
export function useProfiles(plugin: PluginDetails) {
  const stored = Object.fromEntries(
    plugin.settings.flatMap(({ key, value }) => (value ? [[key, value]] : [])),
  );
  const offered = useOfferedProfiles(plugin.summary.id, stored);
  return {
    profiles: offered.data?.choices[qualityProfile]?.choices,
    unoffered: offered.error
      ? `Couldn’t get ${pluginName(plugin.summary)}’s quality profiles: ${serverMessage(offered.error)}`
      : undefined,
  };
}
