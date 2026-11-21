import { QualityRow } from "@/features/settings/components/quality-row";
import type { DefaultQuality } from "@/gen/nama/v1/request_pb";

/** A row per Fulfiller with a Quality to pick, or that there's nothing to pick. */
export function QualityRows({
  defaults,
  values,
  onEdit,
}: {
  defaults: DefaultQuality[];
  values: Record<string, string>;
  onEdit: (edits: Record<string, string>) => void;
}) {
  if (defaults.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        Your Requests come in one Quality on this Server, so there’s nothing to pick.
      </p>
    );
  }
  return (
    <div className="flex flex-col">
      {defaults.map((fulfiller) => (
        <QualityRow
          key={fulfiller.pluginId}
          fulfiller={fulfiller}
          value={values[fulfiller.pluginId]}
          onChange={(quality) => {
            onEdit({ [fulfiller.pluginId]: quality });
          }}
        />
      ))}
    </div>
  );
}
