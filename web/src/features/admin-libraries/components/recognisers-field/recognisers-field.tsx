import { useId } from "react";

import { AddRecognisers } from "@/features/admin-libraries/components/recognisers-field/add-recognisers";
import { RecogniserList } from "@/features/admin-libraries/components/recognisers-field/recogniser-list";
import { usePlugins } from "@/features/admin-libraries/data/use-plugins";
import { type PluginOrderKind, pluginsIn } from "@/features/admin-libraries/lib/recognisers";

/**
 * The Library's Plugins of `kind` (its Recognisers or Segment sources) in the `order` it asks
 * them, which an Admin picks from every such Plugin, enabled or not, and reorders, each change
 * handed to `setOrder`.
 */
export function RecognisersField({
  kind,
  order,
  setOrder,
  error,
}: {
  kind: PluginOrderKind;
  order: string[];
  setOrder: (order: string[]) => void;
  error?: string | undefined;
}) {
  const id = useId();
  // Only `Manage settings` lists Plugins. Without the list, the stored ids show as they are.
  const data = usePlugins();
  const listed = data !== undefined;
  const recognisers = pluginsIn(kind.point, data?.summaries);

  return (
    <fieldset className="flex flex-col gap-2" aria-describedby={`${id}-hint ${id}-error`}>
      <legend className="mb-2 text-sm font-medium">{kind.legend}</legend>
      <RecogniserList
        kind={kind}
        order={order}
        setOrder={setOrder}
        recognisers={recognisers}
        listed={listed}
      />
      <AddRecognisers
        recognisers={recognisers.filter((plugin) => !order.includes(plugin.id))}
        add={(pluginId) => {
          setOrder([...order, pluginId]);
        }}
      />
      <p id={`${id}-hint`} className="text-sm text-muted-foreground">
        {kind.hint(listed && recognisers.length === 0 && order.length === 0)}
      </p>
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </fieldset>
  );
}
