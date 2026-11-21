import { HomeLayoutRows } from "@/features/settings/components/home-layout-rows";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { ResetHomeLayout } from "@/features/settings/components/reset-home-layout";
import { useHomeLayout } from "@/features/settings/data/use-home-layout";

/**
 * Which Home rows, and whether Featured, the signed-in User's Home shows, and the rows' order,
 * each saved as it changes, and going back to the Server's.
 */
export function HomePane() {
  const layout = useHomeLayout();
  return (
    <PaneFrame section="home" action={<ResetHomeLayout />}>
      <div className="flex flex-col gap-2.5">
        <p className="text-xs text-pretty text-muted-foreground">
          Drag a row, or use its arrows, to move it, and switch off what your Home shouldn’t show.
          What you leave alone follows the Server’s default. Once you move a row, the order is
          yours, and a new row goes at the end.
        </p>
        {layout && <HomeLayoutRows saved={layout} />}
      </div>
    </PaneFrame>
  );
}
