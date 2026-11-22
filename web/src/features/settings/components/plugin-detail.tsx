import { Suspense, useId } from "react";

import { PluginSections } from "@/features/admin-plugins/components/plugin-sections";
import { PointFailures } from "@/features/admin-plugins/components/point-failures";
import { LoadedPlugin } from "@/features/settings/components/loaded-plugin";
import { PaneSkeleton } from "@/features/settings/components/pane-skeleton";
import { PluginHeader } from "@/features/settings/components/plugin-header";
import { PluginPoints } from "@/features/settings/components/plugin-points";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";

/**
 * The Plugin `summary` names, in place of the list: its header, what it is, and a card for its
 * Extension Points and per part of its setup, the sections its page has. What the list already
 * knows shows at once; the rest waits for the Plugin's details.
 */
export function PluginDetail({ summary }: { summary: PluginSummary }) {
  const titleId = useId();
  const { id } = summary;
  // ponytail: the sections, styled for the Plugin page they had, restyled from here as Cards
  // (DESIGN.md, "Cards"). Now Settings alone shows them, move this into them as they next change.
  const sections = (
    <Suspense fallback={<PaneSkeleton />}>
      <LoadedPlugin id={id}>{(plugin) => <PluginSections plugin={plugin} />}</LoadedPlugin>
    </Suspense>
  );
  const cards = (
    <div className="mt-7.5 flex flex-col gap-3.5 *:mt-0 *:max-w-none *:rounded-xl *:border *:bg-card *:p-4.5">
      <PluginPoints extensionPoints={summary.extensionPoints} />
      {sections}
    </div>
  );
  const failures = (
    <Suspense fallback={null}>
      <LoadedPlugin id={id}>
        {(plugin) => <PointFailures failures={plugin.failures} />}
      </LoadedPlugin>
    </Suspense>
  );
  return (
    <section aria-labelledby={titleId} className="flex flex-col">
      <PluginHeader titleId={titleId} summary={summary} />
      <div className="flex flex-col px-4 pt-6 pb-4 md:px-5 md:pb-6">
        <p className="max-w-prose text-sm/6 text-pretty text-muted-foreground empty:hidden">
          {summary.description}
        </p>
        {failures}
        {cards}
      </div>
    </section>
  );
}
