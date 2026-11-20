import { DragHandle } from "@/features/admin-home/components/drag-handle";
import { DropMarker } from "@/features/admin-home/components/drop-marker";
import { MoveButtons } from "@/features/admin-home/components/move-buttons";
import { RowLabel } from "@/features/admin-home/components/row-label";
import { ShownSwitch } from "@/features/admin-home/components/shown-switch";
import type { RowDrag } from "@/features/admin-home/hooks/use-row-drag";
import type { RowName } from "@/features/admin-home/lib/home-layout";

/** One Home row of the layout, at `place`: dragged or moved by its arrows, and shown or not. */
export function LayoutRow({
  name,
  shown,
  place,
  drag,
  onMove,
  onShownChange,
}: {
  name: RowName;
  shown: boolean;
  place: { index: number; count: number };
  drag: RowDrag;
  onMove: (from: number, before: number) => void;
  onShownChange: (shown: boolean) => void;
}) {
  return (
    <li
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
