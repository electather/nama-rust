import { type ComponentProps, Suspense, lazy } from "react";

import { MoreActionsButton } from "@/features/title/components/more-actions-button";
import type { MovieMoreMenu as Menu } from "@/features/title/components/movie-more-menu";
import { ErrorBoundary } from "@/shared/components/error-boundary";

// The menu and its dialogs load after the page, so Play shows without waiting for them.
const MovieMoreMenu = lazy(async () => {
  const module = await import("@/features/title/components/movie-more-menu");
  return { default: module.MovieMoreMenu };
});

/** A Movie's "…" menu once its code has loaded, disabled until then. Nothing if it fails. */
export function MovieMoreLoader(props: ComponentProps<typeof Menu>) {
  return (
    <ErrorBoundary failed={null}>
      <Suspense fallback={<MoreActionsButton label="More actions" disabled />}>
        <MovieMoreMenu {...props} />
      </Suspense>
    </ErrorBoundary>
  );
}
