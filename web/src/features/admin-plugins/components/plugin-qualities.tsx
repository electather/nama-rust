import { QualitiesSection } from "@/features/admin-plugins/components/qualities-section";
import { takesQualities } from "@/features/admin-plugins/lib/qualities";
import type { PluginDetails } from "@/lib/plugins";

/**
 * The Qualities an Admin offers Requesters on a Fulfiller's page: the service's quality profiles,
 * as its settings check offers them on the stored settings, under friendly names. Nothing for a
 * Plugin that takes none.
 */
export function PluginQualities({ plugin }: { plugin: PluginDetails }) {
  return takesQualities(plugin) && <QualitiesSection plugin={plugin} />;
}
