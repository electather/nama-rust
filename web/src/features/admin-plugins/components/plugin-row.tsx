import { Link } from "@tanstack/react-router";

import { EnableOrSetUp } from "@/features/admin-plugins/components/enable-or-set-up";
import { ExtensionPoints } from "@/features/admin-plugins/components/extension-points";
import { NeedsAttention } from "@/features/admin-plugins/components/needs-attention";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { kindAndVersion, pluginName } from "@/lib/plugins";
import { AdminRow } from "@/shared/components/admin-list/admin-row";
import { PluginLogo } from "@/shared/components/plugin-logo";

/** One Plugin on the list: its logo, what it is, its state and how to enable or set it up. */
export function PluginRow({ plugin }: { plugin: PluginSummary }) {
  return (
    <AdminRow start={<PluginLogo plugin={plugin} />} end={<EnableOrSetUp plugin={plugin} />}>
      <Link
        to="/admin/plugins/$pluginId"
        params={{ pluginId: plugin.id }}
        className="truncate rounded-sm font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        {pluginName(plugin)}
      </Link>
      {plugin.description !== "" && <p className="text-sm text-pretty">{plugin.description}</p>}
      <span className="text-sm text-muted-foreground">{kindAndVersion(plugin)}</span>
      {plugin.enabled && plugin.settingsMissing && <NeedsAttention />}
      <ExtensionPoints extensionPoints={plugin.extensionPoints} />
    </AdminRow>
  );
}
