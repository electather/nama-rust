import { RowLabel } from "@/shared/components/home-layout/row-label";
import { ShownSwitch } from "@/shared/components/home-layout/shown-switch";
import type { RowName } from "@/shared/lib/home-layout";

/** One row of the User's Home layout, or Featured, and whether their Home shows it. */
export function ShownRow({
  name,
  shown,
  onShownChange,
}: {
  name: RowName;
  shown: boolean;
  onShownChange: (shown: boolean) => void;
}) {
  return (
    <li className="flex items-center gap-3 border-b py-3 last:border-b-0">
      <RowLabel name={name} />
      <ShownSwitch label={name.label} shown={shown} onShownChange={onShownChange} />
    </li>
  );
}
