import { useId, useState } from "react";

import { AddRecognisers } from "@/features/admin-libraries/components/recognisers-field/add-recognisers";
import { RecogniserList } from "@/features/admin-libraries/components/recognisers-field/recogniser-list";
import { usePlugins } from "@/features/admin-libraries/data/use-plugins";
import {
  recognisersHint,
  recognisersIn,
  resetWith,
} from "@/features/admin-libraries/lib/recognisers";

const none: string[] = [];

/**
 * The Library's Recognisers in the order it asks them, which an Admin picks from every Recogniser
 * Plugin, enabled or not, and reorders. Sent as one `recognisers` form value per Plugin id. None
 * leaves the Server's own Matching alone.
 */
export function RecognisersField({
  initial = none,
  error,
}: {
  initial?: string[] | undefined;
  error?: string | undefined;
}) {
  const id = useId();
  const [order, setOrder] = useState(initial);
  // Only `Manage settings` lists Plugins. Without the list, the stored ids show as they are.
  const data = usePlugins();
  const listed = data !== undefined;
  const recognisers = recognisersIn(data?.summaries);

  return (
    <fieldset
      className="flex flex-col gap-2"
      aria-describedby={`${id}-hint ${id}-error`}
      ref={resetWith(() => {
        setOrder(initial);
      })}
    >
      <legend className="mb-2 text-sm font-medium">Recognisers</legend>
      <RecogniserList order={order} setOrder={setOrder} recognisers={recognisers} listed={listed} />
      <AddRecognisers
        recognisers={recognisers.filter((plugin) => !order.includes(plugin.id))}
        add={(pluginId) => {
          setOrder([...order, pluginId]);
        }}
      />
      <p id={`${id}-hint`} className="text-sm text-muted-foreground">
        {recognisersHint(listed && recognisers.length === 0 && order.length === 0)}
      </p>
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </fieldset>
  );
}
