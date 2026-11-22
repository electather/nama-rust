import { useOfferedProfiles } from "@/features/admin-plugins/data/use-offered-profiles";
import { qualityProfile } from "@/features/admin-plugins/lib/qualities";
import { serverMessage } from "@/shared/lib/forms";
import { type PluginDetails, pluginName } from "@/shared/lib/plugins";

/**
 * The profiles `plugin`'s settings check offers on its stored settings, none until it answers, and
 * why there are none when it failed.
 */
export function useProfiles(plugin: PluginDetails) {
  const offered = useOfferedProfiles(plugin);
  return {
    profiles: offered.data?.choices[qualityProfile]?.choices,
    unoffered: offered.error
      ? `Couldn’t get ${pluginName(plugin.summary)}’s quality profiles: ${serverMessage(offered.error)}`
      : undefined,
  };
}
