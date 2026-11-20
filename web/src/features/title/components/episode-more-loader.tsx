import { type ComponentProps, Suspense, lazy } from "react";

import type { EpisodeMoreMenu as Menu } from "@/features/title/components/episode-more-menu";
import { MoreActionsButton } from "@/features/title/components/more-actions-button";
import { ErrorBoundary } from "@/shared/components/error-boundary";

// The menu and its dialog load after the page, so the row shows without waiting for them.
const EpisodeMoreMenu = lazy(async () => {
  const module = await import("@/features/title/components/episode-more-menu");
  return { default: module.EpisodeMoreMenu };
});

/** An Episode card's "…" once its code has loaded, disabled until then. Nothing if it fails. */
export function EpisodeMoreLoader(props: ComponentProps<typeof Menu>) {
  const loading = <MoreActionsButton label={`More for ${props.episode.title}`} quiet disabled />;
  return (
    <ErrorBoundary failed={null}>
      <Suspense fallback={loading}>
        <EpisodeMoreMenu {...props} />
      </Suspense>
    </ErrorBoundary>
  );
}
