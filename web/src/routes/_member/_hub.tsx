import { Outlet, createFileRoute, notFound } from "@tanstack/react-router";

import { CentredUnderTopBar } from "@/features/auth/components/centred-under-top-bar";
import { isAtHub } from "@/shared/hooks/access";

/** Signed-in pages only the Hub has: a Server answers them "Page not found" (ADR 0105). */
export const Route = createFileRoute("/_member/_hub")({
  beforeLoad: async ({ context }) => {
    if (!(await isAtHub(context))) {
      throw notFound();
    }
  },
  component: HubPageLayout,
});

function HubPageLayout() {
  return (
    <CentredUnderTopBar>
      <Outlet />
    </CentredUnderTopBar>
  );
}
