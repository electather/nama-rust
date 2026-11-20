import { type ComponentProps, Suspense, lazy } from "react";

import { ErrorBoundary } from "@/shared/components/error-boundary";
import type { QualityMenu as Menu } from "@/shared/components/request-button/quality-menu";

// Base UI's menu and the Qualities load after the page, so Request shows without waiting for them.
const QualityMenu = lazy(async () => {
  const module = await import("@/shared/components/request-button/quality-menu");
  return { default: module.QualityMenu };
});

/**
 * The Quality menu once its code and the Qualities have loaded. Nothing until then, or if they
 * fail: Request still asks at the User's Quality.
 */
export function QualityMenuLoader(props: ComponentProps<typeof Menu>) {
  return (
    <ErrorBoundary failed={null}>
      <Suspense fallback={null}>
        <QualityMenu {...props} />
      </Suspense>
    </ErrorBoundary>
  );
}
