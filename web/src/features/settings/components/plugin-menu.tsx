import { Link } from "@tanstack/react-router";
import { Ellipsis } from "lucide-react";

import { enabledFromItsView, switchable } from "@/features/settings/lib/plugin-list";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { useModalContainer } from "@/shared/hooks/use-modal-container";
import { pluginName } from "@/shared/lib/plugins";
import { Button } from "@/shared/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLinkItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

/**
 * The "…" at the end of a Plugin's row: opening it, and enabling or disabling it. Where a setting
 * or Plugin Permissions come first, "Enable…" opens it, as its own view asks for them.
 */
export function PluginMenu({
  plugin,
  onFlip,
}: {
  plugin: PluginSummary;
  onFlip: (plugin: PluginSummary) => void;
}) {
  const [trigger, container] = useModalContainer();
  const more = (
    <Button
      ref={trigger}
      variant="ghost"
      size="icon-sm"
      aria-label={`More for ${pluginName(plugin)}`}
    />
  );
  const open = <Link to="." search={(rest) => ({ ...rest, plugin: plugin.id })} />;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={more}>
        <Ellipsis aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuLinkItem render={open}>Open</DropdownMenuLinkItem>
        {switchable(plugin) && (
          <DropdownMenuItem
            onClick={() => {
              onFlip(plugin);
            }}
          >
            {plugin.enabled ? "Disable" : "Enable"}
          </DropdownMenuItem>
        )}
        {enabledFromItsView(plugin) && (
          <DropdownMenuLinkItem render={open}>Enable…</DropdownMenuLinkItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
