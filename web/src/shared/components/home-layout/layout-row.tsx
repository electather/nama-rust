import { DragHandle } from "@/shared/components/home-layout/drag-handle";
import { DropMarker } from "@/shared/components/home-layout/drop-marker";
import { MoveButtons } from "@/shared/components/home-layout/move-buttons";
import { RowLabel } from "@/shared/components/home-layout/row-label";
import { ShownSwitch } from "@/shared/components/home-layout/shown-switch";
import type { RowDrag } from "@/shared/hooks/use-row-drag";
import type { RowName } from "@/shared/lib/home-layout";

/**
 * One Home row of the layout, at `place`: dragged or moved by its arrows, and shown or not.
 * `?setting=` finds it by its `setting` key.
 */
export function LayoutRow({
  setting,
  name,
  shown,
  place,
  drag,
  onMove,
  onShownChange,
}: {
  setting: string;
  name: RowName;
  shown: boolean;
  place: { index: number; count: number };
  drag: RowDrag;
  onMove: (from: number, before: number) => void;
  onShownChange: (shown: boolean) => void;
}) {
  return (
    <li
      data-setting={setting}
      data-dragging={drag.dragging || undefined}
      className="relative flex items-center gap-3 border-b p-3 last:border-b-0 data-dragging:bg-muted"
    >
      {drag.marker !== undefined && <DropMarker edge={drag.marker} />}
      <DragHandle {...drag.handle} />
      <RowLabel name={name} />
      <MoveButtons label={name.label} place={place} onMove={onMove} />
      <ShownSwitch label={name.label} shown={shown} onShownChange={onShownChange} />
    </li>
  );
}
