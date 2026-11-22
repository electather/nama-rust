import { Ban } from "lucide-react";
import { Suspense, useEffect, useRef } from "react";

import { LoadedPlugin } from "@/features/settings/components/loaded-plugin";
import { PluginActions } from "@/features/settings/components/plugin-actions";
import { PluginMore } from "@/features/settings/components/plugin-more";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { PluginLogo } from "@/shared/components/plugin-logo";
import { StateLabel } from "@/shared/components/state-label";
import { kindAndVersion, pluginName } from "@/shared/lib/plugins";

/**
 * The top of a Plugin's view, over a hairline: its logo, its name over its id and where it comes
 * from, and at the end its "…" and, while it's disabled, Enable. One the Server refused says so
 * there instead. Where there's room it stays at the top of the pane while the rest scrolls under
 * it. Its "…" and Enable need the Plugin's details, so they appear once those load.
 */
export function PluginHeader({ titleId, summary }: { titleId: string; summary: PluginSummary }) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // The list this replaced held focus, so it moves to what took its place.
    title.current?.focus();
  }, []);
  const name = (
    <div className="flex min-w-0 flex-1 basis-50 flex-col gap-0.5">
      <h3
        ref={title}
        id={titleId}
        tabIndex={-1}
        className="truncate text-lg font-medium outline-none"
      >
        {pluginName(summary)}
      </h3>
      <p className="text-sm text-muted-foreground">
        {summary.id} · {kindAndVersion(summary)}
      </p>
    </div>
  );
  const more = (
    <Suspense fallback={null}>
      <LoadedPlugin id={summary.id}>{(plugin) => <PluginMore plugin={plugin} />}</LoadedPlugin>
    </Suspense>
  );
  const refused = (
    <div className="ms-auto flex items-center gap-2">
      <StateLabel label="Refused" Icon={Ban} />
      {more}
    </div>
  );
  const end =
    summary.refusedReason === "" ? (
      <Suspense fallback={null}>
        <LoadedPlugin id={summary.id}>{(plugin) => <PluginActions plugin={plugin} />}</LoadedPlugin>
      </Suspense>
    ) : (
      refused
    );
  return (
    <header data-sticks className="top-0 z-10 bg-background px-4 pt-4 md:sticky md:px-6 md:pt-2">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2.5 border-b pb-6">
        <PluginLogo large plugin={summary} />
        {name}
        {end}
      </div>
    </header>
  );
}
