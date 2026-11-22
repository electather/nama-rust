import { HomeLayoutRows } from "@/features/settings/components/home-layout-rows";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { useHomeLayout } from "@/features/settings/data/use-home-layout";

/**
 * Which Home rows, and whether Featured, the signed-in User's Home shows, and the rows' order,
 * each saved as it changes. Going back to the Server's is in the header over it.
 */
export function HomePane() {
  const layout = useHomeLayout();
  return (
    <PaneFrame section="home">
      <div className="flex flex-col gap-3">
        <p className="text-xs text-pretty text-muted-foreground">
          What you leave alone follows the Server’s default. Once you move a row, the order is
          yours, and a new row goes last.
        </p>
        {layout && <HomeLayoutRows saved={layout} />}
      </div>
    </PaneFrame>
  );
}
