import { type ComponentProps, Suspense, lazy } from "react";

import { MoreActionsButton } from "@/features/title/components/more-actions-button";
import type { ShowMoreMenu as Menu } from "@/features/title/components/show-more-menu";
import { ErrorBoundary } from "@/shared/components/error-boundary";

// The menu and its dialogs load after the page, so Play shows without waiting for them.
const ShowMoreMenu = lazy(async () => {
  const module = await import("@/features/title/components/show-more-menu");
  return { default: module.ShowMoreMenu };
});

/** A Show's "…" menu once its code has loaded, disabled until then. Nothing if it fails. */
export function ShowMoreLoader(props: ComponentProps<typeof Menu>) {
  return (
    <ErrorBoundary failed={null}>
      <Suspense fallback={<MoreActionsButton label="More actions" disabled />}>
        <ShowMoreMenu {...props} />
      </Suspense>
    </ErrorBoundary>
  );
}
