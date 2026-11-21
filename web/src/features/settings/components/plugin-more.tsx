import { ArrowUpRight, Ellipsis } from "lucide-react";

import { pluginLinks } from "@/features/admin-plugins/lib/plugin-links";
import { useModalContainer } from "@/shared/hooks/use-modal-container";
import { type PluginDetails, pluginName } from "@/shared/lib/plugins";
import { Button } from "@/shared/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLinkItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

/**
 * The "…" in a Plugin's header: its links out, each in a new tab that can't reach this one, and
 * Disable where `onDisable` is given. A Plugin with neither has none.
 */
export function PluginMore({
  plugin,
  onDisable,
}: {
  plugin: PluginDetails;
  onDisable?: (() => void) | undefined;
}) {
  const [trigger, container] = useModalContainer();
  const links = pluginLinks(plugin);
  if (links.length === 0 && onDisable === undefined) {
    return null;
  }
  const more = (
    <Button
      ref={trigger}
      variant="ghost"
      size="icon"
      aria-label={`More for ${pluginName(plugin.summary)}`}
    />
  );
  const out = links.map(({ name, href }) => (
    <DropdownMenuLinkItem
      key={name}
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      closeOnClick
    >
      {name}
      <ArrowUpRight aria-hidden className="ms-auto size-4 text-muted-foreground" />
    </DropdownMenuLinkItem>
  ));
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={more}>
        <Ellipsis aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        {out}
        {onDisable !== undefined && (
          <DropdownMenuItem onClick={onDisable}>Disable</DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
