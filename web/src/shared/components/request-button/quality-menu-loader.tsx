import { type ComponentProps, Suspense, lazy } from "react";

import { ErrorBoundary } from "@/shared/components/error-boundary";
import { QualityChevron } from "@/shared/components/request-button/quality-chevron";
import type { QualityMenu as Menu } from "@/shared/components/request-button/quality-menu";

// Base UI's menu and the Qualities load after the page, so Request shows without waiting for them.
const QualityMenu = lazy(async () => {
  const module = await import("@/shared/components/request-button/quality-menu");
  return { default: module.QualityMenu };
});

/**
 * The Quality menu once its code and the Qualities have loaded, its chevron disabled until then,
 * so Request doesn't shift when it arrives. Nothing if they fail: Request still asks at the User's
 * Quality.
 */
export function QualityMenuLoader(props: ComponentProps<typeof Menu>) {
  const loading = <QualityChevron what={props.what} variant={props.variant} disabled />;
  return (
    <ErrorBoundary failed={null}>
      <Suspense fallback={loading}>
        <QualityMenu {...props} />
      </Suspense>
    </ErrorBoundary>
  );
}
