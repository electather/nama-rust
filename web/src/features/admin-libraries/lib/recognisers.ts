import { ExtensionPoint, type PluginSummary } from "@/gen/nama/v1/plugin_pb";

/** One kind of Plugin a Library asks in an order an Admin sets, as the order field shows it. */
export interface PluginOrderKind {
  /** The field's legend, which also names its list. */
  legend: string;
  point: ExtensionPoint;
  /** What the field says it does, or, while `noneInstalled`, what happens without any. */
  hint: (noneInstalled: boolean) => string;
}

/** The Plugins a Library asks what a file is. */
export const RECOGNISERS: PluginOrderKind = {
  legend: "Recognisers",
  point: ExtensionPoint.RECOGNISER,
  hint: (noneInstalled) =>
    noneInstalled
      ? "No Recogniser is installed, so the Server’s own Matching reads every file."
      : "Each file goes to these Recognisers in order, and the first that can tell says what it is. The Server’s own Matching reads the rest.",
};

/** The Plugins a Library asks where its Versions' Intro, Recap and Credits are (ADR 0075). */
export const SEGMENT_SOURCES: PluginOrderKind = {
  legend: "Segment sources",
  point: ExtensionPoint.SEGMENT_SOURCE,
  hint: (noneInstalled) =>
    noneInstalled
      ? "No Segment source is installed, so Segments come only from Chapter names."
      : "After Chapter names, these Segment sources are asked in order where the Intro, Recap and Credits are, and the first answer for each wins.",
};

/** The Plugins of `summaries` that serve `point`. */
export function pluginsIn(point: ExtensionPoint, summaries: PluginSummary[] = []) {
  return summaries.filter((plugin) => plugin.extensionPoints.includes(point));
}

/** What admins know the Plugin `pluginId` by: the name of its `plugin`, or else its id. */
export function labelOf(pluginId: string, plugin: PluginSummary | undefined) {
  return plugin === undefined || plugin.name === "" ? pluginId : plugin.name;
}

/**
 * The note under the row for `plugin`: that it's off, which the Library skips until it's enabled,
 * or, once the Plugins are `listed`, that there's no such Plugin. Until the list arrives, no stored
 * Plugin is known to be missing.
 */
export function recogniserNote(plugin: PluginSummary | undefined, listed: boolean) {
  if (plugin === undefined) {
    return listed ? "Not available" : undefined;
  }
  return plugin.enabled ? undefined : "Not enabled";
}
