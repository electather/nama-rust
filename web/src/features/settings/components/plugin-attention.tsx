import { Link } from "@tanstack/react-router";
import { Ban, TriangleAlert } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";

import { needsAttention } from "@/features/settings/lib/plugin-list";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { StateLabel } from "@/shared/components/state-label";
import { pluginName } from "@/shared/lib/plugins";

/**
 * What the row of a Plugin waiting on an admin says: why it waits, and for one that runs without a
 * setting it needs, a button to its view to give it. Nothing for any other Plugin, whose "…" holds
 * what there is to do.
 */
export function PluginAttention({ plugin }: { plugin: PluginSummary }) {
  if (plugin.refusedReason !== "") {
    return <StateLabel tone="warning" label="Can’t run" Icon={Ban} />;
  }
  return (
    needsAttention(plugin) && (
      <>
        <StateLabel tone="warning" label="Missing a setting" Icon={TriangleAlert} />
        <Link
          to="."
          search={(rest) => ({ ...rest, plugin: plugin.id })}
          aria-label={`Set up ${pluginName(plugin)}`}
          className={buttonVariants({ variant: "secondary" })}
        >
          Set up
        </Link>
      </>
    )
  );
}
