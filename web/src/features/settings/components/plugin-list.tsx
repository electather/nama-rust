import { useId, useState } from "react";

import { PluginGroup } from "@/features/settings/components/plugin-group";
import { PluginToolbar } from "@/features/settings/components/plugin-toolbar";
import { SaveToast } from "@/features/settings/components/save-toast";
import { usePluginSwitch } from "@/features/settings/hooks/use-plugin-switch";
import {
  type KindFilter,
  matchingPlugins,
  pluginGroups,
} from "@/features/settings/lib/plugin-list";
import { sectionTitles } from "@/features/settings/lib/sections";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";

/**
 * Every Plugin on the Server under the state it's in, the ones waiting on an admin first, narrowed
 * by what's typed and by where they come from. Each opens in place of the list.
 */
export function PluginList({ summaries }: { summaries: PluginSummary[] }) {
  const titleId = useId();
  const [typed, setTyped] = useState("");
  const [kind, setKind] = useState<KindFilter>("all");
  const { refused, dismiss, flip } = usePluginSwitch();
  const groups = pluginGroups(matchingPlugins(summaries, typed, kind));
  const listed = (
    <div className="flex flex-col gap-4 px-4 pt-3 pb-4 md:px-5 md:pb-6">
      {groups.map((group) => (
        <PluginGroup key={group.title} {...group} onFlip={flip} />
      ))}
      <output className="text-sm text-muted-foreground empty:hidden">
        {groups.length === 0 && "No Plugin matches."}
      </output>
      <p className="text-xs text-pretty text-muted-foreground">
        You add a Plugin on the computer this Server runs on, then restart it.
      </p>
    </div>
  );
  return (
    <>
      <section aria-labelledby={titleId} className="flex flex-col">
        {/* The toolbar is the top of this pane, so its title is only said. */}
        <h3 id={titleId} className="sr-only">
          {sectionTitles.plugins}
        </h3>
        <PluginToolbar typed={typed} kind={kind} onType={setTyped} onKind={setKind} />
        {listed}
      </section>
      <SaveToast label="Not changed" says={refused} onDismiss={dismiss} />
    </>
  );
}
