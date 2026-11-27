import { SelectBox } from "@/features/settings/components/select-box";
import { spaceOptions } from "@/features/settings/lib/server-values";
import type { RowControl } from "@/shared/components/setting-row";

/** A dropdown of how much disk a cache, of resized copies or subtitles, may take, `value` chosen. */
export function SpaceSelect({
  control,
  value,
  onChange,
}: {
  control: RowControl;
  value: string;
  onChange: (space: string) => void;
}) {
  return (
    <SelectBox control={control} options={spaceOptions(value)} value={value} onChange={onChange} />
  );
}
