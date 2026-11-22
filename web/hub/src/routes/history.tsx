import { signedInAccount } from "@hub/features/account/data/account";
import { HistoryPage } from "@hub/features/viewing/components/history-page";
import { createFileRoute, redirect } from "@tanstack/react-router";

/** The viewing history page, for a signed-in Account. */
export const Route = createFileRoute("/history")({
  beforeLoad: async ({ context }) => {
    if (!(await signedInAccount(context))) {
      throw redirect({ to: "/sign-in" });
    }
  },
  component: HistoryPage,
});
