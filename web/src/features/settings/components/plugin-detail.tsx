import { useId } from "react";

import { PluginSections } from "@/features/admin-plugins/components/plugin-sections";
import { PointFailures } from "@/features/admin-plugins/components/point-failures";
import { usePlugin } from "@/features/admin-plugins/data/use-plugin";
import { PluginHeader } from "@/features/settings/components/plugin-header";
import { PluginPoints } from "@/features/settings/components/plugin-points";
import { hasSummary } from "@/shared/lib/plugins";

/**
 * The Plugin `id` in place of the list: its header, what it is, and a card for its Extension Points
 * and per part of its setup, the sections its page has.
 */
export function PluginDetail({ id }: { id: string }) {
  const plugin = usePlugin(id);
  const titleId = useId();
  if (!hasSummary(plugin)) {
    return null;
  }
  // ponytail: the page's sections restyled from here, each a Card (DESIGN.md, "Cards"). Give them a
  // look of their own to choose from if a third place shows them.
  const cards = (
    <div className="mt-7.5 flex flex-col gap-3.5 *:mt-0 *:max-w-none *:rounded-xl *:border *:bg-card *:p-4.5">
      <PluginPoints extensionPoints={plugin.summary.extensionPoints} />
      <PluginSections plugin={plugin} headed />
    </div>
  );
  return (
    <section aria-labelledby={titleId} className="flex flex-col">
      <PluginHeader titleId={titleId} plugin={plugin} />
      <div className="flex flex-col px-4 pt-6 pb-4 md:px-7.5 md:pb-7.5">
        <p className="max-w-prose leading-7 text-pretty empty:hidden">
          {plugin.summary.description}
        </p>
        <PointFailures failures={plugin.failures} />
        {cards}
      </div>
    </section>
  );
}
