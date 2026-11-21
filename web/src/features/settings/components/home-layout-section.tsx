import { HomeLayoutRows } from "@/features/settings/components/home-layout-rows";
import { ResetHomeLayout } from "@/features/settings/components/reset-home-layout";
import { useHomeLayout } from "@/features/settings/data/use-home-layout";

/**
 * Which Home rows, and whether Featured, the signed-in User's Home shows, and the rows' order,
 * each saved as it changes, and going back to the Server's.
 */
export function HomeLayoutSection() {
  const layout = useHomeLayout().data?.layout;
  if (layout === undefined) {
    return null;
  }
  return (
    <section aria-labelledby="home-layout" className="mt-12 max-w-2xl">
      <h2 id="home-layout" className="font-medium">
        Home
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        Drag a row, or use its arrows, to move it, and switch off what your Home shouldn’t show.
        What you leave alone follows the Server’s default. Once you move a row, the order is yours,
        and a new row goes at the end.
      </p>
      <HomeLayoutRows saved={layout} />
      <ResetHomeLayout />
    </section>
  );
}
