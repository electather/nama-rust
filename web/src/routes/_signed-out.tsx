import { createFileRoute, Outlet } from "@tanstack/react-router";

import { SignedOutLayout } from "@/shared/components/sign-in-layout/signed-out-layout";

export const Route = createFileRoute("/_signed-out")({
  component: SignedOutRoute,
});

function SignedOutRoute() {
  return (
    <SignedOutLayout>
      <Outlet />
    </SignedOutLayout>
  );
}
