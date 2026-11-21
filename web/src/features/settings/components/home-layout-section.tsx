import { HomeLayoutRows } from "@/features/settings/components/home-layout-rows";
import { ResetHomeLayout } from "@/features/settings/components/reset-home-layout";
import { useHomeLayout } from "@/features/settings/data/use-home-layout";

/**
 * Which Home rows, and whether Featured, the signed-in User's Home shows, each a switch that
 * saves as it's flipped, and going back to the Server's.
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
        Choose what your Home shows. Anything you don’t change follows the Server’s default.
      </p>
      <HomeLayoutRows saved={layout} />
      <ResetHomeLayout />
    </section>
  );
}
