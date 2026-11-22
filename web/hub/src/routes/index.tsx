import { AccountPage } from "@hub/features/account/components/account-page";
import { signedInAccount } from "@hub/features/account/data/account";
import { createFileRoute, redirect } from "@tanstack/react-router";

/** The Account page, for a signed-in Account. */
export const Route = createFileRoute("/")({
  beforeLoad: async ({ context }) => {
    if (!(await signedInAccount(context))) {
      throw redirect({ to: "/sign-in" });
    }
  },
  component: AccountPage,
});
