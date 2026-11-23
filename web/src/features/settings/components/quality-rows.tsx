import { QualityRow } from "@/features/settings/components/quality-row";
import type { DefaultQuality } from "@/gen/nama/v1/request_pb";

/** A row per Fulfiller with a Quality to pick. */
export function QualityRows({
  defaults,
  values,
  onEdit,
}: {
  defaults: DefaultQuality[];
  values: Record<string, string>;
  onEdit: (edits: Record<string, string>) => void;
}) {
  return (
    <ul aria-label="Default Quality" className="flex flex-col">
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
    </ul>
  );
}
