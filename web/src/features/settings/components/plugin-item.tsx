import { Link } from "@tanstack/react-router";

import { ItemRow } from "@/features/settings/components/item-row";
import { PluginAttention } from "@/features/settings/components/plugin-attention";
import { PluginMenu } from "@/features/settings/components/plugin-menu";
import { usePrefetchPlugin } from "@/features/settings/data/use-prefetch-plugin";
import { pluginMeta } from "@/features/settings/lib/plugin-list";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { PluginLogo } from "@/shared/components/plugin-logo";
import { pluginName } from "@/shared/lib/plugins";

/**
 * One Plugin on the list: its logo, its name and what it is, and at the end what there is to do
 * about it and its "…". All of the row opens it but those: its name's link is stretched over it.
 */
export function PluginItem({
  plugin,
  onFlip,
}: {
  plugin: PluginSummary;
  onFlip: (plugin: PluginSummary) => void;
}) {
  const prefetch = usePrefetchPlugin();
  const warm = () => {
    void prefetch(plugin.id);
  };
  const name = (
    <Link
      onPointerEnter={warm}
      onFocus={warm}
      to="."
      search={(rest) => ({ ...rest, plugin: plugin.id })}
      className="outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/50"
    >
      {pluginName(plugin)}
    </Link>
  );
  return (
    <ItemRow
      beside
      linked
      start={<PluginLogo plugin={plugin} />}
      name={name}
      meta={pluginMeta(plugin)}
    >
      <PluginAttention plugin={plugin} />
      <PluginMenu plugin={plugin} onFlip={onFlip} />
    </ItemRow>
  );
}
