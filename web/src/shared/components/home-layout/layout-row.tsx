import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { DragHandle } from "@/shared/components/home-layout/drag-handle";
import { MoveButtons } from "@/shared/components/home-layout/move-buttons";
import { RowLabel } from "@/shared/components/home-layout/row-label";
import { IconTile } from "@/shared/components/icon-tile";
import type { RowDrag } from "@/shared/hooks/use-row-drag";
import type { RowName } from "@/shared/lib/home-layout";

declare module "react" {
  interface CSSProperties {
    /** How far a Home row is from its place while one is dragged. */
    "--offset"?: string;
  }
}

/**
 * One row of a reorderable list, such as a Home row or a title source, at `place`: dragged by its
 * grip or icon, which lifts it, or moved by its arrows, dimmed while `hidden`, with its own `control`
 * at the end. `?setting=` finds it by its `setting` key.
 */
export function LayoutRow({
  setting,
  Icon,
  name,
  hidden = false,
  place,
  drag,
  onMove,
  control,
}: {
  setting?: string;
  Icon: LucideIcon;
  name: RowName;
  hidden?: boolean;
  place: { index: number; count: number };
  drag: RowDrag;
  onMove: (from: number, before: number) => void;
  control: ReactNode;
}) {
  return (
    // The rows it passes slide out of a dragged row's way; the dragged row follows the pointer.
    <li
      data-setting={setting}
      data-hidden={hidden || undefined}
      data-dragging={drag.dragging || undefined}
      data-sliding={drag.sliding || undefined}
      style={{ "--offset": `${drag.offset}px` }}
      className="group relative -mx-2 flex translate-y-(--offset) gap-x-3 rounded-lg px-2 data-dragging:z-10 data-dragging:bg-popover data-dragging:shadow-lg data-sliding:transition-transform data-sliding:duration-200 data-sliding:ease-out-expo motion-reduce:transition-none"
    >
      <DragHandle {...drag.handle}>
        <IconTile Icon={Icon} />
      </DragHandle>
      <div className="flex min-w-0 flex-1 items-center gap-3 border-t py-1.5 group-first:border-t-0 group-data-dragging:border-transparent">
        <RowLabel name={name} />
        <MoveButtons label={name.label} place={place} onMove={onMove} />
        {control}
      </div>
    </li>
  );
}
