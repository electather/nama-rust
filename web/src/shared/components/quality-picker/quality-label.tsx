import type { Quality } from "@/gen/nama/v1/plugin_pb";

/** A Quality's radio button and name, marked if it's the default. */
export function QualityLabel({
  quality,
  picked,
  descriptionId,
}: {
  quality: Quality;
  picked: boolean;
  descriptionId: string | undefined;
}) {
  return (
    <label className="flex items-center gap-3 font-medium">
      <input
        type="radio"
        name="quality"
        value={String(quality.id)}
        defaultChecked={picked}
        aria-describedby={descriptionId}
        className="size-4 accent-foreground"
      />
      <span>
        {quality.name}
        {quality.isDefault && <span className="font-normal text-muted-foreground"> (Default)</span>}
      </span>
    </label>
  );
}
