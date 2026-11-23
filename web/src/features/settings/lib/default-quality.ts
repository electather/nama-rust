import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";
import type { DefaultQuality } from "@/gen/nama/v1/request_pb";

// What each Fulfiller fetches, as its row names it.
const fetched: Partial<Record<ExtensionPoint, string>> = {
  [ExtensionPoint.MOVIE_FULFILLER]: "Movies",
  [ExtensionPoint.SHOW_FULFILLER]: "Shows",
};

/** A Fulfiller's row label, such as "Quality for Movies". */
export function qualityLabel(fulfiller: DefaultQuality) {
  return `Quality for ${fetched[fulfiller.fulfiller] ?? fulfiller.pluginId}`;
}

/**
 * What a Fulfiller's row says: what it fetches, such as "Movies", and the Plugin it's from.
 */
export function fetchedName({ fulfiller, pluginId }: DefaultQuality) {
  // ponytail: the Plugin's id capitalised stands in for its name, which `DefaultQuality` doesn't carry.
  const plugin = pluginId.charAt(0).toUpperCase() + pluginId.slice(1);
  return { label: fetched[fulfiller] ?? pluginId, from: `From ${plugin}` };
}

/** The Quality the Server asks a Fulfiller for when the User picks none: its list's default. */
function listDefault({ qualities }: DefaultQuality) {
  return qualities.find((offered) => offered.isDefault);
}

/** The User's own default on `fulfiller`'s list, if they picked one and it's still offered. */
function yours({ qualities, quality }: DefaultQuality) {
  return qualities.find((offered) => offered.id === quality);
}

/** Each Fulfiller's Quality in effect, by its Plugin's id, as its dropdown is valued. */
export function savedQualities(defaults: DefaultQuality[]): Record<string, string> {
  return Object.fromEntries(
    defaults.map((fulfiller) => [
      fulfiller.pluginId,
      String((yours(fulfiller) ?? listDefault(fulfiller))?.id),
    ]),
  );
}

/** Why the User's default no longer applies, if an Admin has since removed it from the list. */
export function removedFlag(fulfiller: DefaultQuality) {
  const removed = fulfiller.quality !== undefined && yours(fulfiller) === undefined;
  return removed
    ? `The Quality you picked is no longer offered, so your Requests get ${listDefault(fulfiller)?.name}.`
    : undefined;
}
