import { ContinueWatchingFrame } from "@/features/home/components/continue-watching-frame";
import { FeaturedSkeleton } from "@/features/home/components/featured-skeleton";
import { HomeLayout } from "@/features/home/components/home-layout";
import { ShelfSkeleton } from "@/features/home/components/shelf-skeleton";
import { backdropTile } from "@/features/home/lib/shelf-tiles";

/**
 * Home while its layout, the rows Plugins give, loads: skeletons only, since Home replaces this
 * page whole once it loads, and would drop a panel opened here.
 */
export function HomePending() {
  return (
    <HomeLayout hero={<FeaturedSkeleton />}>
      <ContinueWatchingFrame headingId="continue-watching-pending">
        <ShelfSkeleton tile={backdropTile} aspect="aspect-4/3" />
      </ContinueWatchingFrame>
    </HomeLayout>
  );
}
