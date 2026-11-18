import { Link } from "@tanstack/react-router";
import { PowerOff } from "lucide-react";

import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { StateLabel } from "@/shared/components/state-label";
import { buttonVariants } from "@/shared/ui/button";

/** A disabled Plugin's state, and a link to its page to set it up. */
export function SetUp({ plugin }: { plugin: PluginSummary }) {
  return (
    <div className="flex items-center gap-2">
      <StateLabel label="Disabled" Icon={PowerOff} />
      <Link
        to="/admin/plugins/$pluginId"
        params={{ pluginId: plugin.id }}
        aria-label={`Set up ${plugin.id}`}
        className={buttonVariants({ variant: "ghost" })}
      >
        Set up
      </Link>
    </div>
  );
}
