import { ArrowDown, ArrowUp, X } from "lucide-react";

import { RowNote } from "@/features/admin-libraries/components/recognisers-field/row-note";
import { type Moves, labelOf } from "@/features/admin-libraries/lib/recognisers";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { Button } from "@/shared/ui/button";

/** One Plugin in the order, sent as the form value `name`, with the buttons that move it or take it out. */
export function RecogniserRow({
  name: formName,
  pluginId,
  plugin,
  listed,
  moves: { up, down, remove },
}: {
  name: string;
  pluginId: string;
  plugin: PluginSummary | undefined;
  listed: boolean;
  moves: Moves;
}) {
  const label = labelOf(pluginId, plugin);
  const buttons = [
    { name: `Move ${label} up`, icon: ArrowUp, onClick: up },
    { name: `Move ${label} down`, icon: ArrowDown, onClick: down },
    { name: `Remove ${label}`, icon: X, onClick: remove },
  ];
  return (
    <li className="flex items-center gap-1 text-sm">
      <input type="hidden" name={formName} value={pluginId} />
      <span className="me-auto">
        {label}
        <RowNote plugin={plugin} listed={listed} />
      </span>
      {buttons.map(({ name, icon: Icon, onClick }) => (
        <Button
          key={name}
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={name}
          disabled={onClick === undefined}
          onClick={onClick}
        >
          <Icon aria-hidden />
        </Button>
      ))}
    </li>
  );
}
