import { createFileRoute, Outlet } from "@tanstack/react-router";

import { HubMemberLayout } from "@/app/hub-member-layout";
import { CentredUnderTopBar } from "@/features/auth/components/centred-under-top-bar";
import { SignedOutLayout } from "@/shared/components/sign-in-layout/signed-out-layout";
import { isAtHub, signedInAccount } from "@/shared/hooks/access";

export const Route = createFileRoute("/_signed-out")({
  // A signed-in Account still opens some of the Hub's, such as a Sign-in Link's page.
  beforeLoad: async ({ context }) => ({
    account: (await isAtHub(context)) ? await signedInAccount(context) : null,
  }),
  component: SignedOutRoute,
});

function SignedOutRoute() {
  const account = Route.useRouteContext({ select: (context) => context.account });
  return account ? (
    <HubMemberLayout email={account.email}>
      <CentredUnderTopBar>
        <Outlet />
      </CentredUnderTopBar>
    </HubMemberLayout>
  ) : (
    <SignedOutLayout>
      <Outlet />
    </SignedOutLayout>
  );
}
