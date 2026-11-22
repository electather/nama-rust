import { Button } from "nama-ui/components/button";

import { NotEnabled } from "@/features/admin-libraries/components/recognisers-field/not-enabled";
import { labelOf } from "@/features/admin-libraries/lib/recognisers";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";

/** A button per Recogniser in `recognisers`, which `add`s it at the end of the order. */
export function AddRecognisers({
  recognisers,
  add,
}: {
  recognisers: PluginSummary[];
  add: (pluginId: string) => void;
}) {
  return (
    <div className="-ms-2.5 flex flex-wrap gap-1">
      {recognisers.map((plugin) => (
        <Button
          key={plugin.id}
          type="button"
          variant="ghost"
          onClick={() => {
            add(plugin.id);
          }}
        >
          Add {labelOf(plugin.id, plugin)}
          {!plugin.enabled && <NotEnabled />}
        </Button>
      ))}
    </div>
  );
}
