import { DefaultHomeRows } from "@/features/admin-home/components/default-home-rows";
import { PaneFrame } from "@/features/settings/components/pane-frame";

/** The Home layout every User starts with, for a User who manages settings to change. */
export function DefaultHomePane() {
  return (
    <PaneFrame section="default-home">
      <div className="flex flex-col gap-2.5">
        <p className="text-xs text-pretty text-muted-foreground">
          Every User’s Home shows these rows in this order, under Featured, until they change their
          own. Drag a row, or use its arrows, to move it, and switch off what Home shouldn’t show. A
          row from a Plugin enabled later goes at the end.
        </p>
        <DefaultHomeRows />
      </div>
    </PaneFrame>
  );
}
