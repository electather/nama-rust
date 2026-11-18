import { refusedSettings } from "@/components/setting-refusals";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";

/**
 * Why the Server refused to save or test the settings, by whichever it refused. A `null` error
 * refused nothing.
 */
export function settingErrors(saving: unknown, testing: unknown) {
  if (saving !== null) {
    return refusedSettings(saving, "save the settings");
  }
  return testing === null ? {} : refusedSettings(testing, "test the settings");
}

/** What saving did, by whether the Plugin `summary` runs with the new settings now. */
export function saved(summary: PluginSummary | undefined) {
  return summary?.enabled === true
    ? "Saved, and in use now."
    : "Saved. They’re used once the Plugin is enabled.";
}
