import { Suspense } from "react";

import { FeaturedHero } from "@/features/home/components/featured-hero";
import { FeaturedSkeleton } from "@/features/home/components/featured-skeleton";
import { ErrorBoundary } from "@/shared/components/error-boundary";

/** Featured, loading on its own, its skeleton until then, or nothing if it fails. */
export function FeaturedSection() {
  return (
    <ErrorBoundary failed={null}>
      <Suspense fallback={<FeaturedSkeleton />}>
        <FeaturedHero />
      </Suspense>
    </ErrorBoundary>
  );
}
