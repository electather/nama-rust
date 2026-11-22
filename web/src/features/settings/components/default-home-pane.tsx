import { DefaultHomeRows } from "@/features/admin-home/components/default-home-rows";
import { PaneFrame } from "@/features/settings/components/pane-frame";

/** The Home layout every User starts with, for a User who manages settings to change. */
export function DefaultHomePane() {
  return (
    <PaneFrame section="default-home">
      <div className="flex flex-col gap-3">
        <p className="text-xs text-pretty text-muted-foreground">
          Every User starts with this Home until they change their own. A Plugin enabled later adds
          its rows last.
        </p>
        <DefaultHomeRows />
      </div>
    </PaneFrame>
  );
}
