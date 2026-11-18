import { RemoveQuality } from "@/features/admin-plugins/components/remove-quality";

/** The end of a Quality's fields: whether it is the default, and removing it. */
export function QualityEnd({
  rowKey,
  number,
  isDefault,
  onRemove,
}: {
  rowKey: number;
  number: number;
  isDefault: boolean;
  onRemove: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <label className="flex items-center gap-3 text-sm">
        <input
          type="radio"
          name="default"
          value={rowKey}
          defaultChecked={isDefault}
          className="size-4 accent-foreground"
        />
        Default
      </label>
      {/* Pulled back by its own padding, so its words line up with the fields' end. */}
      <div className="-me-4">
        <RemoveQuality number={number} onRemove={onRemove} />
      </div>
    </div>
  );
}
