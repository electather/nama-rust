import { useState } from "react";

import { useDisablePlugin } from "@/features/admin-plugins/data/use-disable-plugin";
import { useEnablePlugin } from "@/features/admin-plugins/data/use-enable-plugin";
import { refusedChange } from "@/features/admin-plugins/lib/refused-change";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { pluginName } from "@/shared/lib/plugins";

/** Enabling or disabling a Plugin from the list, and why the Server refused the last one asked. */
export function usePluginSwitch() {
  const enable = useEnablePlugin();
  const disable = useDisablePlugin();
  const [refused, setRefused] = useState<string>();

  async function change(plugin: PluginSummary) {
    const [asked, doing] = plugin.enabled ? [disable, "disable"] : [enable, "enable"];
    setRefused(undefined);
    try {
      await asked.mutateAsync({ id: plugin.id });
    } catch (error) {
      // No settings to name a missing one by its label: the list has only summaries.
      setRefused(refusedChange(error, `${doing} ${pluginName(plugin)}`, []));
    }
  }

  return {
    refused,
    dismiss: () => {
      setRefused(undefined);
    },
    flip: (plugin: PluginSummary) => {
      void change(plugin);
    },
  };
}
