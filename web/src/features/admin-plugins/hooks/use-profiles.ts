import { useOfferedProfiles } from "@/features/admin-plugins/data/use-offered-profiles";
import { profilesGone, qualityProfile, type Row } from "@/features/admin-plugins/lib/qualities";
import { serverMessage } from "@/shared/lib/forms";
import { type PluginDetails, pluginName } from "@/shared/lib/plugins";

/**
 * The profiles `plugin`'s settings check offers on its stored settings, why there are none when it
 * failed, and a flag under each of `rows` whose profile it doesn't offer. A secret isn't sent, so
 * the Server uses the stored one.
 */
export function useProfiles(plugin: PluginDetails, rows: Row[]) {
  const stored = Object.fromEntries(
    plugin.settings.flatMap(({ key, value }) => (value ? [[key, value]] : [])),
  );
  const offered = useOfferedProfiles(plugin.summary.id, stored);
  const profiles = offered.data?.choices[qualityProfile]?.choices;
  return {
    profiles: profiles ?? [],
    gone: profilesGone(rows, profiles, plugin),
    unoffered:
      offered.error &&
      `Couldn’t get ${pluginName(plugin.summary)}’s quality profiles: ${serverMessage(offered.error)}`,
  };
}
