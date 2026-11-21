import { points } from "@/features/admin-plugins/lib/extension-points";
import { asksWhichToGrant } from "@/features/admin-plugins/lib/plugin-permissions";
import { PluginKind, type PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { kindAndVersion, pluginName } from "@/shared/lib/plugins";

/** Which Plugins the list shows, by where they come from. */
export type KindFilter = "all" | "built-in" | "installed";

/** The choices of `KindFilter`, as the list's toolbar offers them. */
export const kindOptions: [KindFilter, string][] = [
  ["all", "All"],
  ["built-in", "Built-in"],
  ["installed", "Installed"],
];

/** Whether a Plugin waits on an admin: the Server can't run it, or it runs without a setting it needs. */
export function needsAttention(plugin: PluginSummary) {
  return plugin.refusedReason !== "" || (plugin.enabled && plugin.settingsMissing);
}

/**
 * Whether a disabled Plugin is enabled from its own view, which asks for what comes first: a
 * setting it needs, or which Plugin Permissions to grant.
 */
export function enabledFromItsView(plugin: PluginSummary) {
  const waits = !plugin.enabled && (plugin.settingsMissing || asksWhichToGrant(plugin));
  return plugin.refusedReason === "" && waits;
}

/** Whether the list can enable or disable a Plugin itself. One the Server refused, not at all. */
export function switchable(plugin: PluginSummary) {
  return plugin.refusedReason === "" && !enabledFromItsView(plugin);
}

/** The line under a Plugin's name: where it comes from, its version, and what it takes part at. */
export function pluginMeta(plugin: PluginSummary) {
  const takesPartAt = plugin.extensionPoints.flatMap((point) => points[point]?.label ?? []);
  return [kindAndVersion(plugin), ...takesPartAt].join(" · ");
}

function ofKind(plugin: PluginSummary, kind: KindFilter) {
  return kind === "all" || (kind === "built-in") === (plugin.kind === PluginKind.BUILT_IN);
}

/** The Plugins of `all` that are of `kind` and answer to the words `typed`. */
export function matchingPlugins(all: PluginSummary[], typed: string, kind: KindFilter) {
  const words = typed.trim().toLowerCase();
  return all.filter((plugin) => {
    const text = `${pluginName(plugin)} ${plugin.id} ${plugin.description} ${pluginMeta(plugin)}`;
    return ofKind(plugin, kind) && text.toLowerCase().includes(words);
  });
}

/** `shown` under the state each is in, the ones waiting on an admin first. A state with none is left out. */
export function pluginGroups(shown: PluginSummary[]) {
  const running = shown.filter((plugin) => !needsAttention(plugin));
  return [
    { title: "Needs attention", plugins: shown.filter((plugin) => needsAttention(plugin)) },
    { title: "Enabled", plugins: running.filter((plugin) => plugin.enabled) },
    { title: "Disabled", plugins: running.filter((plugin) => !plugin.enabled) },
  ].filter((group) => group.plugins.length > 0);
}
